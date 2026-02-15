use std::sync::Arc;

use modkit_security::SecurityContext;
use crate::domain::error::DomainError;
use crate::domain::dto::{
    CreateRouteRequest, CreateUpstreamRequest, ListQuery, Route, UpdateRouteRequest,
    UpdateUpstreamRequest, Upstream,
};
use crate::domain::repo::{RouteRepository, UpstreamRepository};
use modkit_macros::domain_model;
use super::ControlPlaneService;
use uuid::Uuid;

/// Control Plane service implementation backed by in-memory repositories.
#[domain_model]
pub(crate) struct ControlPlaneServiceImpl {
    upstreams: Arc<dyn UpstreamRepository>,
    routes: Arc<dyn RouteRepository>,
}

impl ControlPlaneServiceImpl {
    #[must_use]
    pub(crate) fn new(
        upstreams: Arc<dyn UpstreamRepository>,
        routes: Arc<dyn RouteRepository>,
    ) -> Self {
        Self { upstreams, routes }
    }
}

/// Generate an alias from the upstream's server endpoints.
/// Single endpoint: host (standard port omitted) or host:port.
fn generate_alias(upstream: &Upstream) -> String {
    let endpoints = &upstream.server.endpoints;
    if endpoints.is_empty() {
        return String::new();
    }
    // Use the first endpoint for alias generation.
    endpoints[0].alias_contribution()
}

#[async_trait::async_trait]
impl ControlPlaneService for ControlPlaneServiceImpl {
    // -- Upstream CRUD --

    async fn create_upstream(
        &self,
        ctx: &SecurityContext,
        req: CreateUpstreamRequest,
    ) -> Result<Upstream, DomainError> {
        let tenant_id = ctx.tenant_id();
        let id = Uuid::new_v4();

        let upstream = Upstream {
            id,
            tenant_id,
            alias: String::new(),
            server: req.server.clone(),
            protocol: req.protocol.clone(),
            enabled: req.enabled,
            auth: req.auth.clone(),
            headers: req.headers.clone(),
            plugins: req.plugins.clone(),
            rate_limit: req.rate_limit.clone(),
            tags: req.tags.clone(),
        };

        let alias = req
            .alias
            .clone()
            .unwrap_or_else(|| generate_alias(&upstream));

        let upstream = Upstream { alias, ..upstream };

        self.upstreams.create(upstream).await.map_err(DomainError::from)
    }

    async fn get_upstream(&self, ctx: &SecurityContext, id: Uuid) -> Result<Upstream, DomainError> {
        let tenant_id = ctx.tenant_id();
        self.upstreams
            .get_by_id(tenant_id, id)
            .await
            .map_err(|_| DomainError::not_found("upstream", id))
    }

    async fn list_upstreams(
        &self,
        ctx: &SecurityContext,
        query: &ListQuery,
    ) -> Result<Vec<Upstream>, DomainError> {
        let tenant_id = ctx.tenant_id();
        self.upstreams
            .list(tenant_id, query)
            .await
            .map_err(DomainError::from)
    }

    async fn update_upstream(
        &self,
        ctx: &SecurityContext,
        id: Uuid,
        req: UpdateUpstreamRequest,
    ) -> Result<Upstream, DomainError> {
        let tenant_id = ctx.tenant_id();
        let mut existing = self
            .upstreams
            .get_by_id(tenant_id, id)
            .await
            .map_err(|_| DomainError::not_found("upstream", id))?;

        // Apply partial update.
        if let Some(server) = req.server {
            existing.server = server;
        }
        if let Some(protocol) = req.protocol {
            existing.protocol = protocol;
        }
        if let Some(alias) = req.alias {
            existing.alias = alias;
        }
        if let Some(auth) = req.auth {
            existing.auth = Some(auth);
        }
        if let Some(headers) = req.headers {
            existing.headers = Some(headers);
        }
        if let Some(plugins) = req.plugins {
            existing.plugins = Some(plugins);
        }
        if let Some(rate_limit) = req.rate_limit {
            existing.rate_limit = Some(rate_limit);
        }
        if let Some(tags) = req.tags {
            existing.tags = tags;
        }
        if let Some(enabled) = req.enabled {
            existing.enabled = enabled;
        }

        self.upstreams
            .update(existing)
            .await
            .map_err(DomainError::from)
    }

    async fn delete_upstream(&self, ctx: &SecurityContext, id: Uuid) -> Result<(), DomainError> {
        let tenant_id = ctx.tenant_id();
        // Cascade delete routes.
        let _ = self.routes.delete_by_upstream(tenant_id, id).await;
        self.upstreams
            .delete(tenant_id, id)
            .await
            .map_err(|_| DomainError::not_found("upstream", id))
    }

    // -- Route CRUD --

    async fn create_route(
        &self,
        ctx: &SecurityContext,
        req: CreateRouteRequest,
    ) -> Result<Route, DomainError> {
        let tenant_id = ctx.tenant_id();
        // Validate that the upstream exists and belongs to this tenant.
        self.upstreams
            .get_by_id(tenant_id, req.upstream_id)
            .await
            .map_err(|_| {
                DomainError::validation(format!(
                    "upstream '{}' not found for this tenant",
                    req.upstream_id
                ))
            })?;

        let route = Route {
            id: Uuid::new_v4(),
            tenant_id,
            upstream_id: req.upstream_id,
            match_rules: req.match_rules,
            plugins: req.plugins,
            rate_limit: req.rate_limit,
            tags: req.tags,
            priority: req.priority,
            enabled: req.enabled,
        };

        self.routes.create(route).await.map_err(DomainError::from)
    }

    async fn get_route(&self, ctx: &SecurityContext, id: Uuid) -> Result<Route, DomainError> {
        let tenant_id = ctx.tenant_id();
        self.routes
            .get_by_id(tenant_id, id)
            .await
            .map_err(|_| DomainError::not_found("route", id))
    }

    async fn list_routes(
        &self,
        ctx: &SecurityContext,
        upstream_id: Uuid,
        query: &ListQuery,
    ) -> Result<Vec<Route>, DomainError> {
        let tenant_id = ctx.tenant_id();
        self.routes
            .list_by_upstream(tenant_id, upstream_id, query)
            .await
            .map_err(DomainError::from)
    }

    async fn update_route(
        &self,
        ctx: &SecurityContext,
        id: Uuid,
        req: UpdateRouteRequest,
    ) -> Result<Route, DomainError> {
        let tenant_id = ctx.tenant_id();
        let mut existing = self
            .routes
            .get_by_id(tenant_id, id)
            .await
            .map_err(|_| DomainError::not_found("route", id))?;

        if let Some(match_rules) = req.match_rules {
            existing.match_rules = match_rules;
        }
        if let Some(plugins) = req.plugins {
            existing.plugins = Some(plugins);
        }
        if let Some(rate_limit) = req.rate_limit {
            existing.rate_limit = Some(rate_limit);
        }
        if let Some(tags) = req.tags {
            existing.tags = tags;
        }
        if let Some(priority) = req.priority {
            existing.priority = priority;
        }
        if let Some(enabled) = req.enabled {
            existing.enabled = enabled;
        }

        self.routes
            .update(existing)
            .await
            .map_err(DomainError::from)
    }

    async fn delete_route(&self, ctx: &SecurityContext, id: Uuid) -> Result<(), DomainError> {
        let tenant_id = ctx.tenant_id();
        self.routes
            .delete(tenant_id, id)
            .await
            .map_err(|_| DomainError::not_found("route", id))
    }

    // -- Resolution --

    async fn resolve_upstream(&self, ctx: &SecurityContext, alias: &str) -> Result<Upstream, DomainError> {
        let tenant_id = ctx.tenant_id();
        let upstream = self
            .upstreams
            .get_by_alias(tenant_id, alias)
            .await
            .map_err(|_| DomainError::not_found("upstream", Uuid::nil()))?;

        if !upstream.enabled {
            return Err(DomainError::upstream_disabled(alias));
        }

        Ok(upstream)
    }

    async fn resolve_route(
        &self,
        ctx: &SecurityContext,
        upstream_id: Uuid,
        method: &str,
        path: &str,
    ) -> Result<Route, DomainError> {
        let tenant_id = ctx.tenant_id();
        self.routes
            .find_matching(tenant_id, upstream_id, method, path)
            .await
            .map_err(|_| DomainError::not_found("route", Uuid::nil()))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::domain::dto::{
        Endpoint, HttpMatch, HttpMethod, MatchRules, PathSuffixMode, Scheme, Server,
    };

    use crate::infra::storage::{InMemoryRouteRepo, InMemoryUpstreamRepo};
    use super::*;

    fn make_service() -> ControlPlaneServiceImpl {
        ControlPlaneServiceImpl::new(
            Arc::new(InMemoryUpstreamRepo::new()),
            Arc::new(InMemoryRouteRepo::new()),
        )
    }

    fn test_ctx(tenant_id: Uuid) -> SecurityContext {
        SecurityContext::builder()
            .tenant_id(tenant_id)
            .subject_id(Uuid::new_v4())
            .build()
    }

    fn make_create_upstream(alias: Option<&str>) -> CreateUpstreamRequest {
        CreateUpstreamRequest {
            server: Server {
                endpoints: vec![Endpoint {
                    scheme: Scheme::Https,
                    host: "api.openai.com".into(),
                    port: 443,
                }],
            },
            protocol: "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1".into(),
            alias: alias.map(String::from),
            auth: None,
            headers: None,
            plugins: None,
            rate_limit: None,
            tags: vec![],
            enabled: true,
        }
    }

    fn make_create_route(upstream_id: Uuid) -> CreateRouteRequest {
        CreateRouteRequest {
            upstream_id,
            match_rules: MatchRules {
                http: Some(HttpMatch {
                    methods: vec![HttpMethod::Post],
                    path: "/v1/chat/completions".into(),
                    query_allowlist: vec![],
                    path_suffix_mode: PathSuffixMode::Append,
                }),
                grpc: None,
            },
            plugins: None,
            rate_limit: None,
            tags: vec![],
            priority: 0,
            enabled: true,
        }
    }

    #[tokio::test]
    async fn upstream_crud_lifecycle() {
        let svc = make_service();
        let tenant = Uuid::new_v4();
        let ctx = test_ctx(tenant);

        // Create
        let u = svc
            .create_upstream(&ctx, make_create_upstream(Some("openai")))
            .await
            .unwrap();
        assert_eq!(u.alias, "openai");

        // Get
        let fetched = svc.get_upstream(&ctx, u.id).await.unwrap();
        assert_eq!(fetched.id, u.id);

        // Update
        let updated = svc
            .update_upstream(
                &ctx,
                u.id,
                UpdateUpstreamRequest {
                    alias: Some("openai-v2".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(updated.alias, "openai-v2");
        assert_eq!(updated.id, u.id);

        // List
        let list = svc
            .list_upstreams(&ctx, &ListQuery::default())
            .await
            .unwrap();
        assert_eq!(list.len(), 1);

        // Delete
        svc.delete_upstream(&ctx, u.id).await.unwrap();
        assert!(svc.get_upstream(&ctx, u.id).await.is_err());
    }

    #[tokio::test]
    async fn alias_auto_generation() {
        let svc = make_service();
        let tenant = Uuid::new_v4();
        let ctx = test_ctx(tenant);

        // Standard port (443) — port omitted in alias.
        let u1 = svc
            .create_upstream(&ctx, make_create_upstream(None))
            .await
            .unwrap();
        assert_eq!(u1.alias, "api.openai.com");

        // Non-standard port — port included.
        let req = CreateUpstreamRequest {
            server: Server {
                endpoints: vec![Endpoint {
                    scheme: Scheme::Https,
                    host: "api.openai.com".into(),
                    port: 8443,
                }],
            },
            protocol: "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1".into(),
            alias: None,
            auth: None,
            headers: None,
            plugins: None,
            rate_limit: None,
            tags: vec![],
            enabled: true,
        };
        let u2 = svc.create_upstream(&ctx, req).await.unwrap();
        assert_eq!(u2.alias, "api.openai.com:8443");
    }

    #[tokio::test]
    async fn duplicate_alias_conflict() {
        let svc = make_service();
        let tenant = Uuid::new_v4();
        let ctx = test_ctx(tenant);

        svc.create_upstream(&ctx, make_create_upstream(Some("openai")))
            .await
            .unwrap();

        let err = svc
            .create_upstream(&ctx, make_create_upstream(Some("openai")))
            .await
            .unwrap_err();
        assert!(matches!(err, DomainError::Conflict { .. }));
    }

    #[tokio::test]
    async fn route_create_with_wrong_tenant_upstream() {
        let svc = make_service();
        let t1 = Uuid::new_v4();
        let t2 = Uuid::new_v4();
        let ctx1 = test_ctx(t1);
        let ctx2 = test_ctx(t2);

        let u = svc
            .create_upstream(&ctx1, make_create_upstream(Some("openai")))
            .await
            .unwrap();

        // Try to create route in different tenant referencing t1's upstream.
        let err = svc
            .create_route(&ctx2, make_create_route(u.id))
            .await
            .unwrap_err();
        assert!(matches!(err, DomainError::Validation { .. }));
    }

    #[tokio::test]
    async fn alias_resolution_enabled() {
        let svc = make_service();
        let tenant = Uuid::new_v4();
        let ctx = test_ctx(tenant);

        let u = svc
            .create_upstream(&ctx, make_create_upstream(Some("openai")))
            .await
            .unwrap();

        let resolved = svc.resolve_upstream(&ctx, "openai").await.unwrap();
        assert_eq!(resolved.id, u.id);
    }

    #[tokio::test]
    async fn alias_resolution_disabled_returns_503() {
        let svc = make_service();
        let tenant = Uuid::new_v4();
        let ctx = test_ctx(tenant);

        let u = svc
            .create_upstream(&ctx, make_create_upstream(Some("openai")))
            .await
            .unwrap();

        // Disable the upstream.
        svc.update_upstream(
            &ctx,
            u.id,
            UpdateUpstreamRequest {
                enabled: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let err = svc.resolve_upstream(&ctx, "openai").await.unwrap_err();
        assert!(matches!(err, DomainError::UpstreamDisabled { .. }));
    }

    #[tokio::test]
    async fn alias_resolution_nonexistent_returns_404() {
        let svc = make_service();
        let tenant = Uuid::new_v4();
        let ctx = test_ctx(tenant);

        let err = svc
            .resolve_upstream(&ctx, "nonexistent")
            .await
            .unwrap_err();
        assert!(matches!(err, DomainError::NotFound { .. }));
    }

    #[tokio::test]
    async fn route_matching_through_cp() {
        let svc = make_service();
        let tenant = Uuid::new_v4();
        let ctx = test_ctx(tenant);

        let u = svc
            .create_upstream(&ctx, make_create_upstream(Some("openai")))
            .await
            .unwrap();
        let r = svc
            .create_route(&ctx, make_create_route(u.id))
            .await
            .unwrap();

        let matched = svc
            .resolve_route(&ctx, u.id, "POST", "/v1/chat/completions")
            .await
            .unwrap();
        assert_eq!(matched.id, r.id);
    }

    #[tokio::test]
    async fn route_matching_no_match_returns_404() {
        let svc = make_service();
        let tenant = Uuid::new_v4();
        let ctx = test_ctx(tenant);

        let u = svc
            .create_upstream(&ctx, make_create_upstream(Some("openai")))
            .await
            .unwrap();

        let err = svc
            .resolve_route(&ctx, u.id, "GET", "/v1/unknown")
            .await
            .unwrap_err();
        assert!(matches!(err, DomainError::NotFound { .. }));
    }

    #[tokio::test]
    async fn delete_upstream_cascades_routes() {
        let svc = make_service();
        let tenant = Uuid::new_v4();
        let ctx = test_ctx(tenant);

        let u = svc
            .create_upstream(&ctx, make_create_upstream(Some("openai")))
            .await
            .unwrap();
        let r = svc
            .create_route(&ctx, make_create_route(u.id))
            .await
            .unwrap();

        svc.delete_upstream(&ctx, u.id).await.unwrap();

        // Route should be gone.
        assert!(svc.get_route(&ctx, r.id).await.is_err());
    }
}
