#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Comprehensive tests for the #[module] macro with the new registry/builder

use anyhow::Result;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use modkit::{
    ModuleCtx,
    config::ConfigProvider,
    contracts::{
        ApiGatewayCapability, DatabaseCapability, Module, OpenApiRegistry, RestApiCapability,
        RunnableCapability,
    },
    module,
};
use std::sync::Arc;

// Helper for tests
struct EmptyConfigProvider;
impl ConfigProvider for EmptyConfigProvider {
    fn get_module_config(&self, _module_name: &str) -> Option<&serde_json::Value> {
        None
    }
}

fn test_module_ctx(cancel: tokio_util::sync::CancellationToken) -> ModuleCtx {
    ModuleCtx::new(
        "test",
        Uuid::new_v4(),
        Arc::new(EmptyConfigProvider),
        Arc::new(modkit::client_hub::ClientHub::default()),
        cancel,
        None,
    )
}

/// Minimal `OpenAPI` registry mock
#[derive(Default)]
struct TestOpenApiRegistry;
impl OpenApiRegistry for TestOpenApiRegistry {
    fn register_operation(&self, _spec: &modkit::api::OperationSpec) {}
    fn ensure_schema_raw(
        &self,
        root_name: &str,
        _schemas: Vec<(
            String,
            utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>,
        )>,
    ) -> String {
        root_name.to_owned()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ---------- Test modules (must be at module scope for `inventory`) ----------

#[derive(Default)]
#[module(name = "basic")]
struct BasicModule;

#[async_trait]
impl Module for BasicModule {
    async fn init(&self, _ctx: &modkit::context::ModuleCtx) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
#[module(name = "full-featured", capabilities = [db, rest, stateful])]
struct FullFeaturedModule;

#[async_trait]
impl Module for FullFeaturedModule {
    async fn init(&self, _ctx: &modkit::context::ModuleCtx) -> Result<()> {
        Ok(())
    }
}
impl DatabaseCapability for FullFeaturedModule {
    fn migrations(&self) -> Vec<Box<dyn sea_orm_migration::MigrationTrait>> {
        vec![]
    }
}
impl RestApiCapability for FullFeaturedModule {
    fn register_rest(
        &self,
        _ctx: &modkit::context::ModuleCtx,
        router: axum::Router,
        _openapi: &dyn OpenApiRegistry,
    ) -> Result<axum::Router> {
        Ok(router)
    }
}
#[async_trait]
impl RunnableCapability for FullFeaturedModule {
    async fn start(&self, _t: CancellationToken) -> Result<()> {
        Ok(())
    }
    async fn stop(&self, _t: CancellationToken) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
#[module(name = "dependent", deps = ["basic", "full-featured"])]
struct DependentModule;

#[async_trait]
impl Module for DependentModule {
    async fn init(&self, _ctx: &modkit::context::ModuleCtx) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
#[module(name = "custom-ctor", ctor = CustomCtorModule::create())]
struct CustomCtorModule {
    value: i32,
}

impl CustomCtorModule {
    fn create() -> Self {
        Self { value: 42 }
    }
}

#[async_trait]
impl Module for CustomCtorModule {
    async fn init(&self, _ctx: &modkit::context::ModuleCtx) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
#[module(name = "db-only", capabilities = [db])]
struct DbOnlyModule;
#[async_trait]
impl Module for DbOnlyModule {
    async fn init(&self, _ctx: &modkit::context::ModuleCtx) -> Result<()> {
        Ok(())
    }
}
impl DatabaseCapability for DbOnlyModule {
    fn migrations(&self) -> Vec<Box<dyn sea_orm_migration::MigrationTrait>> {
        vec![]
    }
}

#[derive(Default)]
#[module(name = "rest-only", capabilities = [rest])]
struct RestOnlyModule;
#[async_trait]
impl Module for RestOnlyModule {
    async fn init(&self, _ctx: &modkit::context::ModuleCtx) -> Result<()> {
        Ok(())
    }
}
impl RestApiCapability for RestOnlyModule {
    fn register_rest(
        &self,
        _ctx: &modkit::context::ModuleCtx,
        router: axum::Router,
        _openapi: &dyn OpenApiRegistry,
    ) -> Result<axum::Router> {
        Ok(router)
    }
}

#[derive(Default)]
#[module(name = "rest-host", capabilities = [rest_host])]
struct TestApiGatewayModule {
    registry: TestOpenApiRegistry,
}

#[async_trait]
impl Module for TestApiGatewayModule {
    async fn init(&self, _ctx: &modkit::context::ModuleCtx) -> Result<()> {
        Ok(())
    }
}

impl ApiGatewayCapability for TestApiGatewayModule {
    fn rest_prepare(
        &self,
        _ctx: &modkit::context::ModuleCtx,
        router: axum::Router,
    ) -> anyhow::Result<axum::Router> {
        Ok(router)
    }

    fn rest_finalize(
        &self,
        _ctx: &modkit::context::ModuleCtx,
        router: axum::Router,
        _module_router: axum::Router,
    ) -> anyhow::Result<axum::Router> {
        Ok(router)
    }

    fn as_registry(&self) -> &dyn OpenApiRegistry {
        &self.registry
    }
}

#[derive(Default)]
#[module(name = "stateful-only", capabilities = [stateful])]
struct StatefulOnlyModule;
#[async_trait]
impl Module for StatefulOnlyModule {
    async fn init(&self, _ctx: &modkit::context::ModuleCtx) -> Result<()> {
        Ok(())
    }
}
#[async_trait]
impl RunnableCapability for StatefulOnlyModule {
    async fn start(&self, _t: CancellationToken) -> Result<()> {
        Ok(())
    }
    async fn stop(&self, _t: CancellationToken) -> Result<()> {
        Ok(())
    }
}

// ---------- Tests ----------

#[tokio::test]
async fn test_basic_macro_and_init() {
    assert_eq!(BasicModule::MODULE_NAME, "basic");
    let ctx = test_module_ctx(CancellationToken::new());
    BasicModule.init(&ctx).await.unwrap();
}

#[tokio::test]
async fn test_custom_ctor_name_and_value() {
    assert_eq!(CustomCtorModule::MODULE_NAME, "custom-ctor");
    let m = CustomCtorModule::create();
    assert_eq!(m.value, 42);
}

#[tokio::test]
async fn test_full_capabilities() {
    assert_eq!(FullFeaturedModule::MODULE_NAME, "full-featured");

    let ctx = test_module_ctx(CancellationToken::new());
    FullFeaturedModule.init(&ctx).await.unwrap();

    // REST sync phase
    let router = axum::Router::new();
    let oas = TestOpenApiRegistry;
    let _router = FullFeaturedModule
        .register_rest(&ctx, router, &oas)
        .unwrap();

    // Stateful
    let token = CancellationToken::new();
    FullFeaturedModule.start(token.clone()).await.unwrap();
    FullFeaturedModule.stop(token).await.unwrap();
}

#[test]
fn test_capability_trait_markers() {
    fn assert_module<T: Module>(_: &T) {}
    fn assert_db<T: DatabaseCapability>(_: &T) {}
    fn assert_rest<T: RestApiCapability>(_: &T) {}
    fn assert_stateful<T: RunnableCapability>(_: &T) {}

    assert_module(&BasicModule);
    assert_module(&DependentModule);
    assert_module(&CustomCtorModule::default());

    assert_db(&FullFeaturedModule);
    assert_db(&DbOnlyModule);

    assert_rest(&FullFeaturedModule);
    assert_rest(&RestOnlyModule);

    assert_stateful(&FullFeaturedModule);
    assert_stateful(&StatefulOnlyModule);
}
