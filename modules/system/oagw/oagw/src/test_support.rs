//! Test utilities for OAGW integration tests.
//!
//! Re-exports CP and DP test builders, plus provides a mock upstream server
//! that simulates upstream services (OpenAI-compatible HTTP JSON, SSE streaming,
//! error conditions, WebSocket, WebTransport stub).
//!
//! # Usage
//! ```ignore
//! let mock = MockUpstream::start().await;
//! // Use mock.base_url() to configure upstream endpoints
//! let requests = mock.recorded_requests().await;
//! mock.stop().await;
//! ```

pub use crate::domain::test_support::{
    APIKEY_AUTH_PLUGIN_ID, TestAppState, TestCpBuilder, TestCredentialResolver, TestDpBuilder,
    build_test_app_state, build_test_gateway,
};
pub use crate::domain::gts_helpers::{format_route_gts, format_upstream_gts, parse_resource_gts};

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{OriginalUri, Path, State, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::response::sse::{Event, Sse};
use axum::routing::{get, post};
use bytes::Bytes;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

// ---------------------------------------------------------------------------
// Recording types
// ---------------------------------------------------------------------------

/// A captured inbound request for test assertions.
#[derive(Debug, Clone)]
pub struct RecordedRequest {
    pub method: String,
    pub uri: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

struct SharedState {
    recorded: Mutex<VecDeque<RecordedRequest>>,
    max_recorded: usize,
}

impl SharedState {
    fn new(max_recorded: usize) -> Self {
        Self {
            recorded: Mutex::new(VecDeque::new()),
            max_recorded,
        }
    }

    async fn record(&self, method: &str, uri: &str, headers: &HeaderMap, body: &[u8]) {
        let hdrs = headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let entry = RecordedRequest {
            method: method.to_string(),
            uri: uri.to_string(),
            headers: hdrs,
            body: body.to_vec(),
        };
        let mut queue = self.recorded.lock().await;
        if queue.len() >= self.max_recorded {
            queue.pop_front();
        }
        queue.push_back(entry);
    }
}

// ---------------------------------------------------------------------------
// MockUpstream
// ---------------------------------------------------------------------------

/// A mock upstream HTTP server bound to a random local port.
pub struct MockUpstream {
    addr: SocketAddr,
    state: Arc<SharedState>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for MockUpstream {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(h) = self.handle.take() {
            h.abort();
        }
    }
}

impl MockUpstream {
    /// Start the mock server on `127.0.0.1:0` (random port).
    pub async fn start() -> Self {
        let state = Arc::new(SharedState::new(200));
        let app = Self::router(Arc::clone(&state));

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind mock upstream");
        let addr = listener.local_addr().expect("failed to get local addr");

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .expect("mock server error");
        });

        Self {
            addr,
            state,
            shutdown_tx: Some(shutdown_tx),
            handle: Some(handle),
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Return a snapshot of all recorded requests (oldest first).
    pub async fn recorded_requests(&self) -> Vec<RecordedRequest> {
        self.state.recorded.lock().await.iter().cloned().collect()
    }

    /// Clear all recorded requests.
    pub async fn clear_recorded(&self) {
        self.state.recorded.lock().await.clear();
    }

    /// Gracefully stop the mock server.
    pub async fn stop(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(h) = self.handle.take() {
            let _ = h.await;
        }
    }

    // -- routing --

    fn router(state: Arc<SharedState>) -> Router {
        Router::new()
            // OpenAI-compatible JSON endpoints
            .route("/v1/chat/completions", post(chat_completions))
            .route("/v1/models", get(models))
            // SSE streaming
            .route("/v1/chat/completions/stream", post(stream_chat_completions))
            // Utility
            .route("/echo", post(echo))
            .route("/status/{code}", get(status))
            // Error simulation
            .route("/error/timeout", get(error_timeout))
            .route("/error/disconnect", get(error_disconnect))
            .route("/error/500", get(error_500))
            .route("/error/slow-body", post(error_slow_body))
            // WebSocket (future use)
            .route("/ws/echo", get(ws_echo))
            .route("/ws/stream", get(ws_stream))
            // WebTransport stub (future use)
            .route("/wt/stub", get(wt_stub))
            .with_state(state)
    }
}

// ---------------------------------------------------------------------------
// OpenAI-compatible HTTP JSON handlers
// ---------------------------------------------------------------------------

async fn chat_completions(
    State(state): State<Arc<SharedState>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    state
        .record("POST", &uri.to_string(), &headers, &body)
        .await;

    let resp = json!({
        "id": "chatcmpl-mock-123",
        "object": "chat.completion",
        "created": 1_234_567_890_u64,
        "model": "gpt-4-mock",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "Hello from mock server"
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 20,
            "total_tokens": 30
        }
    });
    (StatusCode::OK, axum::Json(resp))
}

async fn models(
    State(state): State<Arc<SharedState>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> impl IntoResponse {
    state.record("GET", &uri.to_string(), &headers, &[]).await;

    let resp = json!({
        "object": "list",
        "data": [
            {"id": "gpt-4", "object": "model", "created": 1_234_567_890_u64, "owned_by": "openai"},
            {"id": "gpt-3.5-turbo", "object": "model", "created": 1_234_567_890_u64, "owned_by": "openai"}
        ]
    });
    (StatusCode::OK, axum::Json(resp))
}

// ---------------------------------------------------------------------------
// SSE streaming handler
// ---------------------------------------------------------------------------

async fn stream_chat_completions(
    State(state): State<Arc<SharedState>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    state
        .record("POST", &uri.to_string(), &headers, &body)
        .await;

    let words = ["Hello", " from", " mock", " server"];

    let stream = async_stream::stream! {
        for (i, word) in words.iter().enumerate() {
            let mut delta = serde_json::Map::new();
            if i == 0 {
                delta.insert("role".into(), Value::String("assistant".into()));
            }
            delta.insert("content".into(), Value::String((*word).into()));

            let chunk = json!({
                "id": "chatcmpl-mock-stream",
                "object": "chat.completion.chunk",
                "created": 1_234_567_890_u64,
                "model": "gpt-4-mock",
                "choices": [{
                    "index": 0,
                    "delta": delta,
                    "finish_reason": Value::Null
                }]
            });
            yield Ok(Event::default().data(chunk.to_string()));
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // Final chunk with finish_reason
        let final_chunk = json!({
            "id": "chatcmpl-mock-stream",
            "object": "chat.completion.chunk",
            "created": 1_234_567_890_u64,
            "model": "gpt-4-mock",
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        });
        yield Ok(Event::default().data(final_chunk.to_string()));

        // OpenAI DONE sentinel
        yield Ok(Event::default().data("[DONE]"));
    };

    Sse::new(stream)
}

// ---------------------------------------------------------------------------
// Utility handlers
// ---------------------------------------------------------------------------

async fn echo(
    State(state): State<Arc<SharedState>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    state
        .record("POST", &uri.to_string(), &headers, &body)
        .await;

    let hdrs: serde_json::Map<String, Value> = headers
        .iter()
        .map(|(k, v)| {
            (
                k.to_string(),
                Value::String(v.to_str().unwrap_or("").to_string()),
            )
        })
        .collect();
    let body_str = String::from_utf8_lossy(&body);

    let resp = json!({
        "headers": hdrs,
        "body": body_str,
    });
    (StatusCode::OK, axum::Json(resp))
}

async fn status(
    State(state): State<Arc<SharedState>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(code): Path<u16>,
) -> impl IntoResponse {
    state.record("GET", &uri.to_string(), &headers, &[]).await;

    let sc = StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let resp = json!({
        "status": code,
        "description": sc.canonical_reason().unwrap_or("Unknown"),
    });
    (sc, axum::Json(resp))
}

// ---------------------------------------------------------------------------
// Error simulation handlers
// ---------------------------------------------------------------------------

async fn error_timeout(
    State(state): State<Arc<SharedState>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> impl IntoResponse {
    state.record("GET", &uri.to_string(), &headers, &[]).await;
    // Sleep indefinitely so the client times out
    tokio::time::sleep(Duration::from_secs(3600)).await;
    StatusCode::OK
}

async fn error_disconnect(
    State(state): State<Arc<SharedState>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> axum::response::Response {
    state.record("GET", &uri.to_string(), &headers, &[]).await;

    // Stream partial data then abort with an error
    let stream = async_stream::stream! {
        yield Ok::<_, std::io::Error>(Bytes::from("partial response data..."));
        tokio::time::sleep(Duration::from_millis(50)).await;
        yield Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionAborted,
            "simulated disconnect",
        ));
    };

    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/plain")
        .body(axum::body::Body::from_stream(stream))
        .expect("response builder should not fail")
}

async fn error_500(
    State(state): State<Arc<SharedState>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> impl IntoResponse {
    state.record("GET", &uri.to_string(), &headers, &[]).await;

    let resp = json!({
        "error": {
            "message": "Internal server error",
            "type": "server_error",
            "code": "internal_error"
        }
    });
    (StatusCode::INTERNAL_SERVER_ERROR, axum::Json(resp))
}

async fn error_slow_body(
    State(state): State<Arc<SharedState>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> axum::response::Response {
    state
        .record("POST", &uri.to_string(), &headers, &body)
        .await;

    let stream = async_stream::stream! {
        yield Ok::<_, std::io::Error>(Bytes::from("chunk1\n"));
        tokio::time::sleep(Duration::from_millis(10)).await;
        yield Ok(Bytes::from("chunk2\n"));
        // Stall forever
        tokio::time::sleep(Duration::from_secs(3600)).await;
        yield Ok(Bytes::from("should never arrive"));
    };

    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/plain")
        .body(axum::body::Body::from_stream(stream))
        .expect("response builder should not fail")
}

// ---------------------------------------------------------------------------
// WebSocket handlers (future use — OAGW WS proxy not in this phase)
// ---------------------------------------------------------------------------

async fn ws_echo(
    State(state): State<Arc<SharedState>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    state.record("GET", &uri.to_string(), &headers, &[]).await;
    ws.on_upgrade(handle_ws_echo)
}

async fn handle_ws_echo(mut socket: WebSocket) {
    while let Some(Ok(msg)) = socket.recv().await {
        match msg {
            Message::Text(text) => {
                if socket.send(Message::Text(text)).await.is_err() {
                    break;
                }
            }
            Message::Binary(data) => {
                if socket.send(Message::Binary(data)).await.is_err() {
                    break;
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
}

async fn ws_stream(
    State(state): State<Arc<SharedState>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    state.record("GET", &uri.to_string(), &headers, &[]).await;
    ws.on_upgrade(handle_ws_stream)
}

async fn handle_ws_stream(mut socket: WebSocket) {
    for i in 0..5_u32 {
        let msg = json!({"seq": i, "data": format!("message {i}")});
        if socket
            .send(Message::Text(msg.to_string().into()))
            .await
            .is_err()
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let _ = socket.send(Message::Close(None)).await;
}

// ---------------------------------------------------------------------------
// WebTransport stub (future use)
// ---------------------------------------------------------------------------

async fn wt_stub(
    State(state): State<Arc<SharedState>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> impl IntoResponse {
    state.record("GET", &uri.to_string(), &headers, &[]).await;

    let resp = json!({
        "error": "WebTransport is not implemented",
        "description": "Placeholder for future WebTransport support"
    });
    (StatusCode::NOT_IMPLEMENTED, axum::Json(resp))
}
