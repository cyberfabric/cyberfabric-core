use std::sync::Arc;

use oagw::test_support::{
    APIKEY_AUTH_PLUGIN_ID, AppHarness, CapturingAuthZResolverClient, DenyingAuthZResolverClient,
    MockBody, MockGuard, MockResponse, parse_resource_gts,
};

// 10.1: E2E — create upstream, create route, proxy chat completion, verify round-trip.
#[tokio::test]
async fn e2e_chat_completion_round_trip() {
    let h = AppHarness::builder()
        .with_credentials(vec![("cred://openai-key".into(), "sk-e2e-test-key".into())])
        .build()
        .await;

    // Create upstream via Management API.
    let resp = h
        .api_v1()
        .post_upstream()
        .with_body(serde_json::json!({
            "server": {
                "endpoints": [{"host": "127.0.0.1", "port": h.mock_port(), "scheme": "http"}]
            },
            "protocol": "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
            "alias": "e2e-openai",
            "enabled": true,
            "tags": []
        }))
        .expect_status(201)
        .await;
    let upstream_id = resp.json()["id"].as_str().unwrap().to_string();

    // Create route via Management API.
    let (_, upstream_uuid) = parse_resource_gts(&upstream_id).unwrap();
    h.api_v1()
        .post_route()
        .with_body(serde_json::json!({
            "upstream_id": upstream_uuid,
            "match": {
                "http": {
                    "methods": ["POST"],
                    "path": "/v1/chat/completions"
                }
            },
            "enabled": true,
            "tags": [],
            "priority": 0
        }))
        .expect_status(201)
        .await;

    // Proxy a chat completion request.
    let resp = h
        .api_v1()
        .proxy_post("e2e-openai", "v1/chat/completions")
        .with_body(serde_json::json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        }))
        .expect_status(200)
        .await;

    let body = resp.json();
    assert!(body.get("id").is_some());
    assert!(body.get("choices").is_some());
}

// 10.2: E2E — SSE streaming round-trip via dynamic MockGuard route.
#[tokio::test]
async fn e2e_sse_streaming() {
    let mut guard = MockGuard::new();

    let chunks: Vec<String> = vec![
        serde_json::json!({"id":"chatcmpl-mock-stream","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"role":"assistant","content":"Hello"},"finish_reason":null}]}).to_string(),
        serde_json::json!({"id":"chatcmpl-mock-stream","object":"chat.completion.chunk","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}).to_string(),
        "[DONE]".to_string(),
    ];
    guard.mock(
        "POST",
        "/v1/chat/completions/stream",
        MockResponse {
            status: 200,
            headers: vec![("content-type".into(), "text/event-stream".into())],
            body: MockBody::Sse(chunks),
        },
    );

    let h = AppHarness::builder().build().await;

    let resp = h
        .api_v1()
        .post_upstream()
        .with_body(serde_json::json!({
            "server": {
                "endpoints": [{"host": "127.0.0.1", "port": h.mock_port(), "scheme": "http"}]
            },
            "protocol": "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
            "alias": "e2e-sse",
            "enabled": true,
            "tags": []
        }))
        .expect_status(201)
        .await;
    let uid = resp.json()["id"].as_str().unwrap().to_string();

    let (_, upstream_uuid) = parse_resource_gts(&uid).unwrap();
    let route_path = guard.path("/v1/chat/completions/stream");
    h.api_v1()
        .post_route()
        .with_body(serde_json::json!({
            "upstream_id": upstream_uuid,
            "match": {
                "http": {
                    "methods": ["POST"],
                    "path": route_path
                }
            },
            "enabled": true,
            "tags": [],
            "priority": 0
        }))
        .expect_status(201)
        .await;

    // Proxy streaming request via guard-prefixed path.
    let resp = h
        .api_v1()
        .proxy_post("e2e-sse", &route_path[1..])
        .with_body(serde_json::json!({"model": "gpt-4", "stream": true}))
        .expect_status(200)
        .await;

    resp.assert_body_contains("data: [DONE]");
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(ct.contains("text/event-stream"), "content-type: {ct}");
}

// 10.3: E2E — auth injection round-trip.
#[tokio::test]
async fn e2e_auth_injection() {
    let h = AppHarness::builder()
        .with_credentials(vec![("cred://openai-key".into(), "sk-e2e-test-key".into())])
        .build()
        .await;

    let resp = h
        .api_v1()
        .post_upstream()
        .with_body(serde_json::json!({
            "server": {
                "endpoints": [{"host": "127.0.0.1", "port": h.mock_port(), "scheme": "http"}]
            },
            "protocol": "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
            "alias": "e2e-auth",
            "enabled": true,
            "tags": [],
            "auth": {
                "type": APIKEY_AUTH_PLUGIN_ID,
                "sharing": "private",
                "config": {
                    "header": "authorization",
                    "prefix": "Bearer ",
                    "secret_ref": "cred://openai-key"
                }
            }
        }))
        .expect_status(201)
        .await;
    let uid = resp.json()["id"].as_str().unwrap().to_string();

    let (_, upstream_uuid) = parse_resource_gts(&uid).unwrap();
    h.api_v1()
        .post_route()
        .with_body(serde_json::json!({
            "upstream_id": upstream_uuid,
            "match": {
                "http": {
                    "methods": ["POST"],
                    "path": "/echo"
                }
            },
            "enabled": true,
            "tags": [],
            "priority": 0
        }))
        .expect_status(201)
        .await;

    // Proxy to echo and verify auth header.
    let resp = h
        .api_v1()
        .proxy_post("e2e-auth", "echo")
        .with_body(serde_json::json!({"test": true}))
        .expect_status(200)
        .await;

    let body = resp.json();
    let headers = body["headers"].as_object().unwrap();
    let auth = headers.get("authorization").unwrap().as_str().unwrap();
    assert_eq!(auth, "Bearer sk-e2e-test-key");
}

// 10.4: E2E — error scenarios.
#[tokio::test]
async fn e2e_nonexistent_alias_returns_404() {
    let h = AppHarness::builder()
        .with_credentials(vec![("cred://openai-key".into(), "sk-e2e-test-key".into())])
        .build()
        .await;

    let resp = h
        .api_v1()
        .proxy_get("nonexistent", "v1/test")
        .expect_status(404)
        .await;

    resp.assert_header("x-oagw-error-source", "gateway");
}

#[tokio::test]
async fn e2e_disabled_upstream_returns_503() {
    let h = AppHarness::builder()
        .with_credentials(vec![("cred://openai-key".into(), "sk-e2e-test-key".into())])
        .build()
        .await;

    h.api_v1()
        .post_upstream()
        .with_body(serde_json::json!({
            "server": {
                "endpoints": [{"host": "127.0.0.1", "port": h.mock_port(), "scheme": "http"}]
            },
            "protocol": "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
            "alias": "e2e-disabled",
            "enabled": false,
            "tags": []
        }))
        .expect_status(201)
        .await;

    h.api_v1()
        .proxy_get("e2e-disabled", "v1/test")
        .expect_status(503)
        .await;
}

#[tokio::test]
async fn e2e_upstream_500_passthrough() {
    let h = AppHarness::builder()
        .with_credentials(vec![("cred://openai-key".into(), "sk-e2e-test-key".into())])
        .build()
        .await;

    let resp = h
        .api_v1()
        .post_upstream()
        .with_body(serde_json::json!({
            "server": {
                "endpoints": [{"host": "127.0.0.1", "port": h.mock_port(), "scheme": "http"}]
            },
            "protocol": "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
            "alias": "e2e-errors",
            "enabled": true,
            "tags": []
        }))
        .expect_status(201)
        .await;
    let uid = resp.json()["id"].as_str().unwrap().to_string();

    let (_, upstream_uuid) = parse_resource_gts(&uid).unwrap();
    h.api_v1()
        .post_route()
        .with_body(serde_json::json!({
            "upstream_id": upstream_uuid,
            "match": {
                "http": {
                    "methods": ["GET"],
                    "path": "/error"
                }
            },
            "enabled": true,
            "tags": [],
            "priority": 0
        }))
        .expect_status(201)
        .await;

    let resp = h
        .api_v1()
        .proxy_get("e2e-errors", "error/500")
        .expect_status(500)
        .await;

    resp.assert_header("x-oagw-error-source", "upstream");
}

// 10.4: E2E — rate limit exceeded.
#[tokio::test]
async fn e2e_rate_limit_returns_429() {
    let h = AppHarness::builder()
        .with_credentials(vec![("cred://openai-key".into(), "sk-e2e-test-key".into())])
        .build()
        .await;

    let resp = h
        .api_v1()
        .post_upstream()
        .with_body(serde_json::json!({
            "server": {
                "endpoints": [{"host": "127.0.0.1", "port": h.mock_port(), "scheme": "http"}]
            },
            "protocol": "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
            "alias": "e2e-rl",
            "enabled": true,
            "tags": [],
            "rate_limit": {
                "algorithm": "token_bucket",
                "sustained": {"rate": 1, "window": "minute"},
                "burst": {"capacity": 1},
                "scope": "tenant",
                "strategy": "reject",
                "cost": 1
            }
        }))
        .expect_status(201)
        .await;
    let uid = resp.json()["id"].as_str().unwrap().to_string();

    let (_, upstream_uuid) = parse_resource_gts(&uid).unwrap();
    h.api_v1()
        .post_route()
        .with_body(serde_json::json!({
            "upstream_id": upstream_uuid,
            "match": {
                "http": {
                    "methods": ["GET"],
                    "path": "/v1/models"
                }
            },
            "enabled": true,
            "tags": [],
            "priority": 0
        }))
        .expect_status(201)
        .await;

    // First request succeeds.
    h.api_v1()
        .proxy_get("e2e-rl", "v1/models")
        .expect_status(200)
        .await;

    // Second request should be rate limited.
    h.api_v1()
        .proxy_get("e2e-rl", "v1/models")
        .expect_status(429)
        .await;
}

// 10.5: E2E — management lifecycle.
#[tokio::test]
async fn e2e_management_lifecycle() {
    let h = AppHarness::builder()
        .with_credentials(vec![("cred://openai-key".into(), "sk-e2e-test-key".into())])
        .build()
        .await;

    // Create.
    let resp = h
        .api_v1()
        .post_upstream()
        .with_body(serde_json::json!({
            "server": {
                "endpoints": [{"host": "127.0.0.1", "port": h.mock_port(), "scheme": "http"}]
            },
            "protocol": "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
            "alias": "lifecycle",
            "enabled": true,
            "tags": []
        }))
        .expect_status(201)
        .await;
    let uid = resp.json()["id"].as_str().unwrap().to_string();

    // List (appears).
    let resp = h.api_v1().list_upstreams().expect_status(200).await;
    let list = resp.json();
    assert!(
        list.as_array()
            .unwrap()
            .iter()
            .any(|u| u["id"].as_str() == Some(uid.as_str()))
    );

    // Update alias.
    h.api_v1()
        .patch_upstream(&uid)
        .with_body(serde_json::json!({"alias": "lifecycle-v2"}))
        .expect_status(200)
        .await;

    // Get (updated).
    let resp = h.api_v1().get_upstream(&uid).expect_status(200).await;
    assert_eq!(resp.json()["alias"].as_str().unwrap(), "lifecycle-v2");

    // Create route.
    let (_, upstream_uuid) = parse_resource_gts(&uid).unwrap();
    h.api_v1()
        .post_route()
        .with_body(serde_json::json!({
            "upstream_id": upstream_uuid,
            "match": {
                "http": {
                    "methods": ["GET"],
                    "path": "/test"
                }
            },
            "enabled": true,
            "tags": [],
            "priority": 0
        }))
        .expect_status(201)
        .await;

    // Delete upstream (cascades routes).
    h.api_v1().delete_upstream(&uid).expect_status(204).await;

    // List (gone).
    let resp = h.api_v1().list_upstreams().expect_status(200).await;
    let list = resp.json();
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
    let h = AppHarness::builder()
        .with_credentials(vec![("cred://openai-key".into(), "sk-e2e-test-key".into())])
        .build()
        .await;

    let resp = h
        .api_v1()
        .post_upstream()
        .with_body(serde_json::json!({
            "server": {
                "endpoints": [{"host": "127.0.0.1", "port": h.mock_port(), "scheme": "http"}]
            },
            "protocol": "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
            "alias": "e2e-cl",
            "enabled": true,
            "tags": []
        }))
        .expect_status(201)
        .await;
    let uid = resp.json()["id"].as_str().unwrap().to_string();

    let (_, upstream_uuid) = parse_resource_gts(&uid).unwrap();
    h.api_v1()
        .post_route()
        .with_body(serde_json::json!({
            "upstream_id": upstream_uuid,
            "match": {
                "http": {
                    "methods": ["POST"],
                    "path": "/v1/test"
                }
            },
            "enabled": true,
            "tags": [],
            "priority": 0
        }))
        .expect_status(201)
        .await;

    // Send request with non-integer Content-Length.
    h.api_v1()
        .proxy_post("e2e-cl", "v1/test")
        .with_body(serde_json::json!({"test": true}))
        .with_header(
            http::header::CONTENT_LENGTH,
            http::HeaderValue::from_static("not-a-number"),
        )
        .expect_status(400)
        .await;
}

// 8.11: Content-Length exceeding 100MB returns 413.
#[tokio::test]
async fn e2e_body_exceeding_limit_returns_413() {
    let h = AppHarness::builder()
        .with_credentials(vec![("cred://openai-key".into(), "sk-e2e-test-key".into())])
        .build()
        .await;

    let resp = h
        .api_v1()
        .post_upstream()
        .with_body(serde_json::json!({
            "server": {
                "endpoints": [{"host": "127.0.0.1", "port": h.mock_port(), "scheme": "http"}]
            },
            "protocol": "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
            "alias": "e2e-big",
            "enabled": true,
            "tags": []
        }))
        .expect_status(201)
        .await;
    let uid = resp.json()["id"].as_str().unwrap().to_string();

    let (_, upstream_uuid) = parse_resource_gts(&uid).unwrap();
    h.api_v1()
        .post_route()
        .with_body(serde_json::json!({
            "upstream_id": upstream_uuid,
            "match": {
                "http": {
                    "methods": ["POST"],
                    "path": "/v1/test"
                }
            },
            "enabled": true,
            "tags": [],
            "priority": 0
        }))
        .expect_status(201)
        .await;

    // Send request with Content-Length exceeding 100MB.
    h.api_v1()
        .proxy_post("e2e-big", "v1/test")
        .with_body("small body")
        .with_header(
            http::header::CONTENT_LENGTH,
            http::HeaderValue::from_static("200000000"),
        )
        .expect_status(413)
        .await;
}

// 10.4: E2E — upstream timeout returns 504 via gated mock that never responds.
// Uses multi_thread runtime so the timer driver runs on a dedicated thread,
// preventing stalls when other test binaries compete for CPU.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn e2e_upstream_timeout_returns_504() {
    let mut guard = MockGuard::new();
    // Register a gated route that will never respond (sender kept alive but not signaled).
    let _gate = guard.mock_gated(
        "GET",
        "/timeout",
        MockResponse {
            status: 200,
            headers: vec![],
            body: MockBody::Json(serde_json::json!({"ok": true})),
        },
    );

    let h = AppHarness::builder()
        .with_request_timeout(std::time::Duration::from_millis(500))
        .build()
        .await;

    let resp = h
        .api_v1()
        .post_upstream()
        .with_body(serde_json::json!({
            "server": {
                "endpoints": [{"host": "127.0.0.1", "port": h.mock_port(), "scheme": "http"}]
            },
            "protocol": "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
            "alias": "e2e-timeout",
            "enabled": true,
            "tags": []
        }))
        .expect_status(201)
        .await;
    let uid = resp.json()["id"].as_str().unwrap().to_string();

    let route_path = guard.path("/timeout");
    let (_, upstream_uuid) = parse_resource_gts(&uid).unwrap();
    h.api_v1()
        .post_route()
        .with_body(serde_json::json!({
            "upstream_id": upstream_uuid,
            "match": {
                "http": {
                    "methods": ["GET"],
                    "path": route_path
                }
            },
            "enabled": true,
            "tags": [],
            "priority": 0
        }))
        .expect_status(201)
        .await;

    // Proxy to gated mock — should return 504.
    let resp = h
        .api_v1()
        .proxy_get("e2e-timeout", &route_path[1..])
        .expect_status(504)
        .await;

    resp.assert_header("x-oagw-error-source", "gateway");
}

// 10.11: E2E — proxy request denied by AuthZ returns 403 Forbidden.
#[tokio::test]
async fn e2e_authz_forbidden_returns_403() {
    let h = AppHarness::builder()
        .with_authz_client(Arc::new(DenyingAuthZResolverClient))
        .build()
        .await;

    // Create upstream and route so the denial is purely from AuthZ, not routing.
    let resp = h
        .api_v1()
        .post_upstream()
        .with_body(serde_json::json!({
            "server": {
                "endpoints": [{"host": "127.0.0.1", "port": h.mock_port(), "scheme": "http"}]
            },
            "protocol": "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
            "alias": "e2e-forbidden",
            "enabled": true,
            "tags": []
        }))
        .expect_status(201)
        .await;
    let upstream_id = resp.json()["id"].as_str().unwrap().to_string();

    let (_, upstream_uuid) = parse_resource_gts(&upstream_id).unwrap();
    h.api_v1()
        .post_route()
        .with_body(serde_json::json!({
            "upstream_id": upstream_uuid,
            "match": {
                "http": {
                    "methods": ["GET"],
                    "path": "/v1/models"
                }
            },
            "enabled": true,
            "tags": [],
            "priority": 0
        }))
        .expect_status(201)
        .await;

    // Proxy request should be denied before reaching the upstream.
    let resp = h
        .api_v1()
        .proxy_get("e2e-forbidden", "v1/models")
        .expect_status(403)
        .await;

    resp.assert_header("x-oagw-error-source", "gateway");

    let body = resp.json();
    assert_eq!(body["status"], 403);
    assert_eq!(body["title"], "Forbidden");
    assert_eq!(
        body["type"],
        "gts.x.core.errors.err.v1~x.oagw.authz.forbidden.v1"
    );
}

// 10.12: E2E — proxy authz evaluation request carries caller's tenant context.
#[tokio::test]
async fn e2e_authz_request_carries_tenant_context() {
    let capturing = Arc::new(CapturingAuthZResolverClient::new());

    let mut guard = MockGuard::new();
    guard.mock(
        "GET",
        "/v1/test",
        MockResponse {
            status: 200,
            headers: vec![],
            body: MockBody::Json(serde_json::json!({"ok": true})),
        },
    );

    let h = AppHarness::builder()
        .with_authz_client(capturing.clone())
        .build()
        .await;

    let resp = h
        .api_v1()
        .post_upstream()
        .with_body(serde_json::json!({
            "server": {
                "endpoints": [{"host": "127.0.0.1", "port": h.mock_port(), "scheme": "http"}]
            },
            "protocol": "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
            "alias": "e2e-authz-ctx",
            "enabled": true,
            "tags": []
        }))
        .expect_status(201)
        .await;
    let upstream_id = resp.json()["id"].as_str().unwrap().to_string();

    let route_path = guard.path("/v1/test");
    let (_, upstream_uuid) = parse_resource_gts(&upstream_id).unwrap();
    h.api_v1()
        .post_route()
        .with_body(serde_json::json!({
            "upstream_id": upstream_uuid,
            "match": {
                "http": {
                    "methods": ["GET"],
                    "path": route_path
                }
            },
            "enabled": true,
            "tags": [],
            "priority": 0
        }))
        .expect_status(201)
        .await;

    h.api_v1()
        .proxy_get("e2e-authz-ctx", &route_path[1..])
        .expect_status(200)
        .await;

    let requests = capturing.recorded();
    assert!(
        !requests.is_empty(),
        "expected at least one captured evaluation request"
    );

    let req = &requests[0];
    let tenant_ctx = req
        .context
        .tenant_context
        .as_ref()
        .expect("expected tenant_context in evaluation request");
    assert_eq!(
        tenant_ctx.root_id,
        Some(h.security_context().subject_tenant_id()),
        "tenant_context.root_id should match subject_tenant_id"
    );
    assert_eq!(req.resource.resource_type, "gts.x.core.oagw.proxy.v1~");
    assert_eq!(req.action.name, "invoke");
}
