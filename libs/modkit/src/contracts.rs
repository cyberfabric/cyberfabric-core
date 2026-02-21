use async_trait::async_trait;
use axum::Router;
use tokio_util::sync::CancellationToken;

pub use crate::api::openapi_registry::OpenApiRegistry;

/// System capability: receives runtime internals before init.
///
/// This trait is internal to modkit and only used by modules with the "system" capability.
/// Normal user modules don't implement this.
#[async_trait]
pub trait SystemCapability: Send + Sync {
    /// Optional pre-init hook for system modules.
    ///
    /// This runs BEFORE `init()` has completed for ALL modules, and only for system modules.
    ///
    /// Default implementation is a no-op so most modules don't need to implement it.
    ///
    /// # Errors
    /// Returns an error if system wiring fails.
    fn pre_init(&self, _sys: &crate::runtime::SystemContext) -> anyhow::Result<()> {
        Ok(())
    }

    /// Optional post-init hook for system modules.
    ///
    /// This runs AFTER `init()` has completed for ALL modules, and only for system modules.
    ///
    /// Default implementation is a no-op so most modules don't need to implement it.
    async fn post_init(&self, _sys: &crate::runtime::SystemContext) -> anyhow::Result<()> {
        Ok(())
    }
}

/// Core module: DI/wiring; do not rely on migrated schema here.
#[async_trait]
pub trait Module: Send + Sync + 'static {
    async fn init(&self, ctx: &crate::context::ModuleCtx) -> anyhow::Result<()>;
}

/// Database capability: modules provide migrations, runtime executes them.
///
/// # Security
///
/// Modules MUST NOT receive raw database connections. They only return migration definitions.
#[cfg(feature = "db")]
pub trait DatabaseCapability: Send + Sync {
    fn migrations(&self) -> Vec<Box<dyn sea_orm_migration::MigrationTrait>>;
}

/// REST API capability: Pure wiring; must be sync. Runs AFTER DB migrations.
pub trait RestApiCapability: Send + Sync {
    /// Register REST routes for this module.
    ///
    /// # Errors
    /// Returns an error if route registration fails.
    fn register_rest(
        &self,
        ctx: &crate::context::ModuleCtx,
        router: Router,
        openapi: &dyn OpenApiRegistry,
    ) -> anyhow::Result<Router>;
}

/// API Gateway capability: handles gateway hosting with prepare/finalize phases.
/// Must be sync. Runs during REST phase, but doesn't start the server.
#[allow(dead_code)]
pub trait ApiGatewayCapability: Send + Sync + 'static {
    /// Prepare a base Router (e.g., global middlewares, /healthz) and optionally touch `OpenAPI` meta.
    /// Do NOT start the server here.
    ///
    /// # Errors
    /// Returns an error if router preparation fails.
    fn rest_prepare(
        &self,
        ctx: &crate::context::ModuleCtx,
        router: Router,
    ) -> anyhow::Result<Router>;

    /// Finalize before start: attach /openapi.json, /docs, persist the Router internally if needed.
    /// Do NOT start the server here.
    ///
    /// # Errors
    /// Returns an error if router finalization fails.
    fn rest_finalize(
        &self,
        ctx: &crate::context::ModuleCtx,
        router: Router,
        module_router: Router,
    ) -> anyhow::Result<Router>;

    // Return OpenAPI registry of the module, e.g., to register endpoints
    fn as_registry(&self) -> &dyn OpenApiRegistry;
}

#[async_trait]
pub trait RunnableCapability: Send + Sync {
    async fn start(&self, cancel: CancellationToken) -> anyhow::Result<()>;
    async fn stop(&self, cancel: CancellationToken) -> anyhow::Result<()>;
}

/// Represents a gRPC service registration callback used by the gRPC hub.
///
/// Each module that exposes gRPC services provides one or more of these.
/// The `register` closure adds the service into the provided `RoutesBuilder`.
#[cfg(feature = "otel")]
pub struct RegisterGrpcServiceFn {
    pub service_name: &'static str,
    pub register: Box<dyn Fn(&mut tonic::service::RoutesBuilder) + Send + Sync>,
}

#[cfg(not(feature = "otel"))]
pub struct RegisterGrpcServiceFn {
    pub service_name: &'static str,
}

/// gRPC Service capability: modules that export gRPC services.
///
/// The runtime will call this during the gRPC registration phase to collect
/// all services that should be exposed on the shared gRPC server.
#[async_trait]
pub trait GrpcServiceCapability: Send + Sync {
    /// Returns all gRPC services this module wants to expose.
    ///
    /// Each installer adds one service to the `tonic::Server` builder.
    async fn get_grpc_services(
        &self,
        ctx: &crate::context::ModuleCtx,
    ) -> anyhow::Result<Vec<RegisterGrpcServiceFn>>;
}

/// gRPC Hub capability: hosts the gRPC server.
///
/// This trait is implemented by the single module responsible for hosting
/// the `tonic::Server` instance. Only one module per process should implement this.
pub trait GrpcHubCapability: Send + Sync {
    /// Returns the bound endpoint after the server starts listening.
    ///
    /// Examples:
    /// - TCP: `http://127.0.0.1:50652`
    /// - Unix socket: `unix:///path/to/socket`
    /// - Named pipe: `pipe://\\.\pipe\name`
    ///
    /// Returns `None` if the server hasn't started listening yet.
    fn bound_endpoint(&self) -> Option<String>;
}
