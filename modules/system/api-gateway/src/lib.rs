#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! API Gateway Module
//!
//! Main API Gateway module — owns the HTTP server (`rest_host`) and collects
//! typed operation specs to emit a single `OpenAPI` document.

// === MODULE DEFINITION ===
pub mod module;
pub use module::ApiGateway;

// === INTERNAL MODULES ===
mod assets;
mod config;
mod cors;
pub mod error;
pub mod middleware;
mod router_cache;
mod web;

#[cfg(feature = "prometheus-metrics")]
pub mod metrics;

// === RE-EXPORTS ===
pub use config::{ApiGatewayConfig, CorsConfig};
