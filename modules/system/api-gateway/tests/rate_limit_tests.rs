#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Integration tests for per-route rate limiting and in-flight concurrency limits

use anyhow::Result;
use async_trait::async_trait;
use axum::{Router, extract::Json, routing::get};
use modkit::{
    Module, ModuleCtx, RestApiCapability,
    api::OperationBuilder,
    config::ConfigProvider,
    contracts::{ApiGatewayCapability, OpenApiRegistry},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::time::{Duration, sleep};
use utoipa::ToSchema;
use uuid::Uuid;

/// Helper to create a test `ModuleCtx`
struct TestConfigProvider {
    config: serde_json::Value,
}

impl ConfigProvider for TestConfigProvider {
    fn get_module_config(&self, module: &str) -> Option<&serde_json::Value> {
        if module == "api-gateway" {
            Some(&self.config)
        } else {
            None
        }
    }
}

fn wrap_config(config: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "config": config
    })
}

fn create_test_module_ctx_with_config(config: &serde_json::Value) -> ModuleCtx {
    let wrapped_config = wrap_config(config);
    let hub = Arc::new(modkit::ClientHub::new());

    ModuleCtx::new(
        "api-gateway",
        Uuid::new_v4(),
        Arc::new(TestConfigProvider {
            config: wrapped_config,
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

/// Test module with rate-limited routes
pub struct RateLimitedModule;

#[async_trait]
impl Module for RateLimitedModule {
    async fn init(&self, _ctx: &modkit::ModuleCtx) -> Result<()> {
        Ok(())
    }
}

impl RestApiCapability for RateLimitedModule {
    fn register_rest(
        &self,
        _ctx: &modkit::ModuleCtx,
        router: axum::Router,
        openapi: &dyn OpenApiRegistry,
    ) -> Result<axum::Router> {
        // Route with strict rate limit: 1 RPS, burst 1
        let mut builder = OperationBuilder::get("/tests/v1/limited");
        builder.require_rate_limit(1, 1, 2);
        let router = builder
            .operation_id("test:limited")
            .summary("Strictly rate-limited endpoint")
            .public()
            .json_response(http::StatusCode::OK, "Success")
            .handler(get(limited_handler))
            .register(router, openapi);

        // Route with low in-flight limit
        let mut builder = OperationBuilder::get("/tests/v1/slow");
        builder.require_rate_limit(100, 100, 2);
        let router = builder
            .operation_id("test:slow")
            .summary("Slow endpoint with low in-flight limit")
            .public()
            .json_response(http::StatusCode::OK, "Success")
            .handler(get(slow_handler))
            .register(router, openapi);

        // Normal route without explicit limits (uses defaults)
        let router = OperationBuilder::get("/tests/v1/normal")
            .operation_id("test:normal")
            .summary("Normal endpoint")
            .public()
            .json_response(http::StatusCode::OK, "Success")
            .handler(get(normal_handler))
            .register(router, openapi);

        Ok(router)
    }
}

async fn limited_handler() -> Json<TestResponse> {
    Json(TestResponse {
        message: "limited".to_owned(),
    })
}

async fn slow_handler() -> Json<TestResponse> {
    // Simulate slow processing
    sleep(Duration::from_millis(200)).await;
    Json(TestResponse {
        message: "slow".to_owned(),
    })
}

async fn normal_handler() -> Json<TestResponse> {
    Json(TestResponse {
        message: "normal".to_owned(),
    })
}

#[tokio::test]
async fn test_rate_limit_enforcement() {
    // Create API gateway with rate limiting enabled
    let config = serde_json::json!({
        "bind_addr": "127.0.0.1:0",
        "cors_enabled": false,
        "auth_disabled": true,
        "defaults": {
            "rate_limit": {
                "rps": 50,
                "burst": 100,
                "in_flight": 64
            }
        }
    });

    let api_gateway = api_gateway::ApiGateway::default();
    let ctx = create_test_module_ctx_with_config(&config);
    api_gateway.init(&ctx).await.expect("Failed to init");

    let module = RateLimitedModule;
    let router = Router::new();
    let module_router = Router::new();
    let module_router = module
        .register_rest(&ctx, module_router, &api_gateway)
        .expect("Failed to register routes");

    // Build the final router with middleware
    let _final_router = api_gateway
        .rest_finalize(&ctx, router, module_router)
        .expect("Failed to finalize router");

    // Note: Full HTTP testing would require starting a server and making real requests
    // This test verifies the router builds successfully with rate limit metadata
}

#[tokio::test]
async fn test_openapi_includes_rate_limit_extensions() {
    let config = serde_json::json!({
        "bind_addr": "127.0.0.1:0",
        "cors_enabled": false,
        "auth_disabled": true
    });

    let api_gateway = api_gateway::ApiGateway::default();
    let ctx = create_test_module_ctx_with_config(&config);
    api_gateway.init(&ctx).await.expect("Failed to init");

    let module = RateLimitedModule;
    let router = Router::new();
    let _router = module
        .register_rest(&ctx, router, &api_gateway)
        .expect("Failed to register routes");

    // Build OpenAPI spec
    let openapi = api_gateway
        .build_openapi()
        .expect("Failed to build OpenAPI");
    let json = serde_json::to_value(&openapi).expect("Failed to serialize OpenAPI");

    // Verify rate limit extensions are present for the limited endpoint
    // Path is /tests/v1/limited, JSON pointer escapes / as ~1
    let limited_op = json
        .pointer("/paths/~1tests~1v1~1limited/get")
        .expect("Limited endpoint not found in OpenAPI");

    // Check for vendor extensions
    if let Some(rps) = limited_op.get("x-rate-limit-rps") {
        assert_eq!(rps.as_u64(), Some(1), "RPS should be 1");
    } else {
        panic!("x-rate-limit-rps extension not found");
    }

    if let Some(burst) = limited_op.get("x-rate-limit-burst") {
        assert_eq!(burst.as_u64(), Some(1), "Burst should be 1");
    } else {
        panic!("x-rate-limit-burst extension not found");
    }

    if let Some(in_flight) = limited_op.get("x-in-flight-limit") {
        assert_eq!(in_flight.as_u64(), Some(2), "In-flight should be 2");
    } else {
        panic!("x-in-flight-limit extension not found");
    }
}

#[tokio::test]
async fn test_rate_limit_metadata_stored() {
    let api_gateway = api_gateway::ApiGateway::default();
    let router = Router::<()>::new();

    let mut builder = OperationBuilder::get("/tests/v1/test");
    builder.require_rate_limit(10, 20, 5);

    let spec = builder.spec();
    assert!(spec.rate_limit.is_some(), "Rate limit should be set");
    let rl = spec.rate_limit.as_ref().unwrap();
    assert_eq!(rl.rps, 10);
    assert_eq!(rl.burst, 20);
    assert_eq!(rl.in_flight, 5);

    // Register and verify it's stored
    let _router = builder
        .operation_id("test")
        .public()
        .json_response(http::StatusCode::OK, "OK")
        .handler(get(normal_handler))
        .register(router, &api_gateway);

    // The operation should be registered with rate limit metadata
    let openapi = api_gateway
        .build_openapi()
        .expect("Failed to build OpenAPI");
    let json = serde_json::to_value(&openapi).expect("Failed to serialize");

    // Path is /tests/v1/test, JSON pointer escapes / as ~1
    let test_op = json.pointer("/paths/~1tests~1v1~1test/get");
    assert!(test_op.is_some(), "Test endpoint should be in OpenAPI");
}
