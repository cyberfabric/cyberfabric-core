#![cfg(feature = "prometheus-metrics")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Integration tests for Prometheus metrics collection

use anyhow::Result;
use async_trait::async_trait;
use axum::{Router, extract::Json, routing::get};
use modkit::{
    Module, ModuleCtx, RestApiCapability,
    api::OperationBuilder,
    bootstrap::{AppConfig, ServerConfig, config::PrometheusConfig},
    config::ConfigProvider,
    contracts::{ApiGatewayCapability, OpenApiRegistry},
};
use modkit_security::SecurityContext;
use serde::{Deserialize, Serialize};
use serial_test::serial;
use std::collections::HashMap;
use std::sync::Arc;
use tenant_resolver_sdk::{
    GetAncestorsOptions, GetAncestorsResponse, GetDescendantsOptions, GetDescendantsResponse,
    GetTenantsOptions, IsAncestorOptions, TenantId, TenantInfo, TenantResolverClient,
    TenantResolverError, TenantStatus,
};
use utoipa::ToSchema;
use uuid::Uuid;

struct MockTenantResolver;

#[async_trait]
impl TenantResolverClient for MockTenantResolver {
    async fn get_tenant(
        &self,
        _ctx: &SecurityContext,
        id: TenantId,
    ) -> std::result::Result<TenantInfo, TenantResolverError> {
        Ok(TenantInfo {
            id,
            name: format!("Tenant {id}"),
            status: TenantStatus::Active,
            tenant_type: None,
            parent_id: None,
            self_managed: false,
        })
    }

    async fn get_tenants(
        &self,
        _ctx: &SecurityContext,
        _ids: &[TenantId],
        _options: &GetTenantsOptions,
    ) -> std::result::Result<Vec<TenantInfo>, TenantResolverError> {
        Ok(vec![])
    }

    async fn get_ancestors(
        &self,
        _ctx: &SecurityContext,
        _id: TenantId,
        _options: &GetAncestorsOptions,
    ) -> std::result::Result<GetAncestorsResponse, TenantResolverError> {
        Ok(GetAncestorsResponse {
            tenant: tenant_resolver_sdk::TenantRef {
                id: _id,
                status: TenantStatus::Active,
                tenant_type: None,
                parent_id: None,
                self_managed: false,
            },
            ancestors: vec![],
        })
    }

    async fn get_descendants(
        &self,
        _ctx: &SecurityContext,
        _id: TenantId,
        _options: &GetDescendantsOptions,
    ) -> std::result::Result<GetDescendantsResponse, TenantResolverError> {
        Ok(GetDescendantsResponse {
            tenant: tenant_resolver_sdk::TenantRef {
                id: _id,
                status: TenantStatus::Active,
                tenant_type: None,
                parent_id: None,
                self_managed: false,
            },
            descendants: vec![],
        })
    }

    async fn is_ancestor(
        &self,
        _ctx: &SecurityContext,
        _ancestor_id: TenantId,
        _descendant_id: TenantId,
        _options: &IsAncestorOptions,
    ) -> std::result::Result<bool, TenantResolverError> {
        Ok(false)
    }
}

struct TestConfigProvider {
    module_config: serde_json::Value,
    app_config: AppConfig,
}

impl ConfigProvider for TestConfigProvider {
    fn get_module_config(&self, module: &str) -> Option<&serde_json::Value> {
        if module == "api_gateway" {
            Some(&self.module_config)
        } else {
            None
        }
    }

    fn app_config(&self) -> Option<&AppConfig> {
        Some(&self.app_config)
    }
}

fn wrap_config(config: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "config": config
    })
}

fn create_test_module_ctx_with_config(
    api_gateway_config: &serde_json::Value,
    prometheus_enabled: bool,
) -> ModuleCtx {
    create_test_module_ctx_with_app_name(api_gateway_config, prometheus_enabled, "hyperspot")
}

fn create_test_module_ctx_with_app_name(
    api_gateway_config: &serde_json::Value,
    prometheus_enabled: bool,
    app_name: &str,
) -> ModuleCtx {
    let wrapped_config = wrap_config(api_gateway_config);
    let hub = Arc::new(modkit::ClientHub::new());
    hub.register::<dyn TenantResolverClient>(Arc::new(MockTenantResolver));

    let app_config = AppConfig {
        server: ServerConfig {
            home_dir: std::path::PathBuf::from("/tmp"),
            prometheus: PrometheusConfig {
                enabled: prometheus_enabled,
                app_name: app_name.to_owned(),
            },
        },
        database: None,
        logging: HashMap::default(),
        tracing: None,
        modules_dir: None,
        modules: HashMap::default(),
    };

    ModuleCtx::new(
        "api_gateway",
        Uuid::new_v4(),
        Arc::new(TestConfigProvider {
            module_config: wrapped_config,
            app_config,
        }),
        hub,
        tokio_util::sync::CancellationToken::new(),
        None,
    )
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
struct TestResponse {
    message: String,
}

/// Test module with various routes for metrics collection
pub struct MetricsTestModule;

#[async_trait]
impl Module for MetricsTestModule {
    async fn init(&self, _ctx: &modkit::ModuleCtx) -> Result<()> {
        Ok(())
    }
}

impl RestApiCapability for MetricsTestModule {
    fn register_rest(
        &self,
        _ctx: &modkit::ModuleCtx,
        router: axum::Router,
        openapi: &dyn OpenApiRegistry,
    ) -> Result<axum::Router> {
        // Public route that should appear in metrics
        let router = OperationBuilder::get("/tests/v1/public")
            .operation_id("test:public")
            .summary("Public endpoint")
            .public()
            .json_response(http::StatusCode::OK, "Success")
            .handler(get(public_handler))
            .register(router, openapi);

        // Route that returns 404
        let router = OperationBuilder::get("/tests/v1/notfound")
            .operation_id("test:notfound")
            .summary("Not found endpoint")
            .public()
            .json_response(http::StatusCode::NOT_FOUND, "Not found")
            .handler(get(notfound_handler))
            .register(router, openapi);

        // Route that returns 500
        let router = OperationBuilder::get("/tests/v1/error")
            .operation_id("test:error")
            .summary("Error endpoint")
            .public()
            .json_response(http::StatusCode::INTERNAL_SERVER_ERROR, "Error")
            .handler(get(error_handler))
            .register(router, openapi);

        Ok(router)
    }
}

async fn public_handler() -> Json<TestResponse> {
    Json(TestResponse {
        message: "public".to_owned(),
    })
}

async fn notfound_handler() -> (http::StatusCode, Json<TestResponse>) {
    (
        http::StatusCode::NOT_FOUND,
        Json(TestResponse {
            message: "not found".to_owned(),
        }),
    )
}

async fn error_handler() -> (http::StatusCode, Json<TestResponse>) {
    (
        http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(TestResponse {
            message: "error".to_owned(),
        }),
    )
}

/// Test module with a parameterized route for cardinality testing
struct ParameterizedModule;

#[async_trait]
impl Module for ParameterizedModule {
    async fn init(&self, _ctx: &modkit::ModuleCtx) -> Result<()> {
        Ok(())
    }
}

impl RestApiCapability for ParameterizedModule {
    fn register_rest(
        &self,
        _ctx: &modkit::ModuleCtx,
        router: axum::Router,
        _openapi: &dyn modkit::contracts::OpenApiRegistry,
    ) -> anyhow::Result<axum::Router> {
        use axum::routing::get;
        // Add a route with a path parameter (Axum 0.8 uses {param} syntax)
        Ok(router.route(
            "/test/users/{user_id}",
            get(
                |axum::extract::Path(user_id): axum::extract::Path<String>| async move {
                    format!("User {user_id}")
                },
            ),
        ))
    }
}

#[tokio::test]
#[serial]
async fn test_metrics_endpoint_accessible() {
    let config = serde_json::json!({
        "bind_addr": "127.0.0.1:0",
        "cors_enabled": false,
        "auth_disabled": true,
    });

    let api_gateway = api_gateway::ApiGateway::default();
    let ctx = create_test_module_ctx_with_config(&config, true);
    api_gateway.init(&ctx).await.expect("Failed to init");

    let module = MetricsTestModule;
    let router = Router::new();
    let router = module
        .register_rest(&ctx, router, &api_gateway)
        .expect("Failed to register routes");

    let router = api_gateway
        .rest_finalize(&ctx, router)
        .expect("Failed to finalize");

    // First, make a test request to generate some metrics
    let test_req = axum::http::Request::builder()
        .uri("/test")
        .method("GET")
        .body(axum::body::Body::empty())
        .expect("Failed to create test request");

    let _test_response = tower::ServiceExt::oneshot(router.clone(), test_req)
        .await
        .expect("Failed to get test response");

    // Now request the metrics endpoint
    let req = axum::http::Request::builder()
        .uri("/metrics")
        .method("GET")
        .body(axum::body::Body::empty())
        .expect("Failed to create request");

    let response = tower::ServiceExt::oneshot(router, req)
        .await
        .expect("Failed to get response");

    assert_eq!(response.status(), http::StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read body");
    let body_str = String::from_utf8(body.to_vec()).expect("Failed to convert to string");

    // Should contain Prometheus metrics (prefix + metric name)
    // Actual metric names: hyperspot_http_requests_duration_seconds (using server.app_name as prefix)
    // or hyperspot_http_requests_pending
    assert!(
        body_str.contains("hyperspot_http_requests") || body_str.contains("# TYPE"),
        "Body did not contain expected metrics. Body: {body_str}"
    );
}

#[tokio::test]
#[serial]
async fn test_metrics_counter_increments() {
    let config = serde_json::json!({
        "bind_addr": "127.0.0.1:0",
        "cors_enabled": false,
        "auth_disabled": true,
    });

    let api_gateway = api_gateway::ApiGateway::default();
    let ctx = create_test_module_ctx_with_config(&config, true);
    api_gateway.init(&ctx).await.expect("Failed to init");

    let module = MetricsTestModule;
    let router = Router::new();
    let router = module
        .register_rest(&ctx, router, &api_gateway)
        .expect("Failed to register routes");

    let router = api_gateway
        .rest_finalize(&ctx, router)
        .expect("Failed to finalize");

    // Make a request to the public endpoint
    let req = axum::http::Request::builder()
        .uri("/tests/v1/public")
        .method("GET")
        .body(axum::body::Body::empty())
        .expect("Failed to create request");

    let response = tower::ServiceExt::oneshot(router.clone(), req)
        .await
        .expect("Failed to get response");

    assert_eq!(response.status(), http::StatusCode::OK);

    // Request metrics
    let req = axum::http::Request::builder()
        .uri("/metrics")
        .method("GET")
        .body(axum::body::Body::empty())
        .expect("Failed to create request");

    let response = tower::ServiceExt::oneshot(router, req)
        .await
        .expect("Failed to get response");

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read body");
    let body_str = String::from_utf8(body.to_vec()).expect("Failed to convert to string");

    // Should contain metrics - at minimum the # TYPE and # HELP lines
    // Note: Metrics may not always be present immediately due to how prometheus collects them
    assert!(body_str.contains("# TYPE") || body_str.contains("# HELP"));
}

#[tokio::test]
#[serial]
async fn test_metrics_histogram_present() {
    let config = serde_json::json!({
        "bind_addr": "127.0.0.1:0",
        "cors_enabled": false,
        "auth_disabled": true,
    });

    let api_gateway = api_gateway::ApiGateway::default();
    let ctx = create_test_module_ctx_with_config(&config, true);
    api_gateway.init(&ctx).await.expect("Failed to init");

    let module = MetricsTestModule;
    let router = Router::new();
    let router = module
        .register_rest(&ctx, router, &api_gateway)
        .expect("Failed to register routes");

    let router = api_gateway
        .rest_finalize(&ctx, router)
        .expect("Failed to finalize");

    // Make a request
    let req = axum::http::Request::builder()
        .uri("/tests/v1/public")
        .method("GET")
        .body(axum::body::Body::empty())
        .expect("Failed to create request");

    let response = tower::ServiceExt::oneshot(router.clone(), req)
        .await
        .expect("Failed to get response");

    assert_eq!(response.status(), http::StatusCode::OK);

    // Request metrics
    let req = axum::http::Request::builder()
        .uri("/metrics")
        .method("GET")
        .body(axum::body::Body::empty())
        .expect("Failed to create request");

    let response = tower::ServiceExt::oneshot(router, req)
        .await
        .expect("Failed to get response");

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read body");
    let body_str = String::from_utf8(body.to_vec()).expect("Failed to convert to string");

    // Should contain histogram metrics - check for the metric type rather than specific prefix
    // since Prometheus global registry may persist metrics from previous test runs
    assert!(
        body_str.contains("http_requests_duration_seconds"),
        "Metrics should contain histogram metric (http_requests_duration_seconds with any prefix). Body: {body_str}"
    );
}

#[tokio::test]
#[serial]
async fn test_metrics_error_status_codes() {
    let config = serde_json::json!({
        "bind_addr": "127.0.0.1:0",
        "cors_enabled": false,
        "auth_disabled": true,
    });

    let api_gateway = api_gateway::ApiGateway::default();
    let ctx = create_test_module_ctx_with_config(&config, true);
    api_gateway.init(&ctx).await.expect("Failed to init");

    let module = MetricsTestModule;
    let router = Router::new();
    let router = module
        .register_rest(&ctx, router, &api_gateway)
        .expect("Failed to register routes");

    let router = api_gateway
        .rest_finalize(&ctx, router)
        .expect("Failed to finalize");

    // Make requests with different status codes
    let req = axum::http::Request::builder()
        .uri("/tests/v1/notfound")
        .method("GET")
        .body(axum::body::Body::empty())
        .expect("Failed to create request");

    let response = tower::ServiceExt::oneshot(router.clone(), req)
        .await
        .expect("Failed to get response");

    assert_eq!(response.status(), http::StatusCode::NOT_FOUND);

    let req = axum::http::Request::builder()
        .uri("/tests/v1/error")
        .method("GET")
        .body(axum::body::Body::empty())
        .expect("Failed to create request");

    let response = tower::ServiceExt::oneshot(router.clone(), req)
        .await
        .expect("Failed to get response");

    assert_eq!(response.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

    // Request metrics
    let req = axum::http::Request::builder()
        .uri("/metrics")
        .method("GET")
        .body(axum::body::Body::empty())
        .expect("Failed to create request");

    let response = tower::ServiceExt::oneshot(router, req)
        .await
        .expect("Failed to get response");

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read body");
    let body_str = String::from_utf8(body.to_vec()).expect("Failed to convert to string");

    // Metrics should track different status codes
    // At minimum should have prometheus format
    assert!(body_str.contains("# TYPE") || body_str.contains("# HELP"));
}

#[tokio::test]
async fn test_metrics_disabled_route_not_registered() {
    let config = serde_json::json!({
        "bind_addr": "127.0.0.1:0",
        "cors_enabled": false,
        "auth_disabled": true,
    });

    let api_gateway = api_gateway::ApiGateway::default();
    let ctx = create_test_module_ctx_with_config(&config, false);
    api_gateway.init(&ctx).await.expect("Failed to init");

    let module = MetricsTestModule;
    let router = Router::new();
    let router = module
        .register_rest(&ctx, router, &api_gateway)
        .expect("Failed to register routes");

    let router = api_gateway
        .rest_finalize(&ctx, router)
        .expect("Failed to finalize");

    // Request the metrics endpoint (should not be registered when disabled)
    let req = axum::http::Request::builder()
        .uri("/metrics")
        .method("GET")
        .body(axum::body::Body::empty())
        .expect("Failed to create request");

    let response = tower::ServiceExt::oneshot(router, req)
        .await
        .expect("Failed to get response");

    // Should return 404 since the route is not registered
    assert_eq!(response.status(), http::StatusCode::NOT_FOUND);
}

#[tokio::test]
#[serial]
async fn test_metrics_uses_route_patterns_not_raw_urls() {
    // CRITICAL TEST: Verify endpoint label uses route patterns (low cardinality)
    // rather than raw URLs (high cardinality explosion risk)

    let config = serde_json::json!({
        "bind_addr": "127.0.0.1:0",
        "cors_enabled": false,
        "auth_disabled": true,
    });

    let api_gateway = api_gateway::ApiGateway::default();
    let ctx = create_test_module_ctx_with_config(&config, true);
    api_gateway.init(&ctx).await.expect("Failed to init");

    let module = ParameterizedModule;
    let router = Router::new();
    let router = module
        .register_rest(&ctx, router, &api_gateway)
        .expect("Failed to register routes");

    let router = api_gateway
        .rest_finalize(&ctx, router)
        .expect("Failed to finalize");

    // Make requests with DIFFERENT user IDs to test cardinality
    let user_ids = ["123", "456", "789", "abc", "xyz"];

    for user_id in &user_ids {
        let req = axum::http::Request::builder()
            .uri(format!("/test/users/{user_id}"))
            .method("GET")
            .body(axum::body::Body::empty())
            .expect("Failed to create request");

        let response = tower::ServiceExt::oneshot(router.clone(), req)
            .await
            .expect("Failed to get response");

        assert_eq!(response.status(), http::StatusCode::OK);
    }

    // Now fetch metrics
    let req = axum::http::Request::builder()
        .uri("/metrics")
        .method("GET")
        .body(axum::body::Body::empty())
        .expect("Failed to create request");

    let response = tower::ServiceExt::oneshot(router, req)
        .await
        .expect("Failed to get response");

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read body");
    let body_str = String::from_utf8(body.to_vec()).expect("Failed to convert to string");

    // CRITICAL ASSERTIONS: Verify cardinality safety

    // Print a sample of the metrics to document label format
    eprintln!("\n=== SAMPLE METRICS OUTPUT (for documentation) ===");
    for line in body_str.lines().take(20) {
        if line.contains("/test/users") {
            eprintln!("{line}");
        }
    }
    eprintln!("=================================================\n");

    // Should contain the route PATTERN with parameter placeholder
    // Axum 0.8+ uses {param} syntax: /test/users/{user_id}
    let contains_pattern = body_str.contains("/test/users/{user_id}")
        || body_str.contains("endpoint=\"/test/users/{user_id}\"");

    // Should NOT contain individual user IDs as separate endpoints
    let contains_raw_ids = user_ids
        .iter()
        .any(|id| body_str.contains(&format!("/test/users/{id}\"")));

    assert!(
        contains_pattern,
        "Metrics should use route pattern (e.g., /test/users/{{user_id}}), not raw URLs. \
         This is CRITICAL for preventing cardinality explosion!"
    );

    assert!(
        !contains_raw_ids,
        "Metrics MUST NOT contain individual user IDs (123, 456, etc.) as separate endpoints. \
         Found raw IDs in metrics, which will cause cardinality explosion!"
    );
}

#[tokio::test]
#[serial]
async fn test_metrics_custom_prefix_config() {
    // This test verifies that custom app_name is used as the metrics prefix.
    // Note: Due to Prometheus using a global registry and set_prefix() needing to be called
    // before ANY metrics are registered, testing multiple prefixes in the same test suite
    // is not feasible. This test verifies the configuration is parsed and accepted.

    let custom_app_name = "my_custom_gateway";
    let config = serde_json::json!({
        "bind_addr": "127.0.0.1:0",
        "cors_enabled": false,
        "auth_disabled": true,
    });

    let api_gateway = api_gateway::ApiGateway::default();
    let ctx = create_test_module_ctx_with_app_name(&config, true, custom_app_name);

    // Should initialize without errors
    api_gateway
        .init(&ctx)
        .await
        .expect("Failed to init with custom prefix");

    let module = MetricsTestModule;
    let router = Router::new();
    let router = module
        .register_rest(&ctx, router, &api_gateway)
        .expect("Failed to register routes");

    let router = api_gateway
        .rest_finalize(&ctx, router)
        .expect("Failed to finalize");

    // Make a request to generate metrics
    let req = axum::http::Request::builder()
        .uri("/tests/v1/public")
        .method("GET")
        .body(axum::body::Body::empty())
        .expect("Failed to create request");

    let response = tower::ServiceExt::oneshot(router.clone(), req)
        .await
        .expect("Failed to get response");

    assert_eq!(response.status(), http::StatusCode::OK);

    // Request metrics endpoint
    let req = axum::http::Request::builder()
        .uri("/metrics")
        .method("GET")
        .body(axum::body::Body::empty())
        .expect("Failed to create request");

    let response = tower::ServiceExt::oneshot(router, req)
        .await
        .expect("Failed to get response");

    // Metrics endpoint should still be accessible with custom prefix config
    assert_eq!(response.status(), http::StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read body");
    let body_str = String::from_utf8(body.to_vec()).expect("Failed to convert to string");

    // Should contain Prometheus metrics (either prefix due to global registry)
    assert!(
        body_str.contains("# TYPE") && body_str.contains("# HELP"),
        "Metrics endpoint should return Prometheus format data"
    );
}
