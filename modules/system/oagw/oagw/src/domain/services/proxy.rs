use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use crate::domain::credential::CredentialResolver;
use crate::domain::dto::{BodyStream, BoxError, ErrorSource, PassthroughMode, PathSuffixMode, ProxyContext, ProxyResponse};
use crate::domain::error::DomainError;
use crate::domain::plugin::AuthContext;

use super::{ControlPlaneService, DataPlaneService};

use crate::domain::plugin::AuthPluginRegistry;
use crate::domain::rate_limit::RateLimiter;

use crate::infra::proxy::headers;
use crate::infra::proxy::request_builder;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Data Plane service implementation: proxy orchestration and plugin execution.
pub struct DataPlaneServiceImpl {
    cp: Arc<dyn ControlPlaneService>,
    http_client: reqwest::Client,
    auth_registry: AuthPluginRegistry,
    rate_limiter: RateLimiter,
    request_timeout: Duration,
}

impl DataPlaneServiceImpl {
    #[must_use]
    pub fn new(
        cp: Arc<dyn ControlPlaneService>,
        credential_resolver: Arc<dyn CredentialResolver>,
    ) -> Self {
        let http_client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            // No overall timeout — SSE streams run indefinitely.
            // Request-header timeout is applied via tokio::time::timeout below.
            .build()
            .expect("failed to build HTTP client");

        let auth_registry = AuthPluginRegistry::with_builtins(credential_resolver);
        let rate_limiter = RateLimiter::new();

        Self {
            cp,
            http_client,
            auth_registry,
            rate_limiter,
            request_timeout: REQUEST_TIMEOUT,
        }
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
    async fn proxy_request(&self, ctx: ProxyContext) -> Result<ProxyResponse, DomainError> {
        let instance_uri = ctx.instance_uri.clone();

        // 1. Resolve upstream by alias.
        let upstream = self.cp.resolve_upstream(&ctx.ctx, &ctx.alias).await?;

        // 2. Resolve route.
        let route = self
            .cp
            .resolve_route(
                &ctx.ctx,
                upstream.id,
                ctx.method.as_ref(),
                &ctx.path_suffix,
            )
            .await?;

        // 2b. Validate query parameters against route's allowlist.
        if let Some(ref http_match) = route.match_rules.http
            && !ctx.query_params.is_empty()
        {
            for (key, _) in &ctx.query_params {
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
            let extra = ctx
                .path_suffix
                .strip_prefix(route_path.as_str())
                .unwrap_or("");
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
        let mut outbound_headers = headers::apply_passthrough(&ctx.headers, &mode, &allowlist);
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
            let mut auth_ctx = AuthContext {
                headers: outbound_headers.clone(),
                config: auth.config.clone().unwrap_or(serde_json::Value::Null),
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
                    _ => DomainError::AuthenticationFailed {
                        detail: e.to_string(),
                        instance: instance_uri.clone(),
                    },
                })?;
            outbound_headers = auth_ctx.headers;
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
        let remaining_suffix = ctx.path_suffix.strip_prefix(route_path).unwrap_or("");
        let url = request_builder::build_upstream_url(
            endpoint,
            route_path,
            remaining_suffix,
            &ctx.query_params,
        );

        // 8. Forward request with timeout on response headers.
        let send_future = self
            .http_client
            .request(ctx.method, &url)
            .headers(outbound_headers)
            .body(ctx.body)
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
        let resp_headers = response.headers().clone();

        let body_stream: BodyStream = Box::pin(
            response
                .bytes_stream()
                .map(|r| r.map_err(|e| Box::new(e) as BoxError)),
        );

        Ok(ProxyResponse {
            status,
            headers: resp_headers,
            body: body_stream,
            error_source: ErrorSource::Upstream,
        })
    }
}
