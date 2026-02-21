//! `CredStore` module.

use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use axum::Router;
use credstore_sdk::{CredStoreClientV1, CredStorePluginSpecV1};
use modkit::api::OpenApiRegistry;
use modkit::{Module, ModuleCtx};
use tracing::info;
use types_registry_sdk::{RegisterResult, TypesRegistryClient};

use crate::api::rest::routes;
use crate::config::CredStoreConfig;
use crate::domain::{CredStoreLocalClient, Service};

/// `CredStore` gateway module.
///
/// This module:
/// 1. Registers the `CredStorePluginSpecV1` schema in types-registry
/// 2. Discovers plugin instances via types-registry (lazy, first-use)
/// 3. Routes secret operations through the selected plugin
/// 4. Implements hierarchical tenant secret resolution via tenant-resolver
/// 5. Registers `Arc<dyn CredStoreClient>` in `ClientHub` for consumers
#[modkit::module(
    name = "credstore",
    deps = ["types-registry", "tenant-resolver"],
    capabilities = [rest]
)]
pub struct CredStoreModule {
    service: OnceLock<Arc<Service>>,
}

impl Default for CredStoreModule {
    fn default() -> Self {
        Self {
            service: OnceLock::new(),
        }
    }
}

#[async_trait]
impl Module for CredStoreModule {
    #[tracing::instrument(skip_all, fields(vendor))]
    async fn init(&self, ctx: &ModuleCtx) -> anyhow::Result<()> {
        let cfg: CredStoreConfig = ctx.config()?;
        tracing::Span::current().record("vendor", cfg.vendor.as_str());
        info!(vendor = %cfg.vendor, "Initializing {} module", Self::MODULE_NAME);

        // Register plugin schema in types-registry
        let registry = ctx.client_hub().get::<dyn TypesRegistryClient>()?;
        let schema_str = CredStorePluginSpecV1::gts_schema_with_refs_as_string();
        let schema_json: serde_json::Value = serde_json::from_str(&schema_str)?;
        let results = registry.register(vec![schema_json]).await?;
        RegisterResult::ensure_all_ok(&results)?;
        info!(
            schema_id = %CredStorePluginSpecV1::gts_schema_id(),
            "Registered CredStore plugin schema in types-registry"
        );

        // Create domain service
        let hub = ctx.client_hub();
        let svc = Arc::new(Service::new(hub, cfg.vendor));
        self.service
            .set(svc.clone())
            .map_err(|_| anyhow::anyhow!("{} module already initialized", Self::MODULE_NAME))?;

        // Register local client in ClientHub
        let api: Arc<dyn CredStoreClientV1> = Arc::new(CredStoreLocalClient::new(svc));
        ctx.client_hub().register::<dyn CredStoreClientV1>(api);

        info!("{} module initialized successfully", Self::MODULE_NAME);

        Ok(())
    }
}

#[async_trait]
impl modkit::contracts::RestApiCapability for CredStoreModule {
    fn register_rest(
        &self,
        _ctx: &ModuleCtx,
        router: Router,
        openapi: &dyn OpenApiRegistry,
    ) -> anyhow::Result<Router> {
        let service = self
            .service
            .get()
            .ok_or_else(|| anyhow::anyhow!("CredStore service not initialized"))?
            .clone();

        let router = routes::register_routes(router, openapi, service);
        info!("CredStore module: REST routes registered");
        Ok(router)
    }
}
