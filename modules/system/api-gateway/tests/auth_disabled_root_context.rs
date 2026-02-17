#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Test that `auth_disabled` mode properly injects default tenant context
use axum::{
    Extension, Router,
    body::Body,
    extract::Request,
    http::{Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use modkit_security::SecurityContext;
use tower::ServiceExt;
use uuid::{Uuid, uuid};

/// Test tenant ID for auth-disabled mode tests
const TEST_DEFAULT_TENANT_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000001");
/// Test subject ID for auth-disabled mode (matches `api_gateway` constant)
const TEST_DEFAULT_SUBJECT_ID: Uuid = uuid!("11111111-0000-0000-0000-000000000001");

/// Test handler that extracts `SecurityContext` and returns its properties as JSON
async fn test_handler(Extension(ctx): Extension<SecurityContext>) -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "tenant_id": ctx.subject_tenant_id(),
        "subject_id": ctx.subject_id()
    }))
}

/// Middleware that simulates `auth_disabled` mode by injecting default tenant context
async fn inject_default_tenant_context(mut req: Request, next: Next) -> Response {
    // This simulates what api_gateway does in auth_disabled mode:
    let ctx = SecurityContext::builder()
        .subject_tenant_id(TEST_DEFAULT_TENANT_ID)
        .subject_id(TEST_DEFAULT_SUBJECT_ID)
        .build()
        .unwrap();

    req.extensions_mut().insert(ctx);
    next.run(req).await
}

#[tokio::test]
async fn test_auth_disabled_injects_default_tenant_context() {
    // Build a router with the auth-disabled middleware
    let app = Router::new()
        .route("/test", get(test_handler))
        .layer(middleware::from_fn(inject_default_tenant_context));

    // Make a request
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Verify response
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        json["tenant_id"],
        serde_json::json!(TEST_DEFAULT_TENANT_ID.to_string()),
        "Should have the default tenant"
    );
    assert_eq!(
        json["subject_id"],
        TEST_DEFAULT_SUBJECT_ID.to_string(),
        "Subject should be TEST_DEFAULT_SUBJECT_ID"
    );
}

#[tokio::test]
async fn test_auth_disabled_uses_default_subject() {
    // Handler that verifies the default subject ID is used
    async fn check_subject(Extension(ctx): Extension<SecurityContext>) -> impl IntoResponse {
        axum::Json(serde_json::json!({
            "subject_id": ctx.subject_id(),
            "is_default_subject": ctx.subject_id() == TEST_DEFAULT_SUBJECT_ID,
        }))
    }

    let app = Router::new()
        .route("/subject", get(check_subject))
        .layer(middleware::from_fn(inject_default_tenant_context));

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/subject")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        json["is_default_subject"], true,
        "Auth-disabled mode should use TEST_DEFAULT_SUBJECT_ID"
    );
}
