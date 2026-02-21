//! Prometheus metrics collection for the API Gateway.
//!
//! This module provides HTTP metrics collection when the `prometheus-metrics` feature is enabled.
//! Metrics are exposed at `/metrics` in Prometheus text format.
//!
//! Collected metrics (prefix is `server.app_name` from `AppConfig`, default: `hyperspot`):
//! - `<app_name>_http_request_total`: Total HTTP requests (counter)
//! - `<app_name>_http_requests_duration_seconds`: Request latency (histogram)
//! - `<app_name>_http_requests_pending`: Current in-flight requests (gauge)
//!
//! All metrics include labels: method, endpoint (matched route), and status code.

use axum::routing::get;
use prometheus_axum_middleware::PrometheusAxumLayer;

/// Initializes the Prometheus metrics configuration.
///
/// This must be called before creating the metrics layer.
/// Sets up:
/// - Metric prefix: typically `server.app_name` from `AppConfig` (default: `hyperspot`)
///
/// # Arguments
///
/// * `prefix` - The metric name prefix to use (e.g., "hyperspot")
///
/// Note: Path exclusion is not supported by prometheus-axum-middleware 0.1.1.
/// High-frequency endpoints should be excluded using middleware ordering instead.
pub fn init_metrics(prefix: &str) {
    prometheus_axum_middleware::set_prefix(prefix);
    tracing::info!("Prometheus metrics initialized with prefix: {}", prefix);
}

/// Creates the Prometheus metrics middleware layer.
///
/// This layer collects HTTP request metrics with the configured prefix:
/// - Counter: `<prefix>_http_request_total` (method, endpoint, status)
/// - Histogram: `<prefix>_http_requests_duration_seconds` (method, endpoint, status)
/// - Gauge: `<prefix>_http_requests_pending` (method, endpoint)
///
/// Note: `init_metrics(prefix)` must be called before using this layer to set the prefix.
///
/// # Returns
///
/// The Prometheus metrics layer.
#[must_use]
pub fn create_metrics_layer() -> PrometheusAxumLayer {
    PrometheusAxumLayer::new()
}

/// Handler for the `/metrics` endpoint.
///
/// Returns Prometheus-formatted metrics in text format.
pub fn metrics_endpoint() -> axum::routing::MethodRouter {
    get(prometheus_axum_middleware::render)
}
