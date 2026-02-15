pub(crate) mod client;
pub(crate) mod management;
pub(crate) mod proxy;

pub(crate) use client::ServiceGatewayClientV1Facade;
pub(crate) use management::ControlPlaneServiceImpl;
pub(crate) use proxy::DataPlaneServiceImpl;

use modkit_security::SecurityContext;
use uuid::Uuid;

use crate::domain::error::DomainError;
use crate::domain::dto::{
    CreateRouteRequest, CreateUpstreamRequest, ListQuery, ProxyContext, ProxyResponse, Route,
    UpdateRouteRequest, UpdateUpstreamRequest, Upstream,
};

/// Internal Control Plane service trait — configuration management and resolution.
#[async_trait::async_trait]
pub(crate) trait ControlPlaneService: Send + Sync {
    // -- Upstream CRUD --

    async fn create_upstream(
        &self,
        ctx: &SecurityContext,
        req: CreateUpstreamRequest,
    ) -> Result<Upstream, DomainError>;

    async fn get_upstream(&self, ctx: &SecurityContext, id: Uuid) -> Result<Upstream, DomainError>;

    async fn list_upstreams(
        &self,
        ctx: &SecurityContext,
        query: &ListQuery,
    ) -> Result<Vec<Upstream>, DomainError>;

    async fn update_upstream(
        &self,
        ctx: &SecurityContext,
        id: Uuid,
        req: UpdateUpstreamRequest,
    ) -> Result<Upstream, DomainError>;

    async fn delete_upstream(&self, ctx: &SecurityContext, id: Uuid) -> Result<(), DomainError>;

    // -- Route CRUD --

    async fn create_route(
        &self,
        ctx: &SecurityContext,
        req: CreateRouteRequest,
    ) -> Result<Route, DomainError>;

    async fn get_route(&self, ctx: &SecurityContext, id: Uuid) -> Result<Route, DomainError>;

    async fn list_routes(
        &self,
        ctx: &SecurityContext,
        upstream_id: Uuid,
        query: &ListQuery,
    ) -> Result<Vec<Route>, DomainError>;

    async fn update_route(
        &self,
        ctx: &SecurityContext,
        id: Uuid,
        req: UpdateRouteRequest,
    ) -> Result<Route, DomainError>;

    async fn delete_route(&self, ctx: &SecurityContext, id: Uuid) -> Result<(), DomainError>;

    // -- Resolution --

    async fn resolve_upstream(&self, ctx: &SecurityContext, alias: &str)
    -> Result<Upstream, DomainError>;

    async fn resolve_route(
        &self,
        ctx: &SecurityContext,
        upstream_id: Uuid,
        method: &str,
        path: &str,
    ) -> Result<Route, DomainError>;
}

/// Internal Data Plane service trait — proxy orchestration and plugin execution.
#[async_trait::async_trait]
pub(crate) trait DataPlaneService: Send + Sync {
    async fn proxy_request(
        &self,
        ctx: ProxyContext,
    ) -> Result<ProxyResponse, DomainError>;
}
