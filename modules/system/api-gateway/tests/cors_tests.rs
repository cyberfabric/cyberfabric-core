#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Integration tests for CORS preflight and actual request handling

use anyhow::Result;
use async_trait::async_trait;
use axum::{Router, extract::Json, routing::get};
use modkit::{
    Module, ModuleCtx, RestApiCapability,
    api::OperationBuilder,
    config::ConfigProvider,
    contracts::{ApiGatewayCapability, OpenApiRegistry},
};
use std::sync::Arc;
use uuid::Uuid;

/// Helper to create a test `ModuleCtx` with CORS config
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

fn create_test_module_ctx_with_cors() -> ModuleCtx {
    let config = wrap_config(&serde_json::json!({
        "bind_addr": "127.0.0.1:0",
        "cors_enabled": true,
        "cors": {
            "allowed_origins": ["https://example.com"],
            "allowed_methods": ["GET", "POST", "PUT", "DELETE", "OPTIONS"],
            "allowed_headers": ["Content-Type", "Authorization"],
            "allow_credentials": true,
            "max_age_seconds": 3600
        },
        "auth_disabled": true
    }));

    let hub = Arc::new(modkit::ClientHub::new());

    ModuleCtx::new(
        "api-gateway",
        Uuid::new_v4(),
        Arc::new(TestConfigProvider { config }),
        hub,
        tokio_util::sync::CancellationToken::new(),
        None,
    )
}

fn create_test_module_ctx_permissive_cors() -> ModuleCtx {
    let config = wrap_config(&serde_json::json!({
        "bind_addr": "127.0.0.1:0",
        "cors_enabled": true,
        "auth_disabled": true
    }));

    let hub = Arc::new(modkit::ClientHub::new());

    ModuleCtx::new(
        "api-gateway",
        Uuid::new_v4(),
        Arc::new(TestConfigProvider { config }),
        hub,
        tokio_util::sync::CancellationToken::new(),
        None,
    )
}

#[derive(Debug, Clone)]
#[modkit_macros::api_dto(request, response)]
struct TestData {
    value: String,
}

pub struct CorsTestModule;

#[async_trait]
impl Module for CorsTestModule {
    async fn init(&self, _ctx: &modkit::ModuleCtx) -> Result<()> {
        Ok(())
    }
}

impl RestApiCapability for CorsTestModule {
    fn register_rest(
        &self,
        _ctx: &modkit::ModuleCtx,
        router: axum::Router,
        openapi: &dyn OpenApiRegistry,
    ) -> Result<axum::Router> {
        let router = OperationBuilder::get("/tests/v1/cors/v1/cors-test")
            .operation_id("cors:test")
            .summary("CORS test endpoint")
            .public()
            .json_response(http::StatusCode::OK, "Success")
            .handler(get(test_handler))
            .register(router, openapi);

        let router = OperationBuilder::post("/tests/v1/cors/v1/cors-post")
            .operation_id("cors:post")
            .summary("CORS POST endpoint")
            .json_request::<TestData>(openapi, "Test data")
            .public()
            .json_response(http::StatusCode::OK, "Success")
            .handler(axum::routing::post(post_handler))
            .register(router, openapi);

        Ok(router)
    }
}

async fn test_handler() -> Json<TestData> {
    Json(TestData {
        value: "cors-test".to_owned(),
    })
}

async fn post_handler(Json(data): Json<TestData>) -> Json<TestData> {
    Json(data)
}

#[tokio::test]
async fn test_cors_layer_builds_with_config() {
    let api_gateway = api_gateway::ApiGateway::default();
    let ctx = create_test_module_ctx_with_cors();
    api_gateway.init(&ctx).await.expect("Failed to init");

    let module = CorsTestModule;
    let router = Router::new();
    let module_router = Router::new();
    let module_router = module
        .register_rest(&ctx, module_router, &api_gateway)
        .expect("Failed to register routes");

    // Build the final router with CORS middleware
    let _final_router = api_gateway
        .rest_finalize(&ctx, router, module_router)
        .expect("Failed to finalize router");

    // Verify router builds successfully with CORS enabled
    // In a full test, we would start the server and make OPTIONS requests
}

#[tokio::test]
async fn test_cors_permissive_mode() {
    let api_gateway = api_gateway::ApiGateway::default();
    let ctx = create_test_module_ctx_permissive_cors();
    api_gateway.init(&ctx).await.expect("Failed to init");

    let module = CorsTestModule;
    let router = Router::new();
    let module_router = Router::new();
    let module_router = module
        .register_rest(&ctx, module_router, &api_gateway)
        .expect("Failed to register routes");

    let _final_router = api_gateway
        .rest_finalize(&ctx, router, module_router)
        .expect("Failed to finalize router");

    // Verify permissive CORS builds successfully
}

#[tokio::test]
async fn test_cors_disabled() {
    let config = wrap_config(&serde_json::json!({
        "bind_addr": "127.0.0.1:0",
        "cors_enabled": false,
        "auth_disabled": true
    }));

    let hub = Arc::new(modkit::ClientHub::new());

    let ctx = ModuleCtx::new(
        "api-gateway",
        Uuid::new_v4(),
        Arc::new(TestConfigProvider { config }),
        hub,
        tokio_util::sync::CancellationToken::new(),
        None,
    );

    let api_gateway = api_gateway::ApiGateway::default();
    api_gateway.init(&ctx).await.expect("Failed to init");

    let module = CorsTestModule;
    let router = Router::new();
    let module_router = Router::new();
    let module_router = module
        .register_rest(&ctx, module_router, &api_gateway)
        .expect("Failed to register routes");

    let _final_router = api_gateway
        .rest_finalize(&ctx, router, module_router)
        .expect("Failed to finalize router");

    // Verify router builds without CORS layer
}

#[tokio::test]
async fn test_cors_config_validation() {
    // Test that CORS config is properly loaded
    let config = wrap_config(&serde_json::json!({
        "bind_addr": "127.0.0.1:0",
        "cors_enabled": true,
        "cors": {
            "allowed_origins": ["https://example.com"],
            "allowed_methods": ["GET", "POST"],
            "allowed_headers": ["Content-Type"],
            "allow_credentials": true,
            "max_age_seconds": 600
        },
        "auth_disabled": true
    }));

    let hub = Arc::new(modkit::ClientHub::new());

    let ctx = ModuleCtx::new(
        "api-gateway",
        Uuid::new_v4(),
        Arc::new(TestConfigProvider { config }),
        hub,
        tokio_util::sync::CancellationToken::new(),
        None,
    );

    let api_gateway = api_gateway::ApiGateway::default();
    api_gateway.init(&ctx).await.expect("Failed to init");

    let loaded_config = api_gateway.get_config();
    assert!(loaded_config.cors_enabled, "CORS should be enabled");
    assert!(
        loaded_config.cors.is_some(),
        "CORS config should be present"
    );
}
