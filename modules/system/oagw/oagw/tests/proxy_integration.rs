use std::sync::Arc;

use bytes::Bytes;
use futures_util::StreamExt;
use http::{Method, StatusCode};
use modkit::client_hub::ClientHub;
use modkit_security::SecurityContext;
use oagw::test_support::{APIKEY_AUTH_PLUGIN_ID, MockUpstream, TestCpBuilder, TestDpBuilder, build_test_gateway};
use oagw_sdk::api::{ErrorSource, ServiceGatewayClientV1};
use oagw_sdk::{
    AuthConfig, BurstConfig, CreateRouteRequest, CreateUpstreamRequest, Endpoint, HttpMatch,
    HttpMethod, MatchRules, PathSuffixMode, RateLimitAlgorithm, RateLimitConfig, RateLimitScope,
    RateLimitStrategy, Scheme, Server, SharingMode, SustainedRate, Window,
};
use uuid::Uuid;

fn make_ctx() -> SecurityContext {
    SecurityContext::builder()
        .tenant_id(Uuid::new_v4())
        .subject_id(Uuid::new_v4())
        .build()
}

struct TestHarness {
    _mock: MockUpstream,
    oagw: Arc<dyn ServiceGatewayClientV1>,
    ctx: SecurityContext,
}

async fn setup() -> TestHarness {
    let mock = MockUpstream::start().await;
    let addr = mock.addr();

    let hub = ClientHub::new();
    let oagw = build_test_gateway(
        &hub,
        TestCpBuilder::new()
            .with_credentials(vec![("cred://openai-key".into(), "sk-test123".into())]),
        TestDpBuilder::new(),
    );

    let ctx = make_ctx();

    // Create upstream pointing at mock server.
    let upstream = oagw
        .create_upstream(
            ctx.clone(),
            CreateUpstreamRequest::builder(
                Server {
                    endpoints: vec![Endpoint {
                        scheme: Scheme::Http,
                        host: "127.0.0.1".into(),
                        port: addr.port(),
                    }],
                },
                "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
            )
            .alias("mock-upstream")
            .auth(AuthConfig {
                plugin_type: APIKEY_AUTH_PLUGIN_ID.into(),
                sharing: SharingMode::Private,
                config: Some(serde_json::json!({
                    "header": "authorization",
                    "prefix": "Bearer ",
                    "secret_ref": "cred://openai-key"
                })),
            })
            .build(),
        )
        .await
        .unwrap();

    // Create route for /v1/chat/completions.
    oagw.create_route(
        ctx.clone(),
        CreateRouteRequest::builder(
            upstream.id,
            MatchRules {
                http: Some(HttpMatch {
                    methods: vec![HttpMethod::Post, HttpMethod::Get],
                    path: "/v1/chat/completions".into(),
                    query_allowlist: vec![],
                    path_suffix_mode: PathSuffixMode::Append,
                }),
                grpc: None,
            },
        )
        .build(),
    )
    .await
    .unwrap();

    // Create route for SSE streaming.
    oagw.create_route(
        ctx.clone(),
        CreateRouteRequest::builder(
            upstream.id,
            MatchRules {
                http: Some(HttpMatch {
                    methods: vec![HttpMethod::Post],
                    path: "/v1/chat/completions/stream".into(),
                    query_allowlist: vec![],
                    path_suffix_mode: PathSuffixMode::Append,
                }),
                grpc: None,
            },
        )
        .build(),
    )
    .await
    .unwrap();

    // Create route for error endpoints.
    oagw.create_route(
        ctx.clone(),
        CreateRouteRequest::builder(
            upstream.id,
            MatchRules {
                http: Some(HttpMatch {
                    methods: vec![HttpMethod::Get],
                    path: "/error".into(),
                    query_allowlist: vec![],
                    path_suffix_mode: PathSuffixMode::Append,
                }),
                grpc: None,
            },
        )
        .build(),
    )
    .await
    .unwrap();

    TestHarness {
        _mock: mock,
        oagw,
        ctx,
    }
}

fn make_proxy_request(
    ctx: &SecurityContext,
    method: Method,
    alias: &str,
    path_suffix: &str,
    body: &str,
) -> (SecurityContext, http::Request<Bytes>) {
    let uri = format!("/{alias}{path_suffix}");
    let mut builder = http::Request::builder().method(method).uri(&uri);
    if !body.is_empty() {
        builder = builder.header(http::header::CONTENT_TYPE, "application/json");
    }
    let req = builder.body(Bytes::from(body.to_string())).unwrap();
    (ctx.clone(), req)
}

/// Collect body stream into bytes.
async fn collect_body(body: oagw_sdk::api::BodyStream) -> Vec<u8> {
    let mut collected = Vec::new();
    tokio::pin!(body);
    while let Some(chunk) = body.next().await {
        match chunk {
            Ok(b) => collected.extend_from_slice(&b),
            Err(_) => break,
        }
    }
    collected
}

// 6.13: Full pipeline — proxy POST /v1/chat/completions with JSON body.
#[tokio::test]
async fn proxy_chat_completion_round_trip() {
    let h = setup().await;
    let body = r#"{"model":"gpt-4","messages":[{"role":"user","content":"Hello"}]}"#;
    let (ctx, req) = make_proxy_request(
        &h.ctx,
        Method::POST,
        "mock-upstream",
        "/v1/chat/completions",
        body,
    );
    let response = h.oagw.proxy_request(ctx, req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = collect_body(response.into_body()).await;
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(body_json.get("id").is_some());
    assert!(body_json.get("choices").is_some());
}

// 6.13 (auth): Verify the mock received the Authorization header.
#[tokio::test]
async fn proxy_injects_auth_header() {
    let h = setup().await;
    let (ctx, req) = make_proxy_request(
        &h.ctx,
        Method::POST,
        "mock-upstream",
        "/v1/chat/completions",
        r#"{"model":"gpt-4","messages":[]}"#,
    );
    let response = h.oagw.proxy_request(ctx, req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Check that mock received auth header.
    let recorded = h._mock.recorded_requests().await;
    assert!(!recorded.is_empty());
    let last = &recorded[recorded.len() - 1];
    let auth_header = last
        .headers
        .iter()
        .find(|(k, _)| k == "authorization")
        .map(|(_, v)| v.as_str())
        .expect("authorization header missing");
    assert_eq!(auth_header, "Bearer sk-test123");
}

// 6.14: SSE streaming — proxy to /v1/chat/completions/stream.
#[tokio::test]
async fn proxy_sse_streaming() {
    let h = setup().await;
    let (ctx, req) = make_proxy_request(
        &h.ctx,
        Method::POST,
        "mock-upstream",
        "/v1/chat/completions/stream",
        r#"{"model":"gpt-4","stream":true}"#,
    );
    let response = h.oagw.proxy_request(ctx, req).await.unwrap();

    // Verify content-type is SSE.
    let ct = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(ct.contains("text/event-stream"), "got content-type: {ct}");

    // Collect all chunks.
    let body_bytes = collect_body(response.into_body()).await;
    let body_str = String::from_utf8(body_bytes).unwrap();
    assert!(body_str.contains("data: [DONE]"));
}

// 6.15: Upstream 500 error passthrough.
#[tokio::test]
async fn proxy_upstream_500_passthrough() {
    let h = setup().await;
    let (ctx, req) = make_proxy_request(&h.ctx, Method::GET, "mock-upstream", "/error/500", "");
    let response = h.oagw.proxy_request(ctx, req).await.unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        response.extensions().get::<ErrorSource>().copied(),
        Some(ErrorSource::Upstream)
    );
}

// 6.17: Pipeline abort — nonexistent alias returns 404 without calling mock.
#[tokio::test]
async fn proxy_nonexistent_alias_returns_404() {
    let h = setup().await;
    let (ctx, req) = make_proxy_request(&h.ctx, Method::GET, "nonexistent", "/v1/test", "");
    match h.oagw.proxy_request(ctx, req).await {
        Err(err) => assert!(matches!(err, oagw_sdk::error::ServiceGatewayError::RouteNotFound { .. })),
        Ok(_) => panic!("expected error"),
    }
}

// 6.17: Pipeline abort — disabled upstream returns 503.
#[tokio::test]
async fn proxy_disabled_upstream_returns_503() {
    let h = setup().await;

    // Create a disabled upstream.
    let upstream = h
        .oagw
        .create_upstream(
            h.ctx.clone(),
            CreateUpstreamRequest::builder(
                Server {
                    endpoints: vec![Endpoint {
                        scheme: Scheme::Http,
                        host: "127.0.0.1".into(),
                        port: 9999,
                    }],
                },
                "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
            )
            .alias("disabled-upstream")
            .enabled(false)
            .build(),
        )
        .await
        .unwrap();
    assert!(!upstream.enabled);

    let (ctx, req) = make_proxy_request(&h.ctx, Method::GET, "disabled-upstream", "/test", "");
    match h.oagw.proxy_request(ctx, req).await {
        Err(err) => assert!(matches!(err, oagw_sdk::error::ServiceGatewayError::UpstreamDisabled { .. })),
        Ok(_) => panic!("expected error"),
    }
}

// 6.17: Pipeline abort — rate limit exceeded returns 429.
#[tokio::test]
async fn proxy_rate_limit_exceeded_returns_429() {
    let mock = MockUpstream::start().await;
    let addr = mock.addr();

    let hub = ClientHub::new();
    let oagw = build_test_gateway(&hub, TestCpBuilder::new(), TestDpBuilder::new());
    let ctx = make_ctx();

    // Create upstream with tight rate limit (1 per minute).
    let upstream = oagw
        .create_upstream(
            ctx.clone(),
            CreateUpstreamRequest::builder(
                Server {
                    endpoints: vec![Endpoint {
                        scheme: Scheme::Http,
                        host: "127.0.0.1".into(),
                        port: addr.port(),
                    }],
                },
                "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
            )
            .alias("rate-limited")
            .rate_limit(RateLimitConfig {
                sharing: SharingMode::Private,
                algorithm: RateLimitAlgorithm::TokenBucket,
                sustained: SustainedRate {
                    rate: 1,
                    window: Window::Minute,
                },
                burst: Some(BurstConfig { capacity: 1 }),
                scope: RateLimitScope::Tenant,
                strategy: RateLimitStrategy::Reject,
                cost: 1,
            })
            .build(),
        )
        .await
        .unwrap();

    oagw.create_route(
        ctx.clone(),
        CreateRouteRequest::builder(
            upstream.id,
            MatchRules {
                http: Some(HttpMatch {
                    methods: vec![HttpMethod::Get],
                    path: "/v1/models".into(),
                    query_allowlist: vec![],
                    path_suffix_mode: PathSuffixMode::Append,
                }),
                grpc: None,
            },
        )
        .build(),
    )
    .await
    .unwrap();

    // First request should succeed.
    let (c, req) = make_proxy_request(&ctx, Method::GET, "rate-limited", "/v1/models", "");
    let response = oagw.proxy_request(c, req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Second request should be rate limited.
    let (c, req) = make_proxy_request(&ctx, Method::GET, "rate-limited", "/v1/models", "");
    match oagw.proxy_request(c, req).await {
        Err(err) => assert!(matches!(err, oagw_sdk::error::ServiceGatewayError::RateLimitExceeded { .. })),
        Ok(_) => panic!("expected rate limit error"),
    }
}

// 6.16: Upstream timeout — proxy to /error/timeout with short timeout, assert 504.
#[tokio::test]
async fn proxy_upstream_timeout_returns_504() {
    let mock = MockUpstream::start().await;
    let addr = mock.addr();

    let hub = ClientHub::new();
    let oagw = build_test_gateway(
        &hub,
        TestCpBuilder::new(),
        TestDpBuilder::new().with_request_timeout(std::time::Duration::from_millis(500)),
    );
    let ctx = make_ctx();

    let upstream = oagw
        .create_upstream(
            ctx.clone(),
            CreateUpstreamRequest::builder(
                Server {
                    endpoints: vec![Endpoint {
                        scheme: Scheme::Http,
                        host: "127.0.0.1".into(),
                        port: addr.port(),
                    }],
                },
                "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
            )
            .alias("timeout-upstream")
            .build(),
        )
        .await
        .unwrap();

    oagw.create_route(
        ctx.clone(),
        CreateRouteRequest::builder(
            upstream.id,
            MatchRules {
                http: Some(HttpMatch {
                    methods: vec![HttpMethod::Get],
                    path: "/error".into(),
                    query_allowlist: vec![],
                    path_suffix_mode: PathSuffixMode::Append,
                }),
                grpc: None,
            },
        )
        .build(),
    )
    .await
    .unwrap();

    let (c, req) = make_proxy_request(
        &ctx,
        Method::GET,
        "timeout-upstream",
        "/error/timeout",
        "",
    );
    match oagw.proxy_request(c, req).await {
        Err(err) => assert!(matches!(err, oagw_sdk::error::ServiceGatewayError::RequestTimeout { .. })),
        Ok(_) => panic!("expected timeout error"),
    }
}

// 8.9: Query allowlist enforcement.
#[tokio::test]
async fn proxy_query_allowlist_allowed_param_succeeds() {
    let mock = MockUpstream::start().await;
    let addr = mock.addr();

    let hub = ClientHub::new();
    let oagw = build_test_gateway(&hub, TestCpBuilder::new(), TestDpBuilder::new());
    let ctx = make_ctx();

    let upstream = oagw
        .create_upstream(
            ctx.clone(),
            CreateUpstreamRequest::builder(
                Server {
                    endpoints: vec![Endpoint {
                        scheme: Scheme::Http,
                        host: "127.0.0.1".into(),
                        port: addr.port(),
                    }],
                },
                "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
            )
            .alias("ql-test")
            .build(),
        )
        .await
        .unwrap();

    oagw.create_route(
        ctx.clone(),
        CreateRouteRequest::builder(
            upstream.id,
            MatchRules {
                http: Some(HttpMatch {
                    methods: vec![HttpMethod::Get],
                    path: "/v1/models".into(),
                    query_allowlist: vec!["version".into()],
                    path_suffix_mode: PathSuffixMode::Append,
                }),
                grpc: None,
            },
        )
        .build(),
    )
    .await
    .unwrap();

    // Allowed param succeeds — query params encoded in URI.
    let (c, req) = make_proxy_request(&ctx, Method::GET, "ql-test", "/v1/models?version=2", "");
    let response = oagw.proxy_request(c, req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn proxy_query_allowlist_unknown_param_rejected() {
    let mock = MockUpstream::start().await;
    let addr = mock.addr();

    let hub = ClientHub::new();
    let oagw = build_test_gateway(&hub, TestCpBuilder::new(), TestDpBuilder::new());
    let ctx = make_ctx();

    let upstream = oagw
        .create_upstream(
            ctx.clone(),
            CreateUpstreamRequest::builder(
                Server {
                    endpoints: vec![Endpoint {
                        scheme: Scheme::Http,
                        host: "127.0.0.1".into(),
                        port: addr.port(),
                    }],
                },
                "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
            )
            .alias("ql-reject")
            .build(),
        )
        .await
        .unwrap();

    oagw.create_route(
        ctx.clone(),
        CreateRouteRequest::builder(
            upstream.id,
            MatchRules {
                http: Some(HttpMatch {
                    methods: vec![HttpMethod::Get],
                    path: "/v1/models".into(),
                    query_allowlist: vec!["version".into()],
                    path_suffix_mode: PathSuffixMode::Append,
                }),
                grpc: None,
            },
        )
        .build(),
    )
    .await
    .unwrap();

    // Unknown param rejected with 400 — query params encoded in URI.
    let (c, req) = make_proxy_request(&ctx, Method::GET, "ql-reject", "/v1/models?version=2&debug=true", "");
    match oagw.proxy_request(c, req).await {
        Err(err) => assert!(matches!(err, oagw_sdk::error::ServiceGatewayError::ValidationError { .. })),
        Ok(_) => panic!("expected validation error"),
    }
}

// 8.10: path_suffix_mode=disabled rejects suffix; append succeeds.
#[tokio::test]
async fn proxy_path_suffix_disabled_rejects_extra_path() {
    let mock = MockUpstream::start().await;
    let addr = mock.addr();

    let hub = ClientHub::new();
    let oagw = build_test_gateway(&hub, TestCpBuilder::new(), TestDpBuilder::new());
    let ctx = make_ctx();

    let upstream = oagw
        .create_upstream(
            ctx.clone(),
            CreateUpstreamRequest::builder(
                Server {
                    endpoints: vec![Endpoint {
                        scheme: Scheme::Http,
                        host: "127.0.0.1".into(),
                        port: addr.port(),
                    }],
                },
                "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
            )
            .alias("psm-test")
            .build(),
        )
        .await
        .unwrap();

    // Route with path_suffix_mode=Disabled.
    oagw.create_route(
        ctx.clone(),
        CreateRouteRequest::builder(
            upstream.id,
            MatchRules {
                http: Some(HttpMatch {
                    methods: vec![HttpMethod::Get],
                    path: "/v1/models".into(),
                    query_allowlist: vec![],
                    path_suffix_mode: PathSuffixMode::Disabled,
                }),
                grpc: None,
            },
        )
        .build(),
    )
    .await
    .unwrap();

    // Exact path succeeds.
    let (c, req) = make_proxy_request(&ctx, Method::GET, "psm-test", "/v1/models", "");
    let response = oagw.proxy_request(c, req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Extra suffix rejected with 400.
    let (c, req) = make_proxy_request(&ctx, Method::GET, "psm-test", "/v1/models/gpt-4", "");
    match oagw.proxy_request(c, req).await {
        Err(err) => assert!(matches!(err, oagw_sdk::error::ServiceGatewayError::ValidationError { .. })),
        Ok(_) => panic!("expected validation error for disabled path_suffix_mode"),
    }
}
