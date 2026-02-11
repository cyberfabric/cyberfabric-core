// === PUBLIC API (from SDK) ===
pub use oagw_sdk::{
    error::ServiceGatewayError,
    CreateRouteRequest, CreateUpstreamRequest, Endpoint, Route,
    UpdateRouteRequest, UpdateUpstreamRequest, Upstream,
    api::ServiceGatewayClientV1,
};

// === MODULE DEFINITION ===
pub mod module;
pub use module::OutboundApiGatewayModule;

// === INTERNAL MODULES ===
#[doc(hidden)]
pub mod api;
#[doc(hidden)]
pub mod config;
pub(crate) mod domain;
pub(crate) mod infra;

#[cfg(any(test, feature = "test-utils"))]
pub mod test_support;
