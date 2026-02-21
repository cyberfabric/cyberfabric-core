use std::sync::{Arc, OnceLock};
use std::time::Duration;

use crate::config::OagwConfig;
use crate::domain::credential::CredentialResolver;
use crate::domain::type_catalog::oagw_gts_entities;
use crate::domain::type_provisioning::TypeProvisioningService;
use crate::infra::type_provisioning::TypeProvisioningServiceImpl;
use async_trait::async_trait;
use authz_resolver_sdk::{AuthZResolverClient, PolicyEnforcer};
use modkit::api::OpenApiRegistry;
use modkit::contracts::SystemCapability;
use modkit::{Module, ModuleCtx, RestApiCapability};
use modkit_security::SecurityContext;
use oagw_sdk::api::ServiceGatewayClientV1;
use tracing::info;
use types_registry_sdk::{RegisterResult, RegisterSummary, TypesRegistryClient};

use crate::api::rest::routes;
use crate::domain::services::{
    ControlPlaneService, ControlPlaneServiceImpl, DataPlaneService, ServiceGatewayClientV1Facade,
};
use crate::infra::proxy::DataPlaneServiceImpl;
use crate::infra::storage::{InMemoryCredentialResolver, InMemoryRouteRepo, InMemoryUpstreamRepo};

/// Shared application state injected into all handlers.
#[derive(Clone)]
pub struct AppState {
    pub(crate) cp: Arc<dyn ControlPlaneService>,
    pub(crate) dp: Arc<dyn DataPlaneService>,
    pub(crate) config: crate::config::RuntimeConfig,
}

/// Outbound API Gateway module: wires repos, services, and routes.
#[modkit::module(
    name = "oagw",
    deps = ["types-registry", "authz-resolver"],
    capabilities = [system, rest]
)]
pub struct OutboundApiGatewayModule {
    state: arc_swap::ArcSwapOption<AppState>,
    registry_client: OnceLock<Arc<dyn TypesRegistryClient>>,
    type_provisioning: OnceLock<Arc<dyn TypeProvisioningService>>,
}

impl Default for OutboundApiGatewayModule {
    fn default() -> Self {
        Self {
            state: arc_swap::ArcSwapOption::from(None),
            registry_client: OnceLock::new(),
            type_provisioning: OnceLock::new(),
        }
    }
}

#[async_trait]
impl Module for OutboundApiGatewayModule {
    async fn init(&self, ctx: &ModuleCtx) -> anyhow::Result<()> {
        info!("Initializing Outbound API Gateway module");

        let cfg: OagwConfig = ctx.config()?;
        info!("OAGW config: proxy_timeout_secs={}", cfg.proxy_timeout_secs);

        // -- Control Plane init --
        let upstream_repo = Arc::new(InMemoryUpstreamRepo::new());
        let route_repo = Arc::new(InMemoryRouteRepo::new());
        let cp: Arc<dyn ControlPlaneService> =
            Arc::new(ControlPlaneServiceImpl::new(upstream_repo, route_repo));

        let cred_resolver = InMemoryCredentialResolver::new();
        for (secret_ref, value) in &cfg.credentials {
            info!("Seeding credential: {secret_ref}");
            cred_resolver.set(secret_ref.clone(), value.clone());
        }
        let cred_resolver: Arc<dyn CredentialResolver> = Arc::new(cred_resolver);

        ctx.client_hub()
            .register::<dyn CredentialResolver>(cred_resolver.clone());

        // -- AuthZ resolver for permission checks --
        let authz = ctx.client_hub().get::<dyn AuthZResolverClient>()?;
        let policy_enforcer = PolicyEnforcer::new(authz);

        // -- Data Plane init --
        let dp: Arc<dyn DataPlaneService> = Arc::new(
            DataPlaneServiceImpl::new(cp.clone(), cred_resolver, policy_enforcer)?
                .with_request_timeout(Duration::from_secs(cfg.proxy_timeout_secs)),
        );

        // -- Facade (for external SDK consumers) --
        let oagw: Arc<dyn ServiceGatewayClientV1> =
            Arc::new(ServiceGatewayClientV1Facade::new(cp.clone(), dp.clone()));

        ctx.client_hub()
            .register::<dyn ServiceGatewayClientV1>(oagw.clone());

        // -- Types Registry: register GTS schemas and builtin instances --
        let registry = ctx.client_hub().get::<dyn TypesRegistryClient>()?;
        let entities = oagw_gts_entities();
        let entity_count = entities.len();
        let results = registry.register(entities).await?;
        let summary = RegisterSummary::from_results(&results);
        if !summary.all_succeeded() {
            for result in &results {
                if let RegisterResult::Err { gts_id, error } = result {
                    tracing::error!(
                        gts_id = gts_id.as_deref().unwrap_or("<unknown>"),
                        error = %error,
                        "Failed to register OAGW GTS entity"
                    );
                }
            }
            anyhow::bail!(
                "OAGW type registration failed: {}/{} entities failed",
                summary.failed,
                summary.total()
            );
        }
        info!(
            count = entity_count,
            "Registered OAGW GTS entities in types-registry"
        );

        self.registry_client
            .set(registry)
            .map_err(|_| anyhow::anyhow!("TypesRegistryClient already set"))?;

        let app_state = AppState {
            cp,
            dp,
            config: (&cfg).into(),
        };

        self.state.store(Some(Arc::new(app_state)));
        info!("Outbound API Gateway module initialized");
        Ok(())
    }
}

#[async_trait]
impl SystemCapability for OutboundApiGatewayModule {
    async fn post_init(&self, _sys: &modkit::runtime::SystemContext) -> anyhow::Result<()> {
        let registry = self
            .registry_client
            .get()
            .ok_or_else(|| anyhow::anyhow!("TypesRegistryClient not set — init() must run first"))?
            .clone();

        let provisioning: Arc<dyn TypeProvisioningService> =
            Arc::new(TypeProvisioningServiceImpl::new(registry));

        // -- Materialize provisioned upstreams and routes into in-memory repos --
        let app_state = self
            .state
            .load()
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("AppState not set — init() must run first"))?
            .as_ref()
            .clone();

        let upstreams = provisioning.list_upstreams().await?;
        for u in &upstreams {
            let ctx = SecurityContext::builder()
                .subject_tenant_id(u.tenant_id)
                .build()?;
            let created = app_state
                .cp
                .create_upstream(&ctx, u.request.clone())
                .await
                .map_err(|e| {
                    anyhow::anyhow!("Failed to provision upstream (tenant={}): {e}", u.tenant_id)
                })?;
            info!(
                id = %created.id,
                tenant_id = %u.tenant_id,
                alias = %created.alias,
                "Provisioned upstream from types-registry"
            );
        }

        let routes = provisioning.list_routes().await?;
        for r in &routes {
            let ctx = SecurityContext::builder()
                .subject_tenant_id(r.tenant_id)
                .build()?;
            let created = app_state
                .cp
                .create_route(&ctx, r.request.clone())
                .await
                .map_err(|e| {
                    anyhow::anyhow!("Failed to provision route (tenant={}): {e}", r.tenant_id)
                })?;
            info!(
                id = %created.id,
                tenant_id = %r.tenant_id,
                "Provisioned route from types-registry"
            );
        }

        info!(
            upstreams = upstreams.len(),
            routes = routes.len(),
            "Type provisioning complete"
        );

        self.type_provisioning
            .set(provisioning)
            .map_err(|_| anyhow::anyhow!("TypeProvisioningService already set"))?;

        Ok(())
    }
}

impl RestApiCapability for OutboundApiGatewayModule {
    fn register_rest(
        &self,
        _ctx: &ModuleCtx,
        router: axum::Router,
        openapi: &dyn OpenApiRegistry,
    ) -> anyhow::Result<axum::Router> {
        info!("Registering OAGW REST routes");

        let state = self
            .state
            .load()
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("OAGW module not initialized — call init() first"))?
            .as_ref()
            .clone();

        let router = routes::register_routes(router, openapi, state);
        Ok(router)
    }
}
