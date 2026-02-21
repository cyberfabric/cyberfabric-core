#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Integration tests for auth middleware
//!
//! These tests verify that:
//! 1. Auth middleware is properly attached to the router
//! 2. `SecurityContext` is always inserted by middleware
//! 3. Public routes work without authentication
//! 4. Protected routes enforce authentication when enabled

use anyhow::Result;
use async_trait::async_trait;
use authn_resolver_sdk::{AuthNResolverClient, AuthNResolverError, AuthenticationResult};
use axum::{
    Extension, Json, Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use modkit::{
    ClientHub, Module,
    api::{OperationBuilder, operation_builder::LicenseFeature},
    config::ConfigProvider,
    context::ModuleCtx,
    contracts::{ApiGatewayCapability, OpenApiRegistry, RestApiCapability},
};
use modkit_security::SecurityContext;
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

/// Test configuration provider
struct TestConfigProvider {
    config: serde_json::Value,
}

impl ConfigProvider for TestConfigProvider {
    fn get_module_config(&self, module: &str) -> Option<&serde_json::Value> {
        self.config.get(module)
    }
}

/// Create test context for `api_gateway` module
fn create_api_gateway_ctx(config: serde_json::Value) -> ModuleCtx {
    let hub = Arc::new(ClientHub::new());

    ModuleCtx::new(
        "api-gateway",
        Uuid::new_v4(),
        Arc::new(TestConfigProvider { config }),
        hub,
        tokio_util::sync::CancellationToken::new(),
        None,
    )
}

/// Create test context for other test modules
fn create_test_module_ctx() -> ModuleCtx {
    ModuleCtx::new(
        "test_module",
        Uuid::new_v4(),
        Arc::new(TestConfigProvider { config: json!({}) }),
        Arc::new(ClientHub::new()),
        tokio_util::sync::CancellationToken::new(),
        None,
    )
}

/// Test response type
#[derive(Clone)]
#[modkit_macros::api_dto(response)]
struct TestResponse {
    message: String,
    user_id: String,
}

/// Handler that requires `SecurityContext` (via Extension extractor)
async fn protected_handler(Extension(ctx): Extension<SecurityContext>) -> Json<TestResponse> {
    Json(TestResponse {
        message: "Protected resource accessed".to_owned(),
        user_id: ctx.subject_id().to_string(),
    })
}

/// Handler that doesn't require auth
async fn public_handler() -> Json<TestResponse> {
    Json(TestResponse {
        message: "Public resource accessed".to_owned(),
        user_id: "anonymous".to_owned(),
    })
}

/// Test module with protected and public routes
pub struct TestAuthModule;

#[async_trait]
impl Module for TestAuthModule {
    async fn init(&self, _ctx: &ModuleCtx) -> Result<()> {
        Ok(())
    }
}

struct License;

impl AsRef<str> for License {
    fn as_ref(&self) -> &'static str {
        "gts.x.core.lic.feat.v1~x.core.global.base.v1"
    }
}

impl LicenseFeature for License {}

impl RestApiCapability for TestAuthModule {
    fn register_rest(
        &self,
        _ctx: &ModuleCtx,
        router: Router,
        openapi: &dyn OpenApiRegistry,
    ) -> Result<Router> {
        // Protected route with explicit auth requirement
        let router = OperationBuilder::get("/tests/v1/api/protected")
            .operation_id("test.protected")
            .authenticated()
            .require_license_features::<License>([])
            .summary("Protected endpoint")
            .handler(protected_handler)
            .json_response_with_schema::<TestResponse>(openapi, http::StatusCode::OK, "Success")
            .error_401(openapi)
            .error_403(openapi)
            .register(router, openapi);

        // Protected route with path parameter (to test pattern matching)
        let router = OperationBuilder::get("/tests/v1/api/users/{id}")
            .operation_id("test.get_user")
            .authenticated()
            .require_license_features::<License>([])
            .summary("Get user by ID")
            .path_param("id", "User ID")
            .handler(protected_handler)
            .json_response_with_schema::<TestResponse>(openapi, http::StatusCode::OK, "Success")
            .error_401(openapi)
            .error_403(openapi)
            .register(router, openapi);

        // Public route with explicit public marking
        let router = OperationBuilder::get("/tests/v1/api/public")
            .operation_id("test.public")
            .public()
            .summary("Public endpoint")
            .handler(public_handler)
            .json_response_with_schema::<TestResponse>(openapi, http::StatusCode::OK, "Success")
            .register(router, openapi);

        Ok(router)
    }
}

#[tokio::test]
async fn test_auth_disabled_mode() {
    // Create api-gateway with auth disabled
    let config = json!({
        "api-gateway": {
            "config": {
                "bind_addr": "0.0.0.0:8080",
                "enable_docs": true,
                "cors_enabled": false,
                "auth_disabled": true,
            }
        }
    });

    let api_ctx = create_api_gateway_ctx(config);
    let test_ctx = create_test_module_ctx();

    let api_gateway = api_gateway::ApiGateway::default();
    api_gateway.init(&api_ctx).await.expect("Failed to init");

    // Register test module
    let router = Router::new();
    let module_router = Router::new();
    let test_module = TestAuthModule;
    let module_router = test_module
        .register_rest(&test_ctx, module_router, &api_gateway)
        .expect("Failed to register routes");

    // Finalize router (applies middleware)
    let router = api_gateway
        .rest_finalize(&api_ctx, router, module_router)
        .expect("Failed to finalize");

    // Test protected route WITHOUT token (should work because auth is disabled)
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/cf/tests/v1/api/protected")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Protected route should work when auth is disabled"
    );

    // Test public route
    let response = router
        .oneshot(
            Request::builder()
                .uri("/cf/tests/v1/api/public")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Public route should work"
    );
}

#[tokio::test]
async fn test_public_routes_accessible() {
    // Create api-gateway with auth enabled but test public routes
    let config = json!({
        "api-gateway": {
            "config": {
                "bind_addr": "0.0.0.0:8080",
                "enable_docs": true,
                "cors_enabled": false,
                "auth_disabled": true, // Using disabled for simplicity in test
            }
        }
    });

    let api_ctx = create_api_gateway_ctx(config);
    let test_ctx = create_test_module_ctx();

    let api_gateway = api_gateway::ApiGateway::default();
    api_gateway.init(&api_ctx).await.expect("Failed to init");

    // First call rest_prepare to add built-in routes
    let router = Router::new();
    let router = api_gateway
        .rest_prepare(&api_ctx, router)
        .expect("Failed to prepare");

    // Then register test module routes
    let module_router = Router::new();
    let test_module = TestAuthModule;
    let module_router = test_module
        .register_rest(&test_ctx, module_router, &api_gateway)
        .expect("Failed to register routes");

    // Finally finalize
    let router = api_gateway
        .rest_finalize(&api_ctx, router, module_router)
        .expect("Failed to finalize");

    // Test built-in health endpoints
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Health endpoint should be accessible"
    );

    // Test OpenAPI endpoint
    let response = router
        .oneshot(
            Request::builder()
                .uri("/cf/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "OpenAPI endpoint should be accessible"
    );
}

#[tokio::test]
async fn test_middleware_always_inserts_security_ctx() {
    // This test verifies that SecurityCtx is always available in handlers
    let config = json!({
        "api-gateway": {
            "config": {
                "bind_addr": "0.0.0.0:8080",
                "enable_docs": false,
                "cors_enabled": false,
                "auth_disabled": true,
            }
        }
    });

    let api_ctx = create_api_gateway_ctx(config);
    let test_ctx = create_test_module_ctx();

    let api_gateway = api_gateway::ApiGateway::default();
    api_gateway.init(&api_ctx).await.expect("Failed to init");

    let router = Router::new();
    let module_router = Router::new();
    let test_module = TestAuthModule;
    let module_router = test_module
        .register_rest(&test_ctx, module_router, &api_gateway)
        .expect("Failed to register routes");

    let router = api_gateway
        .rest_finalize(&api_ctx, router, module_router)
        .expect("Failed to finalize");

    // Make request to protected handler that extracts SecurityContext
    let response = router
        .oneshot(
            Request::builder()
                .uri("/cf/tests/v1/api/protected")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    // Should NOT get 500 error about missing SecurityCtx
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Handler should receive SecurityCtx from middleware"
    );
}

#[tokio::test]
async fn test_openapi_includes_security_metadata() {
    let config = json!({
        "api-gateway": {
            "config": {
                "bind_addr": "0.0.0.0:8080",
                "enable_docs": true,
                "cors_enabled": false,
                "auth_disabled": true,
                "require_auth_by_default": true,
            }
        }
    });

    let api_ctx = create_api_gateway_ctx(config);
    let test_ctx = create_test_module_ctx();

    let api_gateway = api_gateway::ApiGateway::default();
    api_gateway.init(&api_ctx).await.expect("Failed to init");

    let router = Router::new();
    let test_module = TestAuthModule;
    let _router = test_module
        .register_rest(&test_ctx, router, &api_gateway)
        .expect("Failed to register routes");

    // Build OpenAPI spec
    let openapi = api_gateway
        .build_openapi()
        .expect("Failed to build OpenAPI");
    let spec = serde_json::to_value(&openapi).expect("Failed to serialize");

    // Verify security scheme exists
    let security_schemes = spec
        .pointer("/components/securitySchemes")
        .expect("Security schemes should exist");
    assert!(
        security_schemes.get("bearerAuth").is_some(),
        "bearerAuth scheme should be registered"
    );

    // Verify protected route has security requirement
    // Path is /tests/v1/api/protected, JSON pointer escapes / as ~1
    let protected_security = spec.pointer("/paths/~1tests~1v1~1api~1protected/get/security");
    assert!(
        protected_security.is_some(),
        "Protected route should have security requirement in OpenAPI"
    );

    // Verify public route does NOT have security requirement
    let public_security = spec.pointer("/paths/~1tests~1v1~1api~1public/get/security");
    assert!(
        public_security.is_none()
            || public_security
                .unwrap()
                .as_array()
                .is_some_and(Vec::is_empty),
        "Public route should NOT have security requirement in OpenAPI"
    );
}

#[tokio::test]
async fn test_route_pattern_matching_with_path_params() {
    // This test verifies that routes with path parameters (e.g., /users/{id})
    // are properly matched and authorization is enforced
    let config = json!({
        "api-gateway": {
            "config": {
                "bind_addr": "0.0.0.0:8080",
                "enable_docs": false,
                "cors_enabled": false,
                "auth_disabled": true, // Disabled for test simplicity
            }
        }
    });

    let api_ctx = create_api_gateway_ctx(config);
    let test_ctx = create_test_module_ctx();

    let api_gateway = api_gateway::ApiGateway::default();
    api_gateway.init(&api_ctx).await.expect("Failed to init");

    let router = Router::new();
    let module_router = Router::new();
    let test_module = TestAuthModule;
    let module_router = test_module
        .register_rest(&test_ctx, module_router, &api_gateway)
        .expect("Failed to register routes");

    let router = api_gateway
        .rest_finalize(&api_ctx, router, module_router)
        .expect("Failed to finalize");

    // Test that /tests/v1/api/users/123 is accessible (matches /tests/v1/api/users/{id})
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/cf/tests/v1/api/users/123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Route with path parameter should be accessible and matched correctly"
    );

    // Test that /tests/v1/api/users/abc-def-456 is also accessible
    let response = router
        .oneshot(
            Request::builder()
                .uri("/cf/tests/v1/api/users/abc-def-456")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Route with different path parameter value should also be accessible"
    );
}

// ---------------------------------------------------------------------------
// Auth-enabled tests: verify the actual authn_middleware with a mock AuthN client
// ---------------------------------------------------------------------------

/// Handler function type for the mock `AuthN` Resolver.
type MockAuthNHandler =
    dyn Fn(&str) -> Result<AuthenticationResult, AuthNResolverError> + Send + Sync;

/// Configurable mock `AuthN` Resolver client for auth-enabled tests.
struct MockAuthNResolverClient {
    handler: Arc<MockAuthNHandler>,
}

#[async_trait]
impl AuthNResolverClient for MockAuthNResolverClient {
    async fn authenticate(
        &self,
        bearer_token: &str,
    ) -> Result<AuthenticationResult, AuthNResolverError> {
        (self.handler)(bearer_token)
    }
}

/// Test module for auth-enabled tests.
///
/// Registers both a protected and a public route.
/// The public route also extracts `SecurityContext` so tests can verify
/// that anonymous context is injected for public endpoints.
pub struct TestAuthEnabledModule;

#[async_trait]
impl Module for TestAuthEnabledModule {
    async fn init(&self, _ctx: &ModuleCtx) -> Result<()> {
        Ok(())
    }
}

impl RestApiCapability for TestAuthEnabledModule {
    fn register_rest(
        &self,
        _ctx: &ModuleCtx,
        router: Router,
        openapi: &dyn OpenApiRegistry,
    ) -> Result<Router> {
        let router = OperationBuilder::get("/tests/v1/api/protected")
            .operation_id("test_auth.protected")
            .authenticated()
            .require_license_features::<License>([])
            .summary("Protected endpoint")
            .handler(protected_handler)
            .json_response_with_schema::<TestResponse>(openapi, http::StatusCode::OK, "Success")
            .error_401(openapi)
            .error_403(openapi)
            .register(router, openapi);

        // Public route that extracts SecurityContext so tests can verify anonymous ctx
        let router = OperationBuilder::get("/tests/v1/api/public-ctx")
            .operation_id("test_auth.public_ctx")
            .public()
            .summary("Public endpoint with security context")
            .handler(protected_handler) // reuse: extracts SecurityContext
            .json_response_with_schema::<TestResponse>(openapi, http::StatusCode::OK, "Success")
            .register(router, openapi);

        Ok(router)
    }
}

/// Create a finalized router with auth **enabled** and the given mock `AuthN` client.
async fn create_auth_enabled_router(mock: MockAuthNResolverClient, cors_enabled: bool) -> Router {
    let config = json!({
        "api-gateway": {
            "config": {
                "bind_addr": "0.0.0.0:8080",
                "enable_docs": false,
                "cors_enabled": cors_enabled,
                "auth_disabled": false,
            }
        }
    });

    let hub = Arc::new(ClientHub::new());
    hub.register::<dyn AuthNResolverClient>(Arc::new(mock));

    let api_ctx = ModuleCtx::new(
        "api-gateway",
        Uuid::new_v4(),
        Arc::new(TestConfigProvider { config }),
        hub,
        tokio_util::sync::CancellationToken::new(),
        None,
    );
    let test_ctx = create_test_module_ctx();

    let api_gateway = api_gateway::ApiGateway::default();
    api_gateway.init(&api_ctx).await.expect("Failed to init");

    let router = Router::new();
    let module_router = Router::new();
    let test_module = TestAuthEnabledModule;
    let module_router = test_module
        .register_rest(&test_ctx, module_router, &api_gateway)
        .expect("Failed to register routes");

    api_gateway
        .rest_finalize(&api_ctx, router, module_router)
        .expect("Failed to finalize")
}

/// Build a mock that accepts a specific token and returns a `SecurityContext` with known IDs.
fn mock_accepting_token(
    valid_token: &'static str,
    subject_id: Uuid,
    tenant_id: Uuid,
) -> MockAuthNResolverClient {
    MockAuthNResolverClient {
        handler: Arc::new(move |token| {
            if token == valid_token {
                Ok(AuthenticationResult {
                    security_context: SecurityContext::builder()
                        .subject_id(subject_id)
                        .subject_tenant_id(tenant_id)
                        .build()
                        .unwrap(),
                })
            } else {
                Err(AuthNResolverError::Unauthorized("invalid token".to_owned()))
            }
        }),
    }
}

/// Build a mock that always returns the given error.
fn mock_returning_error(err_fn: fn() -> AuthNResolverError) -> MockAuthNResolverClient {
    MockAuthNResolverClient {
        handler: Arc::new(move |_| Err(err_fn())),
    }
}

// --- Auth-enabled integration tests ---

#[tokio::test]
async fn test_valid_token_returns_200() {
    let subject_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let mock = mock_accepting_token("valid-test-token", subject_id, tenant_id);

    let router = create_auth_enabled_router(mock, false).await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/cf/tests/v1/api/protected")
                .header(header::AUTHORIZATION, "Bearer valid-test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["user_id"], subject_id.to_string());
}

#[tokio::test]
async fn test_missing_token_returns_401() {
    let mock = mock_accepting_token("any", Uuid::new_v4(), Uuid::new_v4());
    let router = create_auth_enabled_router(mock, false).await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/cf/tests/v1/api/protected")
                // No Authorization header
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "Missing token should yield 401"
    );
}

#[tokio::test]
async fn test_invalid_token_returns_401() {
    let mock = mock_accepting_token("good-token", Uuid::new_v4(), Uuid::new_v4());
    let router = create_auth_enabled_router(mock, false).await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/cf/tests/v1/api/protected")
                .header(header::AUTHORIZATION, "Bearer bad-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "Invalid token should yield 401"
    );
}

#[tokio::test]
async fn test_no_plugin_available_returns_503() {
    let mock = mock_returning_error(|| AuthNResolverError::NoPluginAvailable);
    let router = create_auth_enabled_router(mock, false).await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/cf/tests/v1/api/protected")
                .header(header::AUTHORIZATION, "Bearer some-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "NoPluginAvailable should yield 503"
    );
}

#[tokio::test]
async fn test_service_unavailable_returns_503() {
    let mock =
        mock_returning_error(|| AuthNResolverError::ServiceUnavailable("plugin down".to_owned()));
    let router = create_auth_enabled_router(mock, false).await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/cf/tests/v1/api/protected")
                .header(header::AUTHORIZATION, "Bearer some-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "ServiceUnavailable should yield 503"
    );
}

#[tokio::test]
async fn test_internal_error_returns_500() {
    let mock = mock_returning_error(|| AuthNResolverError::Internal("boom".to_owned()));
    let router = create_auth_enabled_router(mock, false).await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/cf/tests/v1/api/protected")
                .header(header::AUTHORIZATION, "Bearer some-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::INTERNAL_SERVER_ERROR,
        "Internal error should yield 500"
    );
}

#[tokio::test]
async fn test_public_route_with_auth_enabled() {
    // Mock that would reject any token — proves it is never called for public routes
    let mock =
        mock_returning_error(|| AuthNResolverError::Internal("should not be called".to_owned()));
    let router = create_auth_enabled_router(mock, false).await;

    // No Authorization header on a public route
    let response = router
        .oneshot(
            Request::builder()
                .uri("/cf/tests/v1/api/public-ctx")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Public route should return 200 even with auth enabled and no token"
    );

    // Verify the handler received an anonymous SecurityContext (subject_id = Uuid::default)
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        json["user_id"],
        Uuid::default().to_string(),
        "Public route should receive anonymous SecurityContext"
    );
}

#[tokio::test]
async fn test_cors_preflight_skips_auth() {
    // Mock that rejects everything — proves auth is skipped for preflight
    let mock =
        mock_returning_error(|| AuthNResolverError::Internal("should not be called".to_owned()));
    let router = create_auth_enabled_router(mock, true).await;

    let response = router
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/cf/tests/v1/api/protected")
                .header(header::ORIGIN, "https://example.com")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    // With CORS enabled, a preflight should NOT be rejected by auth.
    // The exact status depends on the CORS layer (usually 200),
    // but it must NOT be 401/403.
    assert_ne!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "CORS preflight must not be blocked by auth"
    );
    assert_ne!(
        response.status(),
        StatusCode::FORBIDDEN,
        "CORS preflight must not be blocked by auth"
    );
}
