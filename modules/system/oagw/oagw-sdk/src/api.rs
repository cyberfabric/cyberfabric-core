use std::pin::Pin;

use bytes::Bytes;
use futures_core::Stream;
use modkit_security::SecurityContext;
use uuid::Uuid;

use crate::error::ServiceGatewayError;
use crate::{
    CreateRouteRequest, CreateUpstreamRequest, ListQuery, Route, UpdateRouteRequest,
    UpdateUpstreamRequest, Upstream,
};

// ---------------------------------------------------------------------------
// Body / Error aliases
// ---------------------------------------------------------------------------

/// Boxed error type for body stream errors.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// A streaming response body.
pub type BodyStream = Pin<Box<dyn Stream<Item = Result<Bytes, BoxError>> + Send>>;

// ---------------------------------------------------------------------------
// Proxy types
// ---------------------------------------------------------------------------

/// Distinguishes gateway-originated errors from upstream-originated errors.
///
/// Available on proxy responses via `resp.extensions().get::<ErrorSource>()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSource {
    Gateway,
    Upstream,
}

// ---------------------------------------------------------------------------
// Service trait
// ---------------------------------------------------------------------------

/// Public API trait for the Outbound API Gateway (Version 1).
///
/// This trait is registered in `ClientHub` by the OAGW module:
/// ```ignore
/// let gw = hub.get::<dyn ServiceGatewayClientV1>()?;
/// ```
#[async_trait::async_trait]
pub trait ServiceGatewayClientV1: Send + Sync {
    // -- Upstream CRUD --

    async fn create_upstream(
        &self,
        ctx: SecurityContext,
        req: CreateUpstreamRequest,
    ) -> Result<Upstream, ServiceGatewayError>;

    async fn get_upstream(
        &self,
        ctx: SecurityContext,
        id: Uuid,
    ) -> Result<Upstream, ServiceGatewayError>;

    async fn list_upstreams(
        &self,
        ctx: SecurityContext,
        query: &ListQuery,
    ) -> Result<Vec<Upstream>, ServiceGatewayError>;

    async fn update_upstream(
        &self,
        ctx: SecurityContext,
        id: Uuid,
        req: UpdateUpstreamRequest,
    ) -> Result<Upstream, ServiceGatewayError>;

    async fn delete_upstream(
        &self,
        ctx: SecurityContext,
        id: Uuid,
    ) -> Result<(), ServiceGatewayError>;

    // -- Route CRUD --

    async fn create_route(
        &self,
        ctx: SecurityContext,
        req: CreateRouteRequest,
    ) -> Result<Route, ServiceGatewayError>;

    async fn get_route(&self, ctx: SecurityContext, id: Uuid)
    -> Result<Route, ServiceGatewayError>;

    async fn list_routes(
        &self,
        ctx: SecurityContext,
        upstream_id: Uuid,
        query: &ListQuery,
    ) -> Result<Vec<Route>, ServiceGatewayError>;

    async fn update_route(
        &self,
        ctx: SecurityContext,
        id: Uuid,
        req: UpdateRouteRequest,
    ) -> Result<Route, ServiceGatewayError>;

    async fn delete_route(&self, ctx: SecurityContext, id: Uuid)
    -> Result<(), ServiceGatewayError>;

    // -- Resolution --

    /// Resolve an upstream by alias. Returns UpstreamDisabled if the upstream exists but is disabled.
    async fn resolve_upstream(
        &self,
        ctx: SecurityContext,
        alias: &str,
    ) -> Result<Upstream, ServiceGatewayError>;

    /// Find the best matching route for the given method and path under an upstream.
    async fn resolve_route(
        &self,
        ctx: SecurityContext,
        upstream_id: Uuid,
        method: &str,
        path: &str,
    ) -> Result<Route, ServiceGatewayError>;

    // -- Proxy --

    /// Execute the full proxy pipeline: resolve -> auth -> rate-limit -> forward -> respond.
    ///
    /// The request URI must follow `/{alias}/{path_suffix}?query` convention.
    /// `ErrorSource` is available on the response via `resp.extensions().get::<ErrorSource>()`.
    async fn proxy_request(
        &self,
        ctx: SecurityContext,
        req: http::Request<Bytes>,
    ) -> Result<http::Response<BodyStream>, ServiceGatewayError>;
}
