use axum::body::Body;
use http::{Method, Request, StatusCode};
use modkit::client_hub::ClientHub;
use modkit_security::SecurityContext;
use oagw::api::rest::routes::test_router;
use oagw::test_support::{
    APIKEY_AUTH_PLUGIN_ID, TestCpBuilder, TestDpBuilder, build_test_app_state, parse_resource_gts,
};

use oagw::test_support::MockUpstream;
use tower::ServiceExt;
use uuid::Uuid;

fn tenant_id() -> Uuid {
    Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap()
}

fn test_security_context() -> SecurityContext {
    SecurityContext::builder()
        .tenant_id(tenant_id())
        .subject_id(Uuid::new_v4())
        .build()
}

struct E2EHarness {
    app: axum::Router,
    _mock: MockUpstream,
    mock_port: u16,
}

async fn setup_e2e() -> E2EHarness {
    let mock = MockUpstream::start().await;
    let mock_port = mock.addr().port();

    let hub = ClientHub::new();
    let h = build_test_app_state(
        &hub,
        TestCpBuilder::new()
            .with_credentials(vec![("cred://openai-key".into(), "sk-e2e-test-key".into())]),
        TestDpBuilder::new(),
    );

    let app = test_router(h.state, test_security_context());

    E2EHarness {
        app,
        _mock: mock,
        mock_port,
    }
}

async fn body_json(body: Body) -> serde_json::Value {
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn body_string(body: Body) -> String {
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn create_upstream_json(
    port: u16,
    alias: &str,
    auth: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut json = serde_json::json!({
        "server": {
            "endpoints": [{"host": "127.0.0.1", "port": port, "scheme": "http"}]
        },
        "protocol": "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
        "alias": alias,
        "enabled": true,
        "tags": []
    });
    if let Some(auth_config) = auth {
        json["auth"] = auth_config;
    }
    json
}

fn create_route_json(upstream_id: &str, methods: &[&str], path: &str) -> serde_json::Value {
    // Extract UUID from GTS id.
    let (_, uuid) = parse_resource_gts(upstream_id).unwrap();
    serde_json::json!({
        "upstream_id": uuid,
        "match": {
            "http": {
                "methods": methods,
                "path": path
            }
        },
        "enabled": true,
        "tags": [],
        "priority": 0
    })
}

/// Helper to make a request against the app.
async fn send(
    app: axum::Router,
    method: Method,
    uri: &str,
    body: Option<serde_json::Value>,
) -> http::Response<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri);

    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }

    let req_body = match body {
        Some(v) => Body::from(serde_json::to_vec(&v).unwrap()),
        None => Body::empty(),
    };

    app.oneshot(builder.body(req_body).unwrap()).await.unwrap()
}

// 10.1: E2E — create upstream, create route, proxy chat completion, verify round-trip.
#[tokio::test]
async fn e2e_chat_completion_round_trip() {
    let h = setup_e2e().await;

    // Create upstream via Management API.
    let resp = send(
        h.app.clone(),
        Method::POST,
        "/oagw/v1/upstreams",
        Some(create_upstream_json(h.mock_port, "e2e-openai", None)),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let upstream = body_json(resp.into_body()).await;
    let upstream_id = upstream["id"].as_str().unwrap();

    // Create route via Management API.
    let resp = send(
        h.app.clone(),
        Method::POST,
        "/oagw/v1/routes",
        Some(create_route_json(
            upstream_id,
            &["POST"],
            "/v1/chat/completions",
        )),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Proxy a chat completion request.
    let chat_body = serde_json::json!({
        "model": "gpt-4",
        "messages": [{"role": "user", "content": "Hello"}]
    });
    let resp = send(
        h.app.clone(),
        Method::POST,
        "/oagw/v1/proxy/e2e-openai/v1/chat/completions",
        Some(chat_body),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp.into_body()).await;
    assert!(body.get("id").is_some());
    assert!(body.get("choices").is_some());
}

// 10.2: E2E — SSE streaming round-trip.
#[tokio::test]
async fn e2e_sse_streaming() {
    let h = setup_e2e().await;

    // Create upstream + route.
    let resp = send(
        h.app.clone(),
        Method::POST,
        "/oagw/v1/upstreams",
        Some(create_upstream_json(h.mock_port, "e2e-sse", None)),
    )
    .await;
    let upstream = body_json(resp.into_body()).await;
    let uid = upstream["id"].as_str().unwrap();

    let resp = send(
        h.app.clone(),
        Method::POST,
        "/oagw/v1/routes",
        Some(create_route_json(
            uid,
            &["POST"],
            "/v1/chat/completions/stream",
        )),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Proxy streaming request.
    let resp = send(
        h.app.clone(),
        Method::POST,
        "/oagw/v1/proxy/e2e-sse/v1/chat/completions/stream",
        Some(serde_json::json!({"model": "gpt-4", "stream": true})),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(ct.contains("text/event-stream"), "content-type: {ct}");
    let body = body_string(resp.into_body()).await;
    assert!(body.contains("data: [DONE]"));
}

// 10.3: E2E — auth injection round-trip.
#[tokio::test]
async fn e2e_auth_injection() {
    let h = setup_e2e().await;

    let auth_config = serde_json::json!({
        "type": APIKEY_AUTH_PLUGIN_ID,
        "sharing": "private",
        "config": {
            "header": "authorization",
            "prefix": "Bearer ",
            "secret_ref": "cred://openai-key"
        }
    });

    // Create upstream with auth.
    let resp = send(
        h.app.clone(),
        Method::POST,
        "/oagw/v1/upstreams",
        Some(create_upstream_json(
            h.mock_port,
            "e2e-auth",
            Some(auth_config),
        )),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let upstream = body_json(resp.into_body()).await;
    let uid = upstream["id"].as_str().unwrap();

    // Create route for echo endpoint (returns request headers in response).
    let resp = send(
        h.app.clone(),
        Method::POST,
        "/oagw/v1/routes",
        Some(create_route_json(uid, &["POST"], "/echo")),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Proxy to echo and verify auth header.
    let resp = send(
        h.app.clone(),
        Method::POST,
        "/oagw/v1/proxy/e2e-auth/echo",
        Some(serde_json::json!({"test": true})),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp.into_body()).await;
    let headers = body["headers"].as_object().unwrap();
    let auth = headers.get("authorization").unwrap().as_str().unwrap();
    assert_eq!(auth, "Bearer sk-e2e-test-key");
}

// 10.4: E2E — error scenarios.
#[tokio::test]
async fn e2e_nonexistent_alias_returns_404() {
    let h = setup_e2e().await;

    let resp = send(
        h.app.clone(),
        Method::GET,
        "/oagw/v1/proxy/nonexistent/v1/test",
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let source = resp
        .headers()
        .get("x-oagw-error-source")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(source, "gateway");
}

#[tokio::test]
async fn e2e_disabled_upstream_returns_503() {
    let h = setup_e2e().await;

    // Create disabled upstream.
    let mut upstream_json = create_upstream_json(h.mock_port, "e2e-disabled", None);
    upstream_json["enabled"] = serde_json::json!(false);

    let resp = send(
        h.app.clone(),
        Method::POST,
        "/oagw/v1/upstreams",
        Some(upstream_json),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    let resp = send(
        h.app.clone(),
        Method::GET,
        "/oagw/v1/proxy/e2e-disabled/v1/test",
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn e2e_upstream_500_passthrough() {
    let h = setup_e2e().await;

    let resp = send(
        h.app.clone(),
        Method::POST,
        "/oagw/v1/upstreams",
        Some(create_upstream_json(h.mock_port, "e2e-errors", None)),
    )
    .await;
    let upstream = body_json(resp.into_body()).await;
    let uid = upstream["id"].as_str().unwrap();

    let resp = send(
        h.app.clone(),
        Method::POST,
        "/oagw/v1/routes",
        Some(create_route_json(uid, &["GET"], "/error")),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Proxy to error/500.
    let resp = send(
        h.app.clone(),
        Method::GET,
        "/oagw/v1/proxy/e2e-errors/error/500",
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let source = resp
        .headers()
        .get("x-oagw-error-source")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(source, "upstream");
}

// 10.4: E2E — rate limit exceeded.
#[tokio::test]
async fn e2e_rate_limit_returns_429() {
    let h = setup_e2e().await;

    let mut upstream_json = create_upstream_json(h.mock_port, "e2e-rl", None);
    upstream_json["rate_limit"] = serde_json::json!({
        "algorithm": "token_bucket",
        "sustained": {"rate": 1, "window": "minute"},
        "burst": {"capacity": 1},
        "scope": "tenant",
        "strategy": "reject",
        "cost": 1
    });

    let resp = send(
        h.app.clone(),
        Method::POST,
        "/oagw/v1/upstreams",
        Some(upstream_json),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let upstream = body_json(resp.into_body()).await;
    let uid = upstream["id"].as_str().unwrap();

    let resp = send(
        h.app.clone(),
        Method::POST,
        "/oagw/v1/routes",
        Some(create_route_json(uid, &["GET"], "/v1/models")),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // First request succeeds.
    let resp = send(
        h.app.clone(),
        Method::GET,
        "/oagw/v1/proxy/e2e-rl/v1/models",
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Second request should be rate limited.
    let resp = send(
        h.app.clone(),
        Method::GET,
        "/oagw/v1/proxy/e2e-rl/v1/models",
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

// 10.5: E2E — management lifecycle.
#[tokio::test]
async fn e2e_management_lifecycle() {
    let h = setup_e2e().await;

    // Create.
    let resp = send(
        h.app.clone(),
        Method::POST,
        "/oagw/v1/upstreams",
        Some(create_upstream_json(h.mock_port, "lifecycle", None)),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let upstream = body_json(resp.into_body()).await;
    let uid = upstream["id"].as_str().unwrap().to_string();

    // List (appears).
    let resp = send(
        h.app.clone(),
        Method::GET,
        "/oagw/v1/upstreams",
        None,
    )
    .await;
    let list = body_json(resp.into_body()).await;
    assert!(
        list.as_array()
            .unwrap()
            .iter()
            .any(|u| u["id"].as_str() == Some(uid.as_str()))
    );

    // Update alias.
    let resp = send(
        h.app.clone(),
        Method::PUT,
        &format!("/oagw/v1/upstreams/{uid}"),
        Some(serde_json::json!({"alias": "lifecycle-v2"})),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Get (updated).
    let resp = send(
        h.app.clone(),
        Method::GET,
        &format!("/oagw/v1/upstreams/{uid}"),
        None,
    )
    .await;
    let updated = body_json(resp.into_body()).await;
    assert_eq!(updated["alias"].as_str().unwrap(), "lifecycle-v2");

    // Create route.
    let resp = send(
        h.app.clone(),
        Method::POST,
        "/oagw/v1/routes",
        Some(create_route_json(&uid, &["GET"], "/test")),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Delete upstream (cascades routes).
    let resp = send(
        h.app.clone(),
        Method::DELETE,
        &format!("/oagw/v1/upstreams/{uid}"),
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // List (gone).
    let resp = send(
        h.app.clone(),
        Method::GET,
        "/oagw/v1/upstreams",
        None,
    )
    .await;
    let list = body_json(resp.into_body()).await;
    assert!(
        !list
            .as_array()
            .unwrap()
            .iter()
            .any(|u| u["id"].as_str() == Some(uid.as_str()))
    );
}

// 8.11: Content-Length with non-integer value returns 400.
#[tokio::test]
async fn e2e_invalid_content_length_returns_400() {
    let h = setup_e2e().await;

    // Create upstream + route so proxy path is valid.
    let resp = send(
        h.app.clone(),
        Method::POST,
        "/oagw/v1/upstreams",
        Some(create_upstream_json(h.mock_port, "e2e-cl", None)),
    )
    .await;
    let upstream = body_json(resp.into_body()).await;
    let uid = upstream["id"].as_str().unwrap();

    let resp = send(
        h.app.clone(),
        Method::POST,
        "/oagw/v1/routes",
        Some(create_route_json(uid, &["POST"], "/v1/test")),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Send request with non-integer Content-Length.
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oagw/v1/proxy/e2e-cl/v1/test")
        .header("content-type", "application/json")
        .header("content-length", "not-a-number")
        .body(Body::from(r#"{"test": true}"#))
        .unwrap();

    let resp = h.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// 8.11: Content-Length exceeding 100MB returns 413.
#[tokio::test]
async fn e2e_body_exceeding_limit_returns_413() {
    let h = setup_e2e().await;

    let resp = send(
        h.app.clone(),
        Method::POST,
        "/oagw/v1/upstreams",
        Some(create_upstream_json(h.mock_port, "e2e-big", None)),
    )
    .await;
    let upstream = body_json(resp.into_body()).await;
    let uid = upstream["id"].as_str().unwrap();

    let resp = send(
        h.app.clone(),
        Method::POST,
        "/oagw/v1/routes",
        Some(create_route_json(uid, &["POST"], "/v1/test")),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Send request with Content-Length exceeding 100MB.
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oagw/v1/proxy/e2e-big/v1/test")
        .header("content-type", "application/json")
        .header("content-length", "200000000") // 200MB
        .body(Body::from("small body"))
        .unwrap();

    let resp = h.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

// 10.4: E2E — upstream timeout returns 504.
#[tokio::test]
async fn e2e_upstream_timeout_returns_504() {
    let mock = MockUpstream::start().await;
    let mock_port = mock.addr().port();

    let hub = ClientHub::new();
    let h = build_test_app_state(
        &hub,
        TestCpBuilder::new(),
        TestDpBuilder::new().with_request_timeout(std::time::Duration::from_millis(500)),
    );
    let app = test_router(h.state, test_security_context());

    // Create upstream + route for /error.
    let resp = send(
        app.clone(),
        Method::POST,
        "/oagw/v1/upstreams",
        Some(create_upstream_json(mock_port, "e2e-timeout", None)),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let upstream = body_json(resp.into_body()).await;
    let uid = upstream["id"].as_str().unwrap();

    let resp = send(
        app.clone(),
        Method::POST,
        "/oagw/v1/routes",
        Some(create_route_json(uid, &["GET"], "/error")),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Proxy to /error/timeout — should return 504.
    let resp = send(
        app.clone(),
        Method::GET,
        "/oagw/v1/proxy/e2e-timeout/error/timeout",
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::GATEWAY_TIMEOUT);
    let source = resp
        .headers()
        .get("x-oagw-error-source")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(source, "gateway");
}
