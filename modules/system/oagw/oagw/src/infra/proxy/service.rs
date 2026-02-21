use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::domain::credential::CredentialResolver;
use crate::domain::error::DomainError;
use crate::domain::model::{PassthroughMode, PathSuffixMode};
use crate::domain::plugin::AuthContext;
use crate::infra::proxy::{actions, resources};
use authz_resolver_sdk::PolicyEnforcer;
use authz_resolver_sdk::pep::AccessRequest;
use futures_util::StreamExt;
use http::{HeaderMap, HeaderName, HeaderValue};
use modkit_security::SecurityContext;
use oagw_sdk::api::ErrorSource;
use oagw_sdk::body::{Body, BodyStream, BoxError};

use crate::domain::services::{ControlPlaneService, DataPlaneService};

use crate::domain::rate_limit::RateLimiter;
use crate::infra::plugin::AuthPluginRegistry;

use super::headers;
use super::request_builder;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Data Plane service implementation: proxy orchestration and plugin execution.
pub struct DataPlaneServiceImpl {
    cp: Arc<dyn ControlPlaneService>,
    http_client: reqwest::Client,
    auth_registry: AuthPluginRegistry,
    rate_limiter: RateLimiter,
    request_timeout: Duration,
    /// Enforces authorization policy before proxying each request.
    policy_enforcer: PolicyEnforcer,
}

impl DataPlaneServiceImpl {
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built.
    pub fn new(
        cp: Arc<dyn ControlPlaneService>,
        credential_resolver: Arc<dyn CredentialResolver>,
        policy_enforcer: PolicyEnforcer,
    ) -> anyhow::Result<Self> {
        let http_client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            // Never follow redirects — upstream could redirect to internal/metadata IPs.
            .redirect(reqwest::redirect::Policy::none())
            // No overall timeout — SSE streams run indefinitely.
            // Request-header timeout is applied via tokio::time::timeout below.
            .build()?;

        let auth_registry = AuthPluginRegistry::with_builtins(credential_resolver);
        let rate_limiter = RateLimiter::new();

        Ok(Self {
            cp,
            http_client,
            auth_registry,
            rate_limiter,
            request_timeout: REQUEST_TIMEOUT,
            policy_enforcer,
        })
    }

    /// Override the request timeout.
    #[must_use]
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }
}

#[async_trait::async_trait]
impl DataPlaneService for DataPlaneServiceImpl {
    async fn proxy_request(
        &self,
        ctx: SecurityContext,
        req: http::Request<Body>,
    ) -> Result<http::Response<Body>, DomainError> {
        let instance_uri = req.uri().to_string();

        self.policy_enforcer
            .access_scope_with(
                &ctx,
                &resources::PROXY,
                actions::INVOKE,
                None,
                &AccessRequest::new()
                    .require_constraints(false)
                    .context_tenant_id(ctx.subject_tenant_id()),
            )
            .await?;

        // Normalize and parse alias and path_suffix from URI.
        let (alias, path_suffix) = {
            let path = req.uri().path();
            let normalized = normalize_path(path);
            let trimmed = normalized.strip_prefix('/').unwrap_or(&normalized);
            match trimmed.find('/') {
                Some(pos) => (trimmed[..pos].to_string(), trimmed[pos..].to_string()),
                None => (trimmed.to_string(), String::new()),
            }
        };

        // Parse query parameters with proper URL decoding.
        let query_params: Vec<(String, String)> = req
            .uri()
            .query()
            .map(|q| {
                form_urlencoded::parse(q.as_bytes())
                    .map(|(k, v)| (k.into_owned(), v.into_owned()))
                    .collect()
            })
            .unwrap_or_default();

        let (parts, body) = req.into_parts();
        let method = parts.method;
        let req_headers = parts.headers;

        // Convert Body to Bytes for the outbound HTTP request.
        let body_bytes = body
            .into_bytes()
            .await
            .map_err(|e| DomainError::Validation {
                detail: format!("failed to read request body: {e}"),
                instance: instance_uri.clone(),
            })?;

        // 1. Resolve upstream by alias.
        let upstream = self.cp.resolve_upstream(&ctx, &alias).await?;

        // 2. Resolve route.
        let route = self
            .cp
            .resolve_route(&ctx, upstream.id, method.as_ref(), &path_suffix)
            .await?;

        // 2b. Validate query parameters against route's allowlist.
        if let Some(ref http_match) = route.match_rules.http
            && !query_params.is_empty()
        {
            for (key, _) in &query_params {
                if !http_match.query_allowlist.contains(key) {
                    return Err(DomainError::Validation {
                        detail: format!(
                            "query parameter '{}' is not in the route's query_allowlist",
                            key
                        ),
                        instance: instance_uri,
                    });
                }
            }
        }

        // 2c. Enforce path_suffix_mode.
        if let Some(ref http_match) = route.match_rules.http
            && http_match.path_suffix_mode == PathSuffixMode::Disabled
        {
            let route_path = &http_match.path;
            let extra = path_suffix.strip_prefix(route_path.as_str()).unwrap_or("");
            if !extra.is_empty() {
                return Err(DomainError::Validation {
                    detail: format!(
                        "path suffix not allowed: route path_suffix_mode is disabled but request has extra path '{}'",
                        extra
                    ),
                    instance: instance_uri,
                });
            }
        }

        // 3. Prepare outbound headers (passthrough + strip).
        let mode = upstream
            .headers
            .as_ref()
            .and_then(|h| h.request.as_ref())
            .map_or(PassthroughMode::None, |r| r.passthrough);
        let allowlist: Vec<String> = upstream
            .headers
            .as_ref()
            .and_then(|h| h.request.as_ref())
            .map_or_else(Vec::new, |r| r.passthrough_allowlist.clone());
        let mut outbound_headers = headers::apply_passthrough(&req_headers, &mode, &allowlist);
        headers::strip_hop_by_hop(&mut outbound_headers);
        headers::strip_internal_headers(&mut outbound_headers);

        // 4. Execute auth plugin.
        if let Some(ref auth) = upstream.auth {
            let plugin = self.auth_registry.resolve(&auth.plugin_type).map_err(|e| {
                DomainError::AuthenticationFailed {
                    detail: e.to_string(),
                    instance: instance_uri.clone(),
                }
            })?;
            let auth_headers: HashMap<String, String> = outbound_headers
                .iter()
                .filter_map(|(k, v)| {
                    v.to_str()
                        .ok()
                        .map(|s| (k.as_str().to_string(), s.to_string()))
                })
                .collect();
            let mut auth_ctx = AuthContext {
                headers: auth_headers,
                config: auth.config.clone().unwrap_or_default(),
            };
            plugin
                .authenticate(&mut auth_ctx)
                .await
                .map_err(|e| match e {
                    crate::domain::plugin::PluginError::SecretNotFound(ref s) => {
                        DomainError::SecretNotFound {
                            detail: s.clone(),
                            instance: instance_uri.clone(),
                        }
                    }
                    crate::domain::plugin::PluginError::Rejected(ref msg) => {
                        DomainError::Validation {
                            detail: msg.clone(),
                            instance: instance_uri.clone(),
                        }
                    }
                    crate::domain::plugin::PluginError::AuthFailed(_)
                    | crate::domain::plugin::PluginError::Internal(_) => {
                        DomainError::AuthenticationFailed {
                            detail: e.to_string(),
                            instance: instance_uri.clone(),
                        }
                    }
                })?;
            outbound_headers = HeaderMap::new();
            for (k, v) in &auth_ctx.headers {
                if let (Ok(name), Ok(val)) = (
                    HeaderName::from_bytes(k.as_bytes()),
                    HeaderValue::from_str(v),
                ) {
                    outbound_headers.insert(name, val);
                }
            }
        }

        // 5. Apply header rules + set Host.
        if let Some(ref hc) = upstream.headers
            && let Some(ref rules) = hc.request
        {
            headers::apply_header_rules(&mut outbound_headers, rules);
        }
        let endpoint =
            upstream
                .server
                .endpoints
                .first()
                .ok_or_else(|| DomainError::DownstreamError {
                    detail: "upstream has no endpoints".into(),
                    instance: instance_uri.clone(),
                })?;
        headers::set_host_header(&mut outbound_headers, &endpoint.host, endpoint.port);

        // 6. Check rate limit (upstream then route).
        if let Some(ref rl) = upstream.rate_limit {
            let key = format!("upstream:{}", upstream.id);
            self.rate_limiter.try_consume(&key, rl, &instance_uri)?;
        }
        if let Some(ref rl) = route.rate_limit {
            let key = format!("route:{}", route.id);
            self.rate_limiter.try_consume(&key, rl, &instance_uri)?;
        }

        // 7. Build URL.
        // path_suffix is the full path from the proxy URL; strip the route prefix
        // so we get: endpoint + route_path + remaining_suffix.
        let route_path = route
            .match_rules
            .http
            .as_ref()
            .map_or("/", |h| h.path.as_str());
        let remaining_suffix = path_suffix.strip_prefix(route_path).unwrap_or("");
        let url = request_builder::build_upstream_url(
            endpoint,
            route_path,
            remaining_suffix,
            &query_params,
        )?;

        // 8. Forward request with timeout on response headers.
        let send_future = self
            .http_client
            .request(method, &url)
            .headers(outbound_headers)
            .body(body_bytes)
            .send();

        let timeout = self.request_timeout;
        let response = tokio::time::timeout(timeout, send_future)
            .await
            .map_err(|_| DomainError::RequestTimeout {
                detail: format!("request to {url} timed out after {timeout:?}"),
                instance: instance_uri.clone(),
            })?
            .map_err(|e| {
                if e.is_connect() {
                    DomainError::ConnectionTimeout {
                        detail: e.to_string(),
                        instance: instance_uri.clone(),
                    }
                } else {
                    DomainError::DownstreamError {
                        detail: e.to_string(),
                        instance: instance_uri.clone(),
                    }
                }
            })?;

        // 9. Build streaming response.
        let status = response.status();
        let mut resp_headers = response.headers().clone();
        headers::sanitize_response_headers(&mut resp_headers);

        let body_stream: BodyStream = Box::pin(
            response
                .bytes_stream()
                .map(|r| r.map_err(|e| Box::new(e) as BoxError)),
        );

        let mut resp = http::Response::builder()
            .status(status)
            .body(Body::Stream(body_stream))
            .map_err(|e| DomainError::DownstreamError {
                detail: format!("failed to build response: {e}"),
                instance: instance_uri,
            })?;

        *resp.headers_mut() = resp_headers;
        resp.extensions_mut().insert(ErrorSource::Upstream);

        Ok(resp)
    }
}

/// Normalize a URL path: collapse consecutive slashes and resolve `.`/`..` segments.
/// Segments that would escape above the root are discarded.
fn normalize_path(path: &str) -> String {
    let mut segments: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            s => segments.push(s),
        }
    }
    let mut result = String::with_capacity(path.len());
    if path.starts_with('/') {
        result.push('/');
    }
    result.push_str(&segments.join("/"));
    result
}

#[cfg(test)]
mod tests {
    use super::normalize_path;

    #[test]
    fn normalize_collapses_double_slashes() {
        assert_eq!(normalize_path("/alias//v1//chat"), "/alias/v1/chat");
    }

    #[test]
    fn normalize_resolves_dot_dot() {
        assert_eq!(normalize_path("/alias/../admin/secret"), "/admin/secret");
    }

    #[test]
    fn normalize_clamps_above_root() {
        assert_eq!(normalize_path("/alias/../../etc/passwd"), "/etc/passwd");
    }

    #[test]
    fn normalize_resolves_single_dot() {
        assert_eq!(normalize_path("/alias/./v1/chat"), "/alias/v1/chat");
    }

    #[test]
    fn normalize_preserves_clean_path() {
        assert_eq!(normalize_path("/alias/v1/chat"), "/alias/v1/chat");
    }
}
