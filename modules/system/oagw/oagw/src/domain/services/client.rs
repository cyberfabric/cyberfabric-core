use std::sync::Arc;

use modkit_macros::domain_model;
use modkit_security::SecurityContext;
use oagw_sdk::api::ServiceGatewayClientV1;
use oagw_sdk::body::Body;
use oagw_sdk::error::ServiceGatewayError;
use uuid::Uuid;

use super::{ControlPlaneService, DataPlaneService};
use crate::domain::error::DomainError;
use crate::domain::model;

/// Facade that implements the public `ServiceGatewayClientV1` trait by
/// delegating to the internal CP and DP services.
#[domain_model]
pub(crate) struct ServiceGatewayClientV1Facade {
    cp: Arc<dyn ControlPlaneService>,
    dp: Arc<dyn DataPlaneService>,
}

impl ServiceGatewayClientV1Facade {
    pub(crate) fn new(cp: Arc<dyn ControlPlaneService>, dp: Arc<dyn DataPlaneService>) -> Self {
        Self { cp, dp }
    }
}

#[async_trait::async_trait]
impl ServiceGatewayClientV1 for ServiceGatewayClientV1Facade {
    async fn create_upstream(
        &self,
        ctx: SecurityContext,
        req: oagw_sdk::CreateUpstreamRequest,
    ) -> Result<oagw_sdk::Upstream, ServiceGatewayError> {
        let internal_req = sdk_create_upstream_to_domain(req);
        let result = self.cp.create_upstream(&ctx, internal_req).await;
        result.map(upstream_to_sdk).map_err(domain_err_to_sdk)
    }

    async fn get_upstream(
        &self,
        ctx: SecurityContext,
        id: Uuid,
    ) -> Result<oagw_sdk::Upstream, ServiceGatewayError> {
        self.cp
            .get_upstream(&ctx, id)
            .await
            .map(upstream_to_sdk)
            .map_err(domain_err_to_sdk)
    }

    async fn list_upstreams(
        &self,
        ctx: SecurityContext,
        query: &oagw_sdk::ListQuery,
    ) -> Result<Vec<oagw_sdk::Upstream>, ServiceGatewayError> {
        let q = model::ListQuery {
            top: query.top,
            skip: query.skip,
        };
        self.cp
            .list_upstreams(&ctx, &q)
            .await
            .map(|v| v.into_iter().map(upstream_to_sdk).collect())
            .map_err(domain_err_to_sdk)
    }

    async fn update_upstream(
        &self,
        ctx: SecurityContext,
        id: Uuid,
        req: oagw_sdk::UpdateUpstreamRequest,
    ) -> Result<oagw_sdk::Upstream, ServiceGatewayError> {
        let internal_req = sdk_update_upstream_to_domain(req);
        self.cp
            .update_upstream(&ctx, id, internal_req)
            .await
            .map(upstream_to_sdk)
            .map_err(domain_err_to_sdk)
    }

    async fn delete_upstream(
        &self,
        ctx: SecurityContext,
        id: Uuid,
    ) -> Result<(), ServiceGatewayError> {
        self.cp
            .delete_upstream(&ctx, id)
            .await
            .map_err(domain_err_to_sdk)
    }

    async fn create_route(
        &self,
        ctx: SecurityContext,
        req: oagw_sdk::CreateRouteRequest,
    ) -> Result<oagw_sdk::Route, ServiceGatewayError> {
        let internal_req = sdk_create_route_to_domain(req);
        self.cp
            .create_route(&ctx, internal_req)
            .await
            .map(route_to_sdk)
            .map_err(domain_err_to_sdk)
    }

    async fn get_route(
        &self,
        ctx: SecurityContext,
        id: Uuid,
    ) -> Result<oagw_sdk::Route, ServiceGatewayError> {
        self.cp
            .get_route(&ctx, id)
            .await
            .map(route_to_sdk)
            .map_err(domain_err_to_sdk)
    }

    async fn list_routes(
        &self,
        ctx: SecurityContext,
        upstream_id: Uuid,
        query: &oagw_sdk::ListQuery,
    ) -> Result<Vec<oagw_sdk::Route>, ServiceGatewayError> {
        let q = model::ListQuery {
            top: query.top,
            skip: query.skip,
        };
        self.cp
            .list_routes(&ctx, upstream_id, &q)
            .await
            .map(|v| v.into_iter().map(route_to_sdk).collect())
            .map_err(domain_err_to_sdk)
    }

    async fn update_route(
        &self,
        ctx: SecurityContext,
        id: Uuid,
        req: oagw_sdk::UpdateRouteRequest,
    ) -> Result<oagw_sdk::Route, ServiceGatewayError> {
        let internal_req = sdk_update_route_to_domain(req);
        self.cp
            .update_route(&ctx, id, internal_req)
            .await
            .map(route_to_sdk)
            .map_err(domain_err_to_sdk)
    }

    async fn delete_route(
        &self,
        ctx: SecurityContext,
        id: Uuid,
    ) -> Result<(), ServiceGatewayError> {
        self.cp
            .delete_route(&ctx, id)
            .await
            .map_err(domain_err_to_sdk)
    }

    async fn resolve_upstream(
        &self,
        ctx: SecurityContext,
        alias: &str,
    ) -> Result<oagw_sdk::Upstream, ServiceGatewayError> {
        self.cp
            .resolve_upstream(&ctx, alias)
            .await
            .map(upstream_to_sdk)
            .map_err(domain_err_to_sdk)
    }

    async fn resolve_route(
        &self,
        ctx: SecurityContext,
        upstream_id: Uuid,
        method: &str,
        path: &str,
    ) -> Result<oagw_sdk::Route, ServiceGatewayError> {
        self.cp
            .resolve_route(&ctx, upstream_id, method, path)
            .await
            .map(route_to_sdk)
            .map_err(domain_err_to_sdk)
    }

    async fn proxy_request(
        &self,
        ctx: SecurityContext,
        req: http::Request<Body>,
    ) -> Result<http::Response<Body>, ServiceGatewayError> {
        self.dp
            .proxy_request(ctx, req)
            .await
            .map_err(domain_err_to_sdk)
    }
}

// ---------------------------------------------------------------------------
// DomainError → ServiceGatewayError
// ---------------------------------------------------------------------------

fn domain_err_to_sdk(err: DomainError) -> ServiceGatewayError {
    match err {
        DomainError::NotFound { entity, id } => ServiceGatewayError::NotFound {
            entity: entity.to_string(),
            instance: format!("{entity}/{id}"),
        },
        DomainError::Conflict { detail } => ServiceGatewayError::ValidationError {
            detail,
            instance: String::new(),
        },
        DomainError::Validation { detail, instance } => {
            ServiceGatewayError::ValidationError { detail, instance }
        }
        DomainError::UpstreamDisabled { alias } => ServiceGatewayError::UpstreamDisabled {
            detail: format!("upstream '{alias}' is disabled"),
            instance: String::new(),
        },
        DomainError::Internal { message } => ServiceGatewayError::DownstreamError {
            detail: message,
            instance: String::new(),
        },
        DomainError::MissingTargetHost { instance } => {
            ServiceGatewayError::MissingTargetHost { instance }
        }
        DomainError::InvalidTargetHost { instance } => {
            ServiceGatewayError::InvalidTargetHost { instance }
        }
        DomainError::UnknownTargetHost { detail, instance } => {
            ServiceGatewayError::UnknownTargetHost { detail, instance }
        }
        DomainError::AuthenticationFailed { detail, instance } => {
            ServiceGatewayError::AuthenticationFailed { detail, instance }
        }
        DomainError::PayloadTooLarge { detail, instance } => {
            ServiceGatewayError::PayloadTooLarge { detail, instance }
        }
        DomainError::RateLimitExceeded {
            detail,
            instance,
            retry_after_secs,
        } => ServiceGatewayError::RateLimitExceeded {
            detail,
            instance,
            retry_after_secs,
        },
        DomainError::SecretNotFound { detail, instance } => {
            ServiceGatewayError::SecretNotFound { detail, instance }
        }
        DomainError::DownstreamError { detail, instance } => {
            ServiceGatewayError::DownstreamError { detail, instance }
        }
        DomainError::ProtocolError { detail, instance } => {
            ServiceGatewayError::ProtocolError { detail, instance }
        }
        DomainError::ConnectionTimeout { detail, instance } => {
            ServiceGatewayError::ConnectionTimeout { detail, instance }
        }
        DomainError::RequestTimeout { detail, instance } => {
            ServiceGatewayError::RequestTimeout { detail, instance }
        }
        DomainError::Forbidden { detail } => ServiceGatewayError::Forbidden { detail },
    }
}

// ---------------------------------------------------------------------------
// SDK request → domain request conversions (using SDK getters for private fields)
// ---------------------------------------------------------------------------

fn sdk_create_upstream_to_domain(
    req: oagw_sdk::CreateUpstreamRequest,
) -> model::CreateUpstreamRequest {
    model::CreateUpstreamRequest {
        server: server_to_domain(req.server().clone()),
        protocol: req.protocol().to_string(),
        alias: req.alias().map(|s| s.to_string()),
        auth: req.auth().cloned().map(auth_config_to_domain),
        headers: req.headers().cloned().map(headers_config_to_domain),
        plugins: req.plugins().cloned().map(plugins_config_to_domain),
        rate_limit: req.rate_limit().cloned().map(rate_limit_config_to_domain),
        tags: req.tags().to_vec(),
        enabled: req.enabled(),
    }
}

fn sdk_update_upstream_to_domain(
    req: oagw_sdk::UpdateUpstreamRequest,
) -> model::UpdateUpstreamRequest {
    model::UpdateUpstreamRequest {
        server: req.server().cloned().map(server_to_domain),
        protocol: req.protocol().map(|s| s.to_string()),
        alias: req.alias().map(|s| s.to_string()),
        auth: req.auth().cloned().map(auth_config_to_domain),
        headers: req.headers().cloned().map(headers_config_to_domain),
        plugins: req.plugins().cloned().map(plugins_config_to_domain),
        rate_limit: req.rate_limit().cloned().map(rate_limit_config_to_domain),
        tags: req.tags().map(|s| s.to_vec()),
        enabled: req.enabled(),
    }
}

fn sdk_create_route_to_domain(req: oagw_sdk::CreateRouteRequest) -> model::CreateRouteRequest {
    model::CreateRouteRequest {
        upstream_id: req.upstream_id(),
        match_rules: match_rules_to_domain(req.match_rules().clone()),
        plugins: req.plugins().cloned().map(plugins_config_to_domain),
        rate_limit: req.rate_limit().cloned().map(rate_limit_config_to_domain),
        tags: req.tags().to_vec(),
        priority: req.priority(),
        enabled: req.enabled(),
    }
}

fn sdk_update_route_to_domain(req: oagw_sdk::UpdateRouteRequest) -> model::UpdateRouteRequest {
    model::UpdateRouteRequest {
        match_rules: req.match_rules().cloned().map(match_rules_to_domain),
        plugins: req.plugins().cloned().map(plugins_config_to_domain),
        rate_limit: req.rate_limit().cloned().map(rate_limit_config_to_domain),
        tags: req.tags().map(|s| s.to_vec()),
        priority: req.priority(),
        enabled: req.enabled(),
    }
}

// ---------------------------------------------------------------------------
// SDK value types → domain value types
// ---------------------------------------------------------------------------

fn sharing_mode_to_domain(v: oagw_sdk::SharingMode) -> model::SharingMode {
    match v {
        oagw_sdk::SharingMode::Private => model::SharingMode::Private,
        oagw_sdk::SharingMode::Inherit => model::SharingMode::Inherit,
        oagw_sdk::SharingMode::Enforce => model::SharingMode::Enforce,
    }
}

fn scheme_to_domain(v: oagw_sdk::Scheme) -> model::Scheme {
    match v {
        oagw_sdk::Scheme::Http => model::Scheme::Http,
        oagw_sdk::Scheme::Https => model::Scheme::Https,
        oagw_sdk::Scheme::Wss => model::Scheme::Wss,
        oagw_sdk::Scheme::Wt => model::Scheme::Wt,
        oagw_sdk::Scheme::Grpc => model::Scheme::Grpc,
    }
}

fn endpoint_to_domain(v: oagw_sdk::Endpoint) -> model::Endpoint {
    model::Endpoint {
        scheme: scheme_to_domain(v.scheme),
        host: v.host,
        port: v.port,
    }
}

fn server_to_domain(v: oagw_sdk::Server) -> model::Server {
    model::Server {
        endpoints: v.endpoints.into_iter().map(endpoint_to_domain).collect(),
    }
}

fn auth_config_to_domain(v: oagw_sdk::AuthConfig) -> model::AuthConfig {
    model::AuthConfig {
        plugin_type: v.plugin_type,
        sharing: sharing_mode_to_domain(v.sharing),
        config: v.config,
    }
}

fn passthrough_mode_to_domain(v: oagw_sdk::PassthroughMode) -> model::PassthroughMode {
    match v {
        oagw_sdk::PassthroughMode::None => model::PassthroughMode::None,
        oagw_sdk::PassthroughMode::Allowlist => model::PassthroughMode::Allowlist,
        oagw_sdk::PassthroughMode::All => model::PassthroughMode::All,
    }
}

fn request_header_rules_to_domain(v: oagw_sdk::RequestHeaderRules) -> model::RequestHeaderRules {
    model::RequestHeaderRules {
        set: v.set,
        add: v.add,
        remove: v.remove,
        passthrough: passthrough_mode_to_domain(v.passthrough),
        passthrough_allowlist: v.passthrough_allowlist,
    }
}

fn response_header_rules_to_domain(v: oagw_sdk::ResponseHeaderRules) -> model::ResponseHeaderRules {
    model::ResponseHeaderRules {
        set: v.set,
        add: v.add,
        remove: v.remove,
    }
}

fn headers_config_to_domain(v: oagw_sdk::HeadersConfig) -> model::HeadersConfig {
    model::HeadersConfig {
        request: v.request.map(request_header_rules_to_domain),
        response: v.response.map(response_header_rules_to_domain),
    }
}

fn window_to_domain(v: oagw_sdk::Window) -> model::Window {
    match v {
        oagw_sdk::Window::Second => model::Window::Second,
        oagw_sdk::Window::Minute => model::Window::Minute,
        oagw_sdk::Window::Hour => model::Window::Hour,
        oagw_sdk::Window::Day => model::Window::Day,
    }
}

fn rate_limit_config_to_domain(v: oagw_sdk::RateLimitConfig) -> model::RateLimitConfig {
    model::RateLimitConfig {
        sharing: sharing_mode_to_domain(v.sharing),
        algorithm: match v.algorithm {
            oagw_sdk::RateLimitAlgorithm::TokenBucket => model::RateLimitAlgorithm::TokenBucket,
            oagw_sdk::RateLimitAlgorithm::SlidingWindow => model::RateLimitAlgorithm::SlidingWindow,
        },
        sustained: model::SustainedRate {
            rate: v.sustained.rate,
            window: window_to_domain(v.sustained.window),
        },
        burst: v.burst.map(|b| model::BurstConfig {
            capacity: b.capacity,
        }),
        scope: match v.scope {
            oagw_sdk::RateLimitScope::Global => model::RateLimitScope::Global,
            oagw_sdk::RateLimitScope::Tenant => model::RateLimitScope::Tenant,
            oagw_sdk::RateLimitScope::User => model::RateLimitScope::User,
            oagw_sdk::RateLimitScope::Ip => model::RateLimitScope::Ip,
            oagw_sdk::RateLimitScope::Route => model::RateLimitScope::Route,
        },
        strategy: match v.strategy {
            oagw_sdk::RateLimitStrategy::Reject => model::RateLimitStrategy::Reject,
            oagw_sdk::RateLimitStrategy::Queue => model::RateLimitStrategy::Queue,
            oagw_sdk::RateLimitStrategy::Degrade => model::RateLimitStrategy::Degrade,
        },
        cost: v.cost,
    }
}

fn plugins_config_to_domain(v: oagw_sdk::PluginsConfig) -> model::PluginsConfig {
    model::PluginsConfig {
        sharing: sharing_mode_to_domain(v.sharing),
        items: v.items,
    }
}

fn http_method_to_domain(v: oagw_sdk::HttpMethod) -> model::HttpMethod {
    match v {
        oagw_sdk::HttpMethod::Get => model::HttpMethod::Get,
        oagw_sdk::HttpMethod::Post => model::HttpMethod::Post,
        oagw_sdk::HttpMethod::Put => model::HttpMethod::Put,
        oagw_sdk::HttpMethod::Delete => model::HttpMethod::Delete,
        oagw_sdk::HttpMethod::Patch => model::HttpMethod::Patch,
    }
}

fn http_match_to_domain(v: oagw_sdk::HttpMatch) -> model::HttpMatch {
    model::HttpMatch {
        methods: v.methods.into_iter().map(http_method_to_domain).collect(),
        path: v.path,
        query_allowlist: v.query_allowlist,
        path_suffix_mode: match v.path_suffix_mode {
            oagw_sdk::PathSuffixMode::Disabled => model::PathSuffixMode::Disabled,
            oagw_sdk::PathSuffixMode::Append => model::PathSuffixMode::Append,
        },
    }
}

fn grpc_match_to_domain(v: oagw_sdk::GrpcMatch) -> model::GrpcMatch {
    model::GrpcMatch {
        service: v.service,
        method: v.method,
    }
}

fn match_rules_to_domain(v: oagw_sdk::MatchRules) -> model::MatchRules {
    model::MatchRules {
        http: v.http.map(http_match_to_domain),
        grpc: v.grpc.map(grpc_match_to_domain),
    }
}

// ---------------------------------------------------------------------------
// domain value types → SDK value types
// ---------------------------------------------------------------------------

fn sharing_mode_to_sdk(v: model::SharingMode) -> oagw_sdk::SharingMode {
    match v {
        model::SharingMode::Private => oagw_sdk::SharingMode::Private,
        model::SharingMode::Inherit => oagw_sdk::SharingMode::Inherit,
        model::SharingMode::Enforce => oagw_sdk::SharingMode::Enforce,
    }
}

fn scheme_to_sdk(v: model::Scheme) -> oagw_sdk::Scheme {
    match v {
        model::Scheme::Http => oagw_sdk::Scheme::Http,
        model::Scheme::Https => oagw_sdk::Scheme::Https,
        model::Scheme::Wss => oagw_sdk::Scheme::Wss,
        model::Scheme::Wt => oagw_sdk::Scheme::Wt,
        model::Scheme::Grpc => oagw_sdk::Scheme::Grpc,
    }
}

fn upstream_to_sdk(u: model::Upstream) -> oagw_sdk::Upstream {
    oagw_sdk::Upstream {
        id: u.id,
        tenant_id: u.tenant_id,
        alias: u.alias,
        server: oagw_sdk::Server {
            endpoints: u
                .server
                .endpoints
                .into_iter()
                .map(|e| oagw_sdk::Endpoint {
                    scheme: scheme_to_sdk(e.scheme),
                    host: e.host,
                    port: e.port,
                })
                .collect(),
        },
        protocol: u.protocol,
        enabled: u.enabled,
        auth: u.auth.map(|a| oagw_sdk::AuthConfig {
            plugin_type: a.plugin_type,
            sharing: sharing_mode_to_sdk(a.sharing),
            config: a.config,
        }),
        headers: u.headers.map(|h| oagw_sdk::HeadersConfig {
            request: h.request.map(|r| oagw_sdk::RequestHeaderRules {
                set: r.set,
                add: r.add,
                remove: r.remove,
                passthrough: match r.passthrough {
                    model::PassthroughMode::None => oagw_sdk::PassthroughMode::None,
                    model::PassthroughMode::Allowlist => oagw_sdk::PassthroughMode::Allowlist,
                    model::PassthroughMode::All => oagw_sdk::PassthroughMode::All,
                },
                passthrough_allowlist: r.passthrough_allowlist,
            }),
            response: h.response.map(|r| oagw_sdk::ResponseHeaderRules {
                set: r.set,
                add: r.add,
                remove: r.remove,
            }),
        }),
        plugins: u.plugins.map(|p| oagw_sdk::PluginsConfig {
            sharing: sharing_mode_to_sdk(p.sharing),
            items: p.items,
        }),
        rate_limit: u.rate_limit.map(rate_limit_config_to_sdk),
        tags: u.tags,
    }
}

fn route_to_sdk(r: model::Route) -> oagw_sdk::Route {
    oagw_sdk::Route {
        id: r.id,
        tenant_id: r.tenant_id,
        upstream_id: r.upstream_id,
        match_rules: oagw_sdk::MatchRules {
            http: r.match_rules.http.map(|h| oagw_sdk::HttpMatch {
                methods: h
                    .methods
                    .into_iter()
                    .map(|m| match m {
                        model::HttpMethod::Get => oagw_sdk::HttpMethod::Get,
                        model::HttpMethod::Post => oagw_sdk::HttpMethod::Post,
                        model::HttpMethod::Put => oagw_sdk::HttpMethod::Put,
                        model::HttpMethod::Delete => oagw_sdk::HttpMethod::Delete,
                        model::HttpMethod::Patch => oagw_sdk::HttpMethod::Patch,
                    })
                    .collect(),
                path: h.path,
                query_allowlist: h.query_allowlist,
                path_suffix_mode: match h.path_suffix_mode {
                    model::PathSuffixMode::Disabled => oagw_sdk::PathSuffixMode::Disabled,
                    model::PathSuffixMode::Append => oagw_sdk::PathSuffixMode::Append,
                },
            }),
            grpc: r.match_rules.grpc.map(|g| oagw_sdk::GrpcMatch {
                service: g.service,
                method: g.method,
            }),
        },
        plugins: r.plugins.map(|p| oagw_sdk::PluginsConfig {
            sharing: sharing_mode_to_sdk(p.sharing),
            items: p.items,
        }),
        rate_limit: r.rate_limit.map(rate_limit_config_to_sdk),
        tags: r.tags,
        priority: r.priority,
        enabled: r.enabled,
    }
}

fn rate_limit_config_to_sdk(v: model::RateLimitConfig) -> oagw_sdk::RateLimitConfig {
    oagw_sdk::RateLimitConfig {
        sharing: sharing_mode_to_sdk(v.sharing),
        algorithm: match v.algorithm {
            model::RateLimitAlgorithm::TokenBucket => oagw_sdk::RateLimitAlgorithm::TokenBucket,
            model::RateLimitAlgorithm::SlidingWindow => oagw_sdk::RateLimitAlgorithm::SlidingWindow,
        },
        sustained: oagw_sdk::SustainedRate {
            rate: v.sustained.rate,
            window: match v.sustained.window {
                model::Window::Second => oagw_sdk::Window::Second,
                model::Window::Minute => oagw_sdk::Window::Minute,
                model::Window::Hour => oagw_sdk::Window::Hour,
                model::Window::Day => oagw_sdk::Window::Day,
            },
        },
        burst: v.burst.map(|b| oagw_sdk::BurstConfig {
            capacity: b.capacity,
        }),
        scope: match v.scope {
            model::RateLimitScope::Global => oagw_sdk::RateLimitScope::Global,
            model::RateLimitScope::Tenant => oagw_sdk::RateLimitScope::Tenant,
            model::RateLimitScope::User => oagw_sdk::RateLimitScope::User,
            model::RateLimitScope::Ip => oagw_sdk::RateLimitScope::Ip,
            model::RateLimitScope::Route => oagw_sdk::RateLimitScope::Route,
        },
        strategy: match v.strategy {
            model::RateLimitStrategy::Reject => oagw_sdk::RateLimitStrategy::Reject,
            model::RateLimitStrategy::Queue => oagw_sdk::RateLimitStrategy::Queue,
            model::RateLimitStrategy::Degrade => oagw_sdk::RateLimitStrategy::Degrade,
        },
        cost: v.cost,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn auth_config_hashmap_round_trips() {
        let mut config = HashMap::new();
        config.insert("header".into(), "authorization".into());
        config.insert("prefix".into(), "Bearer ".into());
        let sdk_auth = oagw_sdk::AuthConfig {
            plugin_type: "test-plugin".into(),
            sharing: oagw_sdk::SharingMode::Private,
            config: Some(config.clone()),
        };
        let domain_auth = auth_config_to_domain(sdk_auth);
        assert_eq!(domain_auth.plugin_type, "test-plugin");
        assert_eq!(domain_auth.sharing, model::SharingMode::Private);
        assert_eq!(domain_auth.config.unwrap(), config);
    }

    #[test]
    fn auth_config_none_stays_none() {
        let sdk_auth = oagw_sdk::AuthConfig {
            plugin_type: "noop".into(),
            sharing: oagw_sdk::SharingMode::Inherit,
            config: None,
        };
        let domain_auth = auth_config_to_domain(sdk_auth);
        assert!(domain_auth.config.is_none());
        assert_eq!(domain_auth.sharing, model::SharingMode::Inherit);
    }

    #[test]
    fn upstream_to_sdk_converts_auth_config_back() {
        let mut config = HashMap::new();
        config.insert("header".into(), "x-api-key".into());
        config.insert("secret_ref".into(), "cred://key".into());

        let domain_upstream = model::Upstream {
            id: Uuid::nil(),
            tenant_id: Uuid::nil(),
            alias: "test".into(),
            server: model::Server {
                endpoints: vec![model::Endpoint {
                    scheme: model::Scheme::Https,
                    host: "example.com".into(),
                    port: 443,
                }],
            },
            protocol: "http".into(),
            enabled: true,
            auth: Some(model::AuthConfig {
                plugin_type: "apikey".into(),
                sharing: model::SharingMode::Private,
                config: Some(config),
            }),
            headers: None,
            plugins: None,
            rate_limit: None,
            tags: vec![],
        };

        let sdk = upstream_to_sdk(domain_upstream);
        let auth = sdk.auth.unwrap();
        assert_eq!(auth.plugin_type, "apikey");
        let config = auth.config.unwrap();
        assert_eq!(config.get("header").unwrap(), "x-api-key");
        assert_eq!(config.get("secret_ref").unwrap(), "cred://key");
    }

    #[test]
    fn domain_err_not_found_maps_to_sdk() {
        let err = DomainError::NotFound {
            entity: "upstream",
            id: Uuid::nil(),
        };
        let sdk_err = domain_err_to_sdk(err);
        assert!(matches!(
            sdk_err,
            ServiceGatewayError::NotFound { ref entity, .. } if entity == "upstream"
        ));
    }

    #[test]
    fn domain_err_validation_maps_to_sdk() {
        let err = DomainError::Validation {
            detail: "bad input".into(),
            instance: "/test".into(),
        };
        let sdk_err = domain_err_to_sdk(err);
        assert!(matches!(
            sdk_err,
            ServiceGatewayError::ValidationError { .. }
        ));
    }

    #[test]
    fn domain_err_rate_limit_maps_to_sdk() {
        let err = DomainError::RateLimitExceeded {
            detail: "too fast".into(),
            instance: "/api".into(),
            retry_after_secs: Some(30),
        };
        let sdk_err = domain_err_to_sdk(err);
        match sdk_err {
            ServiceGatewayError::RateLimitExceeded {
                retry_after_secs, ..
            } => assert_eq!(retry_after_secs, Some(30)),
            _ => panic!("expected RateLimitExceeded"),
        }
    }

    #[test]
    fn domain_err_timeout_maps_to_sdk() {
        let err = DomainError::RequestTimeout {
            detail: "timed out".into(),
            instance: "/slow".into(),
        };
        let sdk_err = domain_err_to_sdk(err);
        assert!(matches!(
            sdk_err,
            ServiceGatewayError::RequestTimeout { .. }
        ));
    }

    #[test]
    fn sharing_mode_round_trip() {
        for (sdk_val, expected_domain) in [
            (oagw_sdk::SharingMode::Private, model::SharingMode::Private),
            (oagw_sdk::SharingMode::Inherit, model::SharingMode::Inherit),
            (oagw_sdk::SharingMode::Enforce, model::SharingMode::Enforce),
        ] {
            let domain = sharing_mode_to_domain(sdk_val);
            assert_eq!(domain, expected_domain);
            let back = sharing_mode_to_sdk(domain);
            assert_eq!(back, sdk_val);
        }
    }

    #[test]
    fn scheme_round_trip() {
        for (sdk_val, expected_domain) in [
            (oagw_sdk::Scheme::Http, model::Scheme::Http),
            (oagw_sdk::Scheme::Https, model::Scheme::Https),
            (oagw_sdk::Scheme::Wss, model::Scheme::Wss),
            (oagw_sdk::Scheme::Wt, model::Scheme::Wt),
            (oagw_sdk::Scheme::Grpc, model::Scheme::Grpc),
        ] {
            let domain = scheme_to_domain(sdk_val);
            assert_eq!(domain, expected_domain);
            let back = scheme_to_sdk(domain);
            assert_eq!(back, sdk_val);
        }
    }
}
