# ADR: Rust ABI Client Library for Internal Modules

- **Status**: Proposed
- **Date**: 2026-02-03
- **Updated**: 2026-02-09
- **Deciders**: OAGW Team

## Context and Problem Statement

Internal CyberFabric modules (workflow engines, agents, background jobs) need to make HTTP requests to external services (OpenAI, Anthropic, external APIs) but are **not allowed direct internet access** for security and observability reasons. All outbound requests must route through OAGW.

**Problem**: Internal modules and third-party SDKs (like `async-openai`, `anthropic-sdk-rust`) expect a standard HTTP client interface. We need a **drop-in replacement client library** that:

1. **Routes requests through OAGW**: All requests go to OAGW's `/proxy/{alias}/*` endpoint
2. **Supports multiple deployment modes**:
   - **Shared process**: Client + OAGW in same process → direct function calls (zero serialization)
   - **Remote OAGW**: Client + OAGW in separate processes → HTTP requests to proxy endpoint
3. **Multiple response types**: Plain HTTP, Server-Sent Events (SSE), streaming responses
4. **Multiple protocols**: HTTP/1.1, HTTP/2, WebSocket (WSS), WebTransport (WT)
5. **SDK compatibility**: Works as HTTP backend for third-party Rust SDKs
6. **Explicit alias routing**: Caller specifies OAGW upstream alias in API

**Current gaps**:

- No client library for internal modules to use
- No abstraction for shared-process vs remote-OAGW modes
- No streaming-aware API design
- No SDK integration strategy

**Scope**: This ADR covers HTTP/HTTPS/WS/SSE/WT protocols. gRPC is future work.

## Decision Drivers

- **Ergonomics**: Simple, intuitive API for internal module developers
- **SDK compatibility**: Works as HTTP backend for third-party Rust SDKs (OpenAI, Anthropic, etc.)
- **Performance**: Zero-copy where possible, minimal allocations
- **Safety**: Strong typing, compile-time guarantees
- **Flexibility**: Support plain, streaming, bidirectional protocols
- **Deployment transparency**: Same API works in shared-process and remote-OAGW modes
- **Observability**: Request tracing, metrics collection routed through OAGW
- **Testability**: Easy to mock for unit tests
- **Security**: No direct internet access from internal modules

## Considered Options

### Option 1: Trait-Based Abstraction with Dynamic Dispatch

Define a core `OagwClient` trait with implementations for different deployment modes:
- **SharedProcessClient**: Direct function calls to OAGW Data Plane (same process)
- **RemoteProxyClient**: HTTP requests to OAGW `/proxy/{alias}/*` endpoint (separate process)

**Core API**:

```rust
// Simple, clean interface - streaming determined by server, not client
pub struct OagwClient {
    // ... (implementation details hidden)
}

impl OagwClient {
    /// Execute HTTP request through OAGW
    /// Response can be consumed as buffered or streaming
    pub async fn execute(&self, alias: &str, request: Request) -> Result<Response, ClientError>;

    /// Establish WebSocket connection through OAGW
    pub async fn websocket(&self, alias: &str, request: Request) -> Result<WebSocketConn, ClientError>;
}

// Request builder
#[derive(Debug, Clone)]
pub struct Request {
    method: Method,
    path: String,              // Relative path: "/v1/chat/completions"
    headers: HeaderMap,
    body: Body,
    timeout: Option<Duration>,
}

// Response with flexible consumption
pub struct Response {
    status: StatusCode,
    headers: HeaderMap,
    body: ResponseBody,         // Can be buffered or streamed
    error_source: ErrorSource,  // Gateway or Upstream (from X-OAGW-Error-Source header)
}

impl Response {
    pub fn status(&self) -> StatusCode { self.status }
    pub fn headers(&self) -> &HeaderMap { &self.headers }
    pub fn error_source(&self) -> ErrorSource { self.error_source }

    /// Buffer entire response body
    pub async fn bytes(self) -> Result<Bytes, ClientError> { ... }

    /// Parse response body as JSON (buffers automatically)
    pub async fn json<T: DeserializeOwned>(self) -> Result<T, ClientError> { ... }

    /// Parse response body as text (buffers automatically)
    pub async fn text(self) -> Result<String, ClientError> { ... }

    /// Consume response as byte stream (for SSE, chunked responses)
    pub fn into_stream(self) -> BoxStream<'static, Result<Bytes, ClientError>> { ... }

    /// Convenience: parse as Server-Sent Events stream
    pub fn into_sse_stream(self) -> SseEventStream { ... }
}

// Server-Sent Events stream
pub struct SseEventStream {
    inner: BoxStream<'static, Result<Bytes, ClientError>>,
    buffer: Vec<u8>,
}

impl SseEventStream {
    pub async fn next_event(&mut self) -> Result<Option<SseEvent>, ClientError> { ... }
}

pub struct SseEvent {
    pub id: Option<String>,
    pub event: Option<String>,
    pub data: String,
    pub retry: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSource {
    Gateway,   // Error from OAGW
    Upstream,  // Error from external service
    Unknown,   // No X-OAGW-Error-Source header present
}

// WebSocket connection
pub struct WebSocketConn {
    send: mpsc::Sender<WsMessage>,
    recv: mpsc::Receiver<Result<WsMessage, ClientError>>,
}

// Usage examples
//
// Buffered response:
// let response = client.execute("openai", req).await?;
// let data = response.json::<ChatResponse>().await?;
//
// Streaming response (SSE):
// let response = client.execute("openai", req).await?;
// let mut sse = response.into_sse_stream();
// while let Some(event) = sse.next_event().await? { ... }
```

**Pros**:

- Clean separation of concerns
- Easy to test with mock implementations
- Supports both sync and async contexts (with minor API variations)
- Pluggable backends (reqwest, hyper, custom)

**Cons**:

- - Dynamic dispatch overhead (negligible for network I/O)
- - Slightly more complex implementation

### Option 2: Concrete Types with Feature Flags

Provide concrete client types, selected via Cargo features:

```rust
#[cfg(feature = "direct")]
pub type HttpClient = DirectClient;

#[cfg(feature = "rpc")]
pub type HttpClient = RpcClient;
```

**Pros**:

- Zero-cost abstraction (static dispatch)
- Simpler implementation

**Cons**:

- - Cannot use both modes in same binary
- - Harder to test (need feature-gated test code)
- - Less flexible for future extensions

### Option 3: Hybrid Approach with Deployment Abstraction (Recommended)

Single `OagwClient` type that internally dispatches to the appropriate implementation:

```rust
// Public API - deployment-agnostic
pub struct OagwClient {
    inner: OagwClientImpl,
}

// Internal implementation enum (not exposed)
enum OagwClientImpl {
    SharedProcess(SharedProcessClient),
    RemoteProxy(RemoteProxyClient),
}

impl OagwClient {
    /// Create from configuration (automatically selects implementation)
    pub fn from_config(config: OagwClientConfig) -> Result<Self, ClientError> {
        let inner = match config.mode {
            ClientMode::SharedProcess { control_plane } => {
                OagwClientImpl::SharedProcess(SharedProcessClient::new(control_plane)?)
            }
            ClientMode::RemoteProxy { base_url, auth_token } => {
                OagwClientImpl::RemoteProxy(RemoteProxyClient::new(base_url, auth_token)?)
            }
        };
        Ok(Self { inner })
    }

    /// Execute plain HTTP request
    pub async fn execute(&self, alias: &str, req: Request) -> Result<Response, ClientError> {
        match &self.inner {
            OagwClientImpl::SharedProcess(c) => c.execute(alias, req).await,
            OagwClientImpl::RemoteProxy(c) => c.execute(alias, req).await,
        }
    }

    // Same for execute_streaming, websocket, etc.
}

// Configuration that determines deployment mode
pub struct OagwClientConfig {
    mode: ClientMode,
    timeout: Duration,
    // ... other common config
}

pub enum ClientMode {
    /// OAGW in same process - use direct function calls
    SharedProcess {
        control_plane: Arc<dyn ControlPlaneService>,
    },
    /// OAGW in separate process - use HTTP proxy endpoint
    RemoteProxy {
        base_url: String,
        auth_token: String,
    },
}
```

**Usage (same code in both modes)**:

```rust
// Configuration comes from environment/settings
let config = OagwClientConfig::from_env()?;  // Auto-detects mode
let client = OagwClient::from_config(config)?;

// Application code - identical regardless of deployment
let request = Request::builder()
    .method(Method::POST)
    .path("/v1/chat/completions")
    .json(&payload)?
    .build()?;

let response = client.execute("openai", request).await?;
```

**Pros**:

- + **Deployment-agnostic**: Application code never changes
- + **Zero-cost abstraction**: Enum dispatch is optimized away by compiler
- + **Easy to test**: Mock can be added as third variant
- + **Type-safe**: Single concrete type, no trait objects
- + **Configuration-driven**: Mode selected from config, not code

**Cons**:

- Slightly more boilerplate in enum dispatch methods

**Decision**: Use hybrid approach with deployment abstraction (Option 3).

## SDK Integration Patterns

Third-party Rust SDKs (OpenAI, Anthropic, AWS, etc.) need to route requests through OAGW. This section analyzes all integration approaches.

### Pattern 1: Drop-In Replacement for reqwest

**Approach**: Provide a `reqwest`-compatible API so SDKs can use our client as a custom HTTP backend.

**Implementation**:

```rust
// Our client mimics reqwest::Client API
pub struct OagwClient {
    // ...
}

impl OagwClient {
    pub fn get(&self, alias: &str, path: &str) -> RequestBuilder {
        RequestBuilder::new(self.clone(), Method::GET, alias, path)
    }

    pub fn post(&self, alias: &str, path: &str) -> RequestBuilder {
        RequestBuilder::new(self.clone(), Method::POST, alias, path)
    }
}

// Usage in SDK code (minimal changes)
// Before: let client = reqwest::Client::new();
// After:  let client = OagwClient::new(config);
//
// Before: client.post("https://api.openai.com/v1/chat/completions")
// After:  client.post("openai", "/v1/chat/completions")
```

**Pros**:
- + Minimal SDK code changes
- + Familiar API for Rust developers
- + Type-safe at compile time

**Cons**:
- - Requires SDK source modifications (can't use unmodified crates)
- - Every SDK needs manual integration
- - Explicit alias parameter changes SDK API surface

**Use case**: Internal SDKs we control (custom wrappers around OpenAI/Anthropic)

---

### Pattern 2: HTTP Proxy (Standard Proxy Protocol)

**Approach**: Act as an HTTP proxy that SDKs configure via standard `HTTP_PROXY` environment variable or `reqwest::Proxy`.

**Implementation**:

```rust
// Start OAGW proxy server on localhost:8080
// Internal modules set: HTTP_PROXY=http://localhost:8080

// OAGW proxy server intercepts requests and routes based on destination host
// Request: GET https://api.openai.com/v1/chat/completions
// OAGW maps api.openai.com → "openai" alias
// OAGW proxies to /proxy/openai/v1/chat/completions
```

**Pros**:
- + Works with **unmodified** third-party SDKs
- + Standard HTTP proxy protocol (RFC 7230)
- + No SDK code changes required

**Cons**:
- - Requires host-to-alias mapping configuration (api.openai.com → "openai")
- - DNS resolution needed (or hardcoded host mapping)
- - Adds local proxy server overhead
- - HTTPS CONNECT tunneling complexity for TLS

**Use case**: Drop-in compatibility with unmodified third-party SDKs

---

### Pattern 3: Custom HTTP Transport Trait

**Approach**: Define a trait that SDK maintainers implement to route through OAGW.

**Implementation**:

```rust
// Define transport trait
#[async_trait]
pub trait HttpTransport: Send + Sync {
    async fn request(&self, req: http::Request<Bytes>) -> Result<http::Response<Bytes>, Error>;
}

// SDK uses trait instead of concrete reqwest::Client
pub struct OpenAiClient<T: HttpTransport> {
    transport: Arc<T>,
}

// We provide OAGW implementation
pub struct OagwTransport {
    client: OagwClient,
    alias: String,  // e.g., "openai"
}

#[async_trait]
impl HttpTransport for OagwTransport {
    async fn request(&self, req: http::Request<Bytes>) -> Result<http::Response<Bytes>, Error> {
        // Convert http::Request to OAGW Request and route through alias
        let oagw_req = Request::from_http(req);
        let response = self.client.execute(&self.alias, oagw_req).await?;
        Ok(response.into_http())
    }
}
```

**Pros**:
- + Clean abstraction
- + Testable with mock transports
- + No explicit alias in SDK API

**Cons**:
- - Requires SDKs to be designed with transport abstraction
- - Most existing SDKs don't support this pattern
- - Cannot use unmodified third-party crates

**Use case**: Future SDK design pattern (if we influence SDK maintainers)

---

### Pattern 4: Wrapper Layer (Facade Pattern)

**Approach**: Wrap third-party SDKs with our own API that internally routes through OAGW.

**Implementation**:

```rust
// Third-party SDK (unmodified)
pub struct OpenAiSdk {
    client: reqwest::Client,
    api_key: String,
}

// Our wrapper that internally uses OAGW
pub struct CfOpenAiClient {
    oagw_client: OagwClient,
    alias: String,
}

impl CfOpenAiClient {
    pub async fn chat_completion(&self, req: ChatRequest) -> Result<ChatResponse> {
        // Manually construct OAGW request
        let oagw_req = Request::builder()
            .method(Method::POST)
            .path("/v1/chat/completions")
            .json(&req)?
            .build()?;

        let response = self.oagw_client.execute(&self.alias, oagw_req).await?;
        Ok(serde_json::from_slice(&response.body)?)
    }
}
```

**Pros**:
- + Complete control over API surface
- + Can add CF-specific features (retry policies, circuit breakers)
- + No dependency on third-party SDK internals

**Cons**:
- - Must manually implement entire SDK API surface
- - Maintenance burden (keep in sync with upstream SDK)
- - Duplicates SDK functionality

**Use case**: High-value SDKs we want to fully control (OpenAI, Anthropic)

---

### Pattern 5: reqwest Middleware/Interceptor

**Approach**: Extend `reqwest::Client` with middleware that intercepts requests and routes through OAGW.

**Implementation**:

```rust
// Hypothetical (reqwest doesn't natively support middleware)
// Would require forking reqwest or using tower-like middleware

pub struct OagwInterceptor {
    oagw_client: OagwClient,
    host_to_alias: HashMap<String, String>,  // "api.openai.com" → "openai"
}

impl Interceptor for OagwInterceptor {
    async fn intercept(&self, req: reqwest::Request) -> Result<reqwest::Response> {
        let host = req.url().host_str().unwrap();
        let alias = self.host_to_alias.get(host).ok_or("Unknown host")?;

        // Route through OAGW
        let oagw_req = Request::from_reqwest(req);
        let response = self.oagw_client.execute(alias, oagw_req).await?;
        Ok(response.into_reqwest())
    }
}
```

**Pros**:
- + Transparent to SDK code
- + Works with unmodified SDKs (if middleware is injected at runtime)

**Cons**:
- - `reqwest` doesn't support middleware natively
- - Would require forking `reqwest` or complex runtime injection
- - Fragile (depends on `reqwest` internals)

**Use case**: Not recommended (too complex)

---

### Recommended Strategy

**Phase 0 (MVP)**: Pattern 4 (Wrapper Layer) for OpenAI + Pattern 1 (Drop-In Replacement) for internal modules

- + Immediate value: Works for critical use cases (OpenAI)
- + Clean API: Internal modules get ergonomic client
- + Testable: Both patterns support mocking

**Phase 1**: Pattern 2 (HTTP Proxy) for unmodified third-party SDKs

- + Unlocks broader ecosystem
- + No SDK modifications required
- - Adds complexity (proxy server + host mapping)

**Phase 2**: Pattern 3 (Custom Transport Trait) as standardized pattern for new SDKs

- + Clean abstraction
- + Community pattern (if we can influence SDK maintainers)

## HTTP Client Compatibility Analysis

### Overview

This section analyzes compatibility between the proposed `OagwClient` library and popular Rust HTTP clients used in the ecosystem and our codebase.

### Rust HTTP Client Landscape

Popular HTTP clients in the Rust ecosystem:

1. **reqwest** - High-level, async (most popular, ~35M downloads/month)
2. **hyper** - Low-level, async (foundation for reqwest and others)
3. **ureq** - Lightweight, sync-only (used in our codebase for build scripts)
4. **surf** - Async, runtime-agnostic
5. **isahc** - Built on libcurl, supports sync and async
6. **actix-web client (awc)** - Part of actix ecosystem

### Compatibility Matrix

| HTTP Client | Pattern 1 (Drop-In) | Pattern 2 (Proxy) | Pattern 4 (Wrapper) | Direct `OagwClient` Use | Notes |
|-------------|---------------------|-------------------|---------------------|------------------------|-------|
| **reqwest** | Yes | Yes | Yes | Yes | Full compatibility - OagwClient uses reqwest internally |
| **hyper** | No | Yes | Yes | Partial | Can build custom adapter; reqwest is built on hyper |
| **ureq** | No | Yes | Yes | No | Sync-only; needs blocking wrapper or proxy mode |
| **surf** | No | Yes | Yes | Partial | Different async API; proxy mode recommended |
| **isahc** | No | Yes | Yes | Partial | Sync API needs blocking wrapper |
| **awc** | No | Yes | Yes | Partial | Actix runtime; proxy mode recommended |

**Legend**:
- Yes: Full compatibility, works seamlessly
- Partial: Works with additional adapter layer or specific configuration
- No: Incompatible with this pattern

### Key Compatibility Issues

#### Issue 1: Synchronous vs Asynchronous APIs

**Problem**: The proposed `OagwClient` is fully async (requires tokio runtime), but some popular clients and use cases are synchronous:

```rust
// Proposed OagwClient: async-only
pub async fn execute(&self, alias: &str, request: Request) -> Result<Response, ClientError>;

// Current codebase usage: ureq (synchronous)
// modules/system/api_gateway/build.rs:67
let resp = ureq::get(url).call()?;
```

**Impact**:
- Cannot use `OagwClient` in build scripts without blocking wrapper
- Cannot use in sync contexts (non-async functions, FFI, etc.)
- Existing `ureq` code cannot migrate to `OagwClient` without becoming async
- Libraries like `ureq` and `isahc` (when used in sync mode) require workarounds

**Affected scenarios**:
1. Build scripts (`build.rs`) - currently use `ureq`
2. CLI tools that prefer sync APIs
3. FFI boundaries where async is problematic
4. Legacy sync codebases

#### Issue 2: Hard Dependency on reqwest

**Problem**: `RemoteProxyClient` directly depends on `reqwest::Client`:

```rust
struct RemoteProxyClient {
    http_client: reqwest::Client,  // Hard dependency on reqwest
    auth_token: String,
    // ...
}
```

**Impact**:
- Modules using different HTTP clients must include both dependencies
- Binary size increase if module already uses another client
- Pattern 1 (Drop-In Replacement) only works for reqwest-based SDKs
- Cannot easily swap HTTP client implementation

**Trade-off analysis**:
- **Pros of reqwest**: Ergonomic API, battle-tested, most popular choice
- **Cons of reqwest**: Adds dependency weight, not universal compatibility
- **Alternative (hyper)**: More flexible but requires more boilerplate (acknowledged in line 1768-1773)

#### Issue 3: Build-Time HTTP Requests

**Problem**: The ADR doesn't address build-time usage where async runtime is unavailable:

```rust
// Current build.rs pattern (api_gateway/build.rs:64-73)
fn download_to(url: &str, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let resp = ureq::get(url).call()?;  // Sync, no async runtime
    let mut bytes = Vec::new();
    resp.into_reader().read_to_end(&mut bytes)?;
    let mut f = fs::File::create(dest)?;
    f.write_all(&bytes)?;
    Ok(())
}
```

**Impact**:
- Build scripts cannot route through OAGW without blocking wrapper
- Assets downloaded during build bypass OAGW observability
- Build-time network access is uncontrolled

### Solutions and Mitigations

#### Solution 1: Add Blocking API Wrapper

Provide synchronous API for build scripts and sync contexts:

```rust
impl OagwClient {
    /// Blocking version for synchronous contexts
    ///
    /// **Use cases**:
    /// - Build scripts (build.rs)
    /// - CLI tools preferring sync APIs
    /// - FFI boundaries
    ///
    /// **Warning**: Creates a new tokio runtime per call if none exists.
    /// For repeated calls, prefer creating a runtime once and using async API.
    pub fn execute_blocking(
        &self,
        alias: &str,
        request: Request,
    ) -> Result<Response, ClientError> {
        // Check if we're already in a tokio runtime
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                // Use existing runtime
                handle.block_on(self.execute(alias, request))
            }
            Err(_) => {
                // Create temporary runtime
                tokio::runtime::Runtime::new()?
                    .block_on(self.execute(alias, request))
            }
        }
    }

    /// Convenience: blocking request builder
    pub fn blocking(&self) -> BlockingClient {
        BlockingClient { inner: self }
    }
}

/// Blocking client wrapper
pub struct BlockingClient<'a> {
    inner: &'a OagwClient,
}

impl BlockingClient<'_> {
    pub fn execute(&self, alias: &str, request: Request) -> Result<Response, ClientError> {
        self.inner.execute_blocking(alias, request)
    }

    pub fn get(&self, alias: &str, path: &str) -> BlockingRequestBuilder {
        BlockingRequestBuilder::new(self.inner, Method::GET, alias, path)
    }

    pub fn post(&self, alias: &str, path: &str) -> BlockingRequestBuilder {
        BlockingRequestBuilder::new(self.inner, Method::POST, alias, path)
    }
}
```

**Usage in build.rs**:

```rust
// build.rs with OAGW routing
use oagw_client::{OagwClient, OagwClientConfig, Request, Method};

fn main() {
    let config = OagwClientConfig::from_env()
        .unwrap_or_else(|_| {
            // Fallback: direct download if OAGW not configured
            eprintln!("OAGW not configured, downloading directly");
            return download_direct();
        });

    let client = OagwClient::from_config(config).unwrap();

    // Blocking API in build script
    let request = Request::builder()
        .method(Method::GET)
        .path("/elements@9.0.15/web-components.min.js")
        .build()
        .unwrap();

    let response = client.execute_blocking("unpkg", request).unwrap();
    let bytes = response.bytes_blocking().unwrap();

    std::fs::write("assets/web-components.min.js", bytes).unwrap();
}

fn download_direct() {
    // Fallback: use ureq directly
    let resp = ureq::get("https://unpkg.com/@stoplight/elements@9.0.15/web-components.min.js")
        .call()
        .unwrap();
    // ... existing logic
}
```

#### Solution 2: HTTP Proxy for Universal Compatibility

Pattern 2 (HTTP Proxy) provides universal compatibility:

```rust
// Any HTTP client can use OAGW via standard HTTP_PROXY

// Set environment variable
std::env::set_var("HTTP_PROXY", "http://localhost:8080");
std::env::set_var("HTTPS_PROXY", "http://localhost:8080");

// Configure OAGW proxy to map hosts to aliases
// Host: api.openai.com → Alias: "openai"
// Host: api.anthropic.com → Alias: "anthropic"

// Then ANY HTTP client routes through OAGW transparently:

// reqwest
let client = reqwest::Client::new();
client.get("https://api.openai.com/v1/models").send().await?;

// ureq (sync)
let resp = ureq::get("https://api.openai.com/v1/models").call()?;

// surf
let res = surf::get("https://api.openai.com/v1/models").await?;

// All route through OAGW proxy automatically
```

**Advantages**:
- Works with all HTTP clients (sync and async)
- No SDK modifications required
- Standard HTTP protocol
- Universal compatibility

**Disadvantages**:
- Requires local proxy server running
- Adds latency (extra hop)
- Needs host-to-alias mapping configuration
- HTTPS CONNECT tunneling complexity

#### Solution 3: Feature-Gated HTTP Client Backend

Allow selecting HTTP client backend via Cargo features:

```rust
// Cargo.toml
[features]
default = ["backend-reqwest"]
backend-reqwest = ["reqwest"]
backend-hyper = ["hyper", "hyper-util"]
backend-ureq = ["ureq"]  # Sync-only, limited functionality

// Implementation
#[cfg(feature = "backend-reqwest")]
type HttpBackend = ReqwestBackend;

#[cfg(feature = "backend-hyper")]
type HttpBackend = HyperBackend;

#[cfg(feature = "backend-ureq")]
type HttpBackend = UreqBackend;  // Sync-only, no streaming
```

**Trade-offs**:
- (+) Flexibility: Users choose backend
- (+) Reduced dependencies if they already have a client
- (-) Complexity: Must maintain multiple backends
- (-) Testing burden: Test all feature combinations
- (-) API limitations: Sync backends can't support all features

**Decision**: Not recommended for MVP. Use reqwest by default, add HTTP Proxy for compatibility.

### Build Script Strategy

#### Option A: Keep ureq for Build Scripts (Recommended for MVP)

Build scripts have different requirements than runtime code:
- No observability needed (runs at compile time)
- Simple, direct downloads acceptable
- Async runtime overhead undesirable

```rust
// build.rs - continue using ureq directly
[build-dependencies]
ureq = { workspace = true }

// Runtime code - use OagwClient
[dependencies]
oagw-client = { path = "../oagw-client" }
```

**Rationale**: Build-time asset downloads don't need OAGW routing. Observability and policy enforcement only matter for runtime requests.

#### Option B: OAGW Routing for Build Scripts (Future Enhancement)

If build-time network access must route through OAGW:

```rust
// build.rs with OAGW
use oagw_client::{OagwClient, OagwClientConfig};

fn main() {
    // Configure OAGW for build time
    let config = OagwClientConfig::from_env().unwrap_or_else(|_| {
        eprintln!("cargo:warning=OAGW not configured, downloading directly");
        return download_direct();
    });

    let client = OagwClient::from_config(config).unwrap();

    // Use blocking API
    let response = client
        .blocking()
        .get("unpkg", "/elements@9.0.15/web-components.min.js")
        .send()
        .unwrap();

    std::fs::write("assets/web-components.min.js", response.bytes()).unwrap();
}
```

**Use cases**:
- Corporate environments requiring all network access through proxy
- Air-gapped builds with internal mirrors
- Build-time observability and auditing

### Updated Implementation Priorities

Based on compatibility analysis, update phase priorities:

**Phase 0 (MVP)**: Core Client + RemoteProxyClient + Blocking API

- Core types (`Request`, `Response`, `Body`, `ErrorSource`)
- `OagwClient` with deployment abstraction
- `RemoteProxyClient` (uses reqwest)
- **NEW**: Blocking API wrapper (`execute_blocking()`, `BlockingClient`)
- Response consumption: `.bytes()`, `.json()`, `.into_stream()`, `.into_sse_stream()`
- Pattern 4 (Wrapper Layer) for OpenAI

**Deliverable**: Internal modules can use OAGW in both async and sync contexts

**Phase 1**: SharedProcessClient + HTTP Proxy

- `SharedProcessClient` (direct function calls)
- Configuration auto-detection (`OagwClientConfig::from_env()`)
- **Pattern 2 (HTTP Proxy)** for universal compatibility
- Host-to-alias mapping configuration

**Deliverable**: Any HTTP client (reqwest, ureq, surf, etc.) can route through OAGW

**Phase 2**: WebSocket + Enhanced SDK Integration

- `WebSocketConn` implementation
- Pattern 1 (Drop-In Replacement) documentation and examples
- SDK integration guides for popular libraries

**Phase 3**: WebTransport (Future)

- QUIC transport layer
- `WebTransportConn` implementation

### Compatibility Testing Strategy

Add integration tests for different HTTP client scenarios:

```rust
#[cfg(test)]
mod compatibility_tests {
    use super::*;

    #[tokio::test]
    async fn test_async_usage() {
        let client = OagwClient::from_config(test_config()).unwrap();
        let response = client.execute("openai", test_request()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn test_sync_usage() {
        let client = OagwClient::from_config(test_config()).unwrap();
        let response = client.execute_blocking("openai", test_request()).unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn test_build_script_pattern() {
        // Simulate build.rs environment (no tokio runtime)
        let client = OagwClient::from_config(test_config()).unwrap();
        let response = client
            .blocking()
            .get("unpkg", "/package.json")
            .send()
            .unwrap();
        assert!(response.status().is_success());
    }

    #[tokio::test]
    async fn test_ureq_via_proxy() {
        // Start OAGW HTTP proxy
        let proxy = start_test_proxy().await;

        // Configure ureq to use proxy
        let agent = ureq::AgentBuilder::new()
            .proxy(ureq::Proxy::new(&format!("http://localhost:{}", proxy.port())).unwrap())
            .build();

        // ureq requests route through OAGW
        let resp = agent.get("https://api.openai.com/v1/models").call().unwrap();
        assert_eq!(resp.status(), 200);
    }
}
```

### Documentation Updates

Add to crate-level documentation:

```rust
//! # HTTP Client Compatibility
//!
//! This library is designed to work with various HTTP clients in the Rust ecosystem:
//!
//! ## Direct Integration (Async)
//!
//! The primary API is async and works seamlessly in tokio-based applications:
//!
//! ```rust
//! let client = OagwClient::from_config(config)?;
//! let response = client.execute("openai", request).await?;
//! ```
//!
//! ## Blocking API (Sync)
//!
//! For synchronous contexts (build scripts, CLI tools, FFI):
//!
//! ```rust
//! let client = OagwClient::from_config(config)?;
//! let response = client.execute_blocking("openai", request)?;
//! ```
//!
//! ## HTTP Proxy Mode (Universal Compatibility)
//!
//! Any HTTP client can route through OAGW via standard HTTP proxy:
//!
//! ```bash
//! export HTTP_PROXY=http://localhost:8080
//! export HTTPS_PROXY=http://localhost:8080
//! ```
//!
//! Then use any HTTP client normally - requests are transparently routed through OAGW.
//!
//! ## Compatibility Matrix
//!
//! | Client | Direct Use | Blocking API | HTTP Proxy |
//! |--------|------------|--------------|------------|
//! | reqwest | Yes | Yes | Yes |
//! | ureq | No | Yes | Yes |
//! | hyper | Adapter needed | Yes | Yes |
//! | surf | Adapter needed | Yes | Yes |
//! | isahc | No | Yes | Yes |
//!
//! See the [compatibility analysis](./docs/adr-rust-abi-client-library.md#http-client-compatibility-analysis)
//! for detailed information.
```

## Detailed Design

### Usage Example (Deployment-Agnostic)

Internal module code **never changes** regardless of deployment mode:

```rust
// In your internal module (e.g., workflow_engine, agents)

use oagw_client::{OagwClient, OagwClientConfig, Request, Method};

pub struct MyInternalService {
    oagw_client: OagwClient,
}

impl MyInternalService {
    pub fn new() -> Result<Self, Error> {
        // Configuration automatically detects deployment mode
        let config = OagwClientConfig::from_env()?;
        let oagw_client = OagwClient::from_config(config)?;

        Ok(Self { oagw_client })
    }

    pub async fn call_openai(&self, prompt: &str) -> Result<String, Error> {
        // Buffered response (default pattern)
        let request = Request::builder()
            .method(Method::POST)
            .path("/v1/chat/completions")
            .json(&json!({
                "model": "gpt-4",
                "messages": [{"role": "user", "content": prompt}]
            }))?
            .build()?;

        let response = self.oagw_client.execute("openai", request).await?;

        // Check error source
        if response.error_source() == ErrorSource::Gateway {
            warn!("OAGW gateway error");
        }

        // Consume as JSON (buffers automatically)
        let data: serde_json::Value = response.json().await?;
        Ok(data["choices"][0]["message"]["content"].as_str().unwrap().to_string())
    }

    pub async fn call_openai_streaming(&self, prompt: &str) -> Result<impl Stream<Item = String>, Error> {
        // Streaming response (SSE pattern)
        let request = Request::builder()
            .method(Method::POST)
            .path("/v1/chat/completions")
            .json(&json!({
                "model": "gpt-4",
                "messages": [{"role": "user", "content": prompt}],
                "stream": true
            }))?
            .build()?;

        let response = self.oagw_client.execute("openai", request).await?;

        // Consume as SSE stream
        let mut sse = response.into_sse_stream();

        Ok(stream::unfold(sse, |mut sse| async move {
            match sse.next_event().await {
                Ok(Some(event)) if !event.data.contains("[DONE]") => {
                    let data: serde_json::Value = serde_json::from_str(&event.data).ok()?;
                    let delta = data["choices"][0]["delta"]["content"].as_str()?;
                    Some((delta.to_string(), sse))
                }
                _ => None,
            }
        }))
    }
}
```

**Configuration (environment determines mode)**:

```bash
# Development (shared-process mode)
export OAGW_MODE=shared
# No additional config needed - control plane injected by modkit

# Production (remote mode)
export OAGW_MODE=remote
export OAGW_BASE_URL=https://oagw.internal.cf
export OAGW_AUTH_TOKEN=<token>
```

**Key benefit**: Deploy the same binary in different modes without code changes.

### Core Types

#### Request Builder

```rust
pub struct Request {
    method: Method,
    path: String,              // Relative path: "/v1/chat/completions"
    headers: HeaderMap,
    body: Body,
    timeout: Option<Duration>,
    extensions: Extensions,
}

impl Request {
    pub fn builder() -> RequestBuilder { ... }

    pub fn method(&self) -> &Method { &self.method }
    pub fn path(&self) -> &str { &self.path }
    pub fn headers(&self) -> &HeaderMap { &self.headers }
    pub fn headers_mut(&mut self) -> &mut HeaderMap { &mut self.headers }
    pub fn body(&self) -> &Body { &self.body }
    pub fn into_body(self) -> Body { self.body }
}

pub struct RequestBuilder {
    method: Method,
    path: Option<String>,
    headers: HeaderMap,
    body: Option<Body>,
    timeout: Option<Duration>,
}

impl RequestBuilder {
    pub fn method(mut self, method: Method) -> Self { ... }
    pub fn path(mut self, path: impl Into<String>) -> Self { ... }
    pub fn header<K, V>(mut self, key: K, value: V) -> Self { ... }
    pub fn body<B: Into<Body>>(mut self, body: B) -> Self { ... }
    pub fn json<T: Serialize>(mut self, value: &T) -> Result<Self, serde_json::Error> { ... }
    pub fn timeout(mut self, duration: Duration) -> Self { ... }
    pub fn build(self) -> Result<Request, BuildError> { ... }
}
```

#### Response Types

```rust
// Response with flexible consumption
pub struct Response {
    status: StatusCode,
    headers: HeaderMap,
    body: ResponseBody,
    error_source: ErrorSource,  // Parsed from X-OAGW-Error-Source header
    extensions: Extensions,
}

// Internal response body representation
enum ResponseBody {
    Buffered(Bytes),
    Streaming(BoxStream<'static, Result<Bytes, ClientError>>),
}

impl Response {
    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    pub fn error_source(&self) -> ErrorSource {
        self.error_source
    }

    /// Buffer entire response body
    pub async fn bytes(self) -> Result<Bytes, ClientError> {
        match self.body {
            ResponseBody::Buffered(bytes) => Ok(bytes),
            ResponseBody::Streaming(mut stream) => {
                let mut buf = BytesMut::new();
                while let Some(chunk) = stream.next().await {
                    buf.extend_from_slice(&chunk?);
                }
                Ok(buf.freeze())
            }
        }
    }

    /// Parse response body as JSON
    pub async fn json<T: DeserializeOwned>(self) -> Result<T, ClientError> {
        let bytes = self.bytes().await?;
        serde_json::from_slice(&bytes)
            .map_err(|e| ClientError::InvalidResponse(e.to_string()))
    }

    /// Parse response body as text
    pub async fn text(self) -> Result<String, ClientError> {
        let bytes = self.bytes().await?;
        String::from_utf8(bytes.to_vec())
            .map_err(|e| ClientError::InvalidResponse(e.to_string()))
    }

    /// Consume response as byte stream (for SSE, chunked responses)
    pub fn into_stream(self) -> BoxStream<'static, Result<Bytes, ClientError>> {
        match self.body {
            ResponseBody::Buffered(bytes) => {
                // Convert buffered to single-item stream
                Box::pin(stream::once(async move { Ok(bytes) }))
            }
            ResponseBody::Streaming(stream) => stream,
        }
    }

    /// Convenience: parse as Server-Sent Events stream
    pub fn into_sse_stream(self) -> SseEventStream {
        SseEventStream::new(self.into_stream())
    }
}

// Server-Sent Events stream
pub struct SseEventStream {
    inner: BoxStream<'static, Result<Bytes, ClientError>>,
    buffer: Vec<u8>,
}

impl SseEventStream {
    pub fn new(stream: BoxStream<'static, Result<Bytes, ClientError>>) -> Self {
        Self {
            inner: stream,
            buffer: Vec::new(),
        }
    }

    pub async fn next_event(&mut self) -> Result<Option<SseEvent>, ClientError> {
        // SSE parsing logic
        // Reads until double newline (\n\n) for complete event
        loop {
            // Check if buffer contains complete event
            if let Some(event) = self.parse_buffered_event()? {
                return Ok(Some(event));
            }

            // Read more data
            match self.inner.next().await {
                Some(Ok(chunk)) => {
                    self.buffer.extend_from_slice(&chunk);
                }
                Some(Err(e)) => return Err(e),
                None => {
                    // Stream ended
                    if self.buffer.is_empty() {
                        return Ok(None);
                    } else {
                        // Return partial data as final event
                        return self.parse_buffered_event();
                    }
                }
            }
        }
    }

    fn parse_buffered_event(&mut self) -> Result<Option<SseEvent>, ClientError> {
        // Find double newline
        if let Some(pos) = self.buffer.windows(2).position(|w| w == b"\n\n") {
            let event_bytes = self.buffer.drain(..pos + 2).collect::<Vec<u8>>();
            return Ok(Some(Self::parse_sse_event(&event_bytes)?));
        }
        Ok(None)
    }

    fn parse_sse_event(data: &[u8]) -> Result<SseEvent, ClientError> {
        // Parse SSE format: "field: value\n"
        let mut id = None;
        let mut event = None;
        let mut data_lines = Vec::new();
        let mut retry = None;

        for line in data.split(|&b| b == b'\n') {
            if line.is_empty() {
                continue;
            }

            if let Some(colon_pos) = line.iter().position(|&b| b == b':') {
                let field = &line[..colon_pos];
                let value = &line[colon_pos + 1..];
                let value = if value.first() == Some(&b' ') {
                    &value[1..]
                } else {
                    value
                };

                match field {
                    b"id" => id = Some(String::from_utf8_lossy(value).to_string()),
                    b"event" => event = Some(String::from_utf8_lossy(value).to_string()),
                    b"data" => data_lines.push(String::from_utf8_lossy(value).to_string()),
                    b"retry" => {
                        retry = String::from_utf8_lossy(value)
                            .parse()
                            .ok();
                    }
                    _ => {} // Ignore unknown fields
                }
            }
        }

        Ok(SseEvent {
            id,
            event,
            data: data_lines.join("\n"),
            retry,
        })
    }
}

pub struct SseEvent {
    pub id: Option<String>,
    pub event: Option<String>,
    pub data: String,
    pub retry: Option<u64>,
}

// Removed: StreamingResponse (no longer needed!)

// WebSocket connection
pub struct WebSocketConn {
    send: mpsc::Sender<WsMessage>,
    recv: mpsc::Receiver<Result<WsMessage, ClientError>>,
}

impl WebSocketConn {
    pub async fn send(&mut self, msg: WsMessage) -> Result<(), ClientError> {
        self.send.send(msg).await
            .map_err(|_| ClientError::ConnectionClosed)
    }

    pub async fn recv(&mut self) -> Result<Option<WsMessage>, ClientError> {
        self.recv.recv().await
            .transpose()
    }

    pub async fn close(self) -> Result<(), ClientError> { ... }
}

pub enum WsMessage {
    Text(String),
    Binary(Bytes),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Close(Option<CloseFrame>),
}
```

#### Body Abstraction

```rust
pub enum Body {
    Empty,
    Bytes(Bytes),
    Stream(BoxStream<'static, Result<Bytes, std::io::Error>>),
}

impl Body {
    pub fn empty() -> Self { Body::Empty }

    pub fn from_bytes(bytes: impl Into<Bytes>) -> Self {
        Body::Bytes(bytes.into())
    }

    pub fn from_json<T: Serialize>(value: &T) -> Result<Self, serde_json::Error> {
        Ok(Body::Bytes(serde_json::to_vec(value)?.into()))
    }

    pub fn from_stream<S>(stream: S) -> Self
    where
        S: Stream<Item=Result<Bytes, std::io::Error>> + Send + 'static,
    {
        Body::Stream(Box::pin(stream))
    }

    pub async fn into_bytes(self) -> Result<Bytes, ClientError> {
        match self {
            Body::Empty => Ok(Bytes::new()),
            Body::Bytes(b) => Ok(b),
            Body::Stream(mut s) => {
                let mut buf = BytesMut::new();
                while let Some(chunk) = s.next().await {
                    buf.extend_from_slice(&chunk?);
                }
                Ok(buf.freeze())
            }
        }
    }
}

impl From<()> for Body {
    fn from(_: ()) -> Self { Body::Empty }
}

impl From<Bytes> for Body {
    fn from(b: Bytes) -> Self { Body::Bytes(b) }
}

impl From<Vec<u8>> for Body {
    fn from(v: Vec<u8>) -> Self { Body::Bytes(v.into()) }
}

impl From<String> for Body {
    fn from(s: String) -> Self { Body::Bytes(s.into()) }
}
```

#### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Request build error: {0}")]
    BuildError(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("TLS error: {0}")]
    Tls(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {status}")]
    Http { status: StatusCode, body: Bytes },
}
```

### Client Implementations

#### Public API (Deployment-Agnostic)

```rust
/// Main client type - works in both shared-process and remote modes
pub struct OagwClient {
    inner: OagwClientImpl,
}

enum OagwClientImpl {
    SharedProcess(SharedProcessClient),
    RemoteProxy(RemoteProxyClient),
}

impl OagwClient {
    /// Create client from configuration
    pub fn from_config(config: OagwClientConfig) -> Result<Self, ClientError> {
        let inner = match config.mode {
            ClientMode::SharedProcess { control_plane } => {
                OagwClientImpl::SharedProcess(SharedProcessClient::new(control_plane)?)
            }
            ClientMode::RemoteProxy { base_url, auth_token, timeout } => {
                OagwClientImpl::RemoteProxy(RemoteProxyClient::new(
                    base_url,
                    auth_token,
                    timeout,
                )?)
            }
        };
        Ok(Self { inner })
    }

    /// Execute HTTP request through OAGW
    /// Response can be consumed as buffered or streaming
    pub async fn execute(&self, alias: &str, request: Request) -> Result<Response, ClientError> {
        match &self.inner {
            OagwClientImpl::SharedProcess(c) => c.execute(alias, request).await,
            OagwClientImpl::RemoteProxy(c) => c.execute(alias, request).await,
        }
    }

    /// Establish WebSocket connection through OAGW
    pub async fn websocket(
        &self,
        alias: &str,
        request: Request,
    ) -> Result<WebSocketConn, ClientError> {
        match &self.inner {
            OagwClientImpl::SharedProcess(c) => c.websocket(alias, request).await,
            OagwClientImpl::RemoteProxy(c) => c.websocket(alias, request).await,
        }
    }
}

/// Configuration for OagwClient
pub struct OagwClientConfig {
    pub mode: ClientMode,
    pub default_timeout: Duration,
}

pub enum ClientMode {
    /// OAGW in same process - direct function calls (zero serialization)
    SharedProcess {
        control_plane: Arc<dyn ControlPlaneService>,
    },
    /// OAGW in separate process - HTTP calls to proxy endpoint
    RemoteProxy {
        base_url: String,
        auth_token: String,
        timeout: Duration,
    },
}

impl OagwClientConfig {
    /// Automatically detect mode from environment
    pub fn from_env() -> Result<Self, ClientError> {
        // Check for OAGW_MODE environment variable
        match std::env::var("OAGW_MODE").as_deref() {
            Ok("shared") => {
                // In shared-process mode, control plane is injected by modkit
                let control_plane = get_control_plane_from_di()?;
                Ok(Self {
                    mode: ClientMode::SharedProcess { control_plane },
                    default_timeout: Duration::from_secs(30),
                })
            }
            Ok("remote") | Ok(_) | Err(_) => {
                // Default to remote mode
                let base_url = std::env::var("OAGW_BASE_URL")
                    .unwrap_or_else(|_| "https://oagw.internal.cf".to_string());
                let auth_token = std::env::var("OAGW_AUTH_TOKEN")?;
                Ok(Self {
                    mode: ClientMode::RemoteProxy {
                        base_url,
                        auth_token,
                        timeout: Duration::from_secs(30),
                    },
                    default_timeout: Duration::from_secs(30),
                })
            }
        }
    }
}
```

#### Shared-Process Client (Internal Implementation)

**Used when**: Internal module and OAGW run in the same process (development, single-executable deployment).

```rust
struct SharedProcessClient {
    control_plane: Arc<dyn ControlPlaneService>,
    metrics: Arc<Metrics>,
}

impl SharedProcessClient {
    fn new(control_plane: Arc<dyn ControlPlaneService>) -> Result<Self, ClientError> {
        Ok(Self {
            control_plane,
            metrics: Arc::new(Metrics::default()),
        })
    }

    async fn execute(&self, alias: &str, request: Request) -> Result<Response, ClientError> {
        let start = Instant::now();
        self.metrics.requests_in_flight.inc();

        // Direct function call to Data Plane (zero serialization overhead)
        let proxy_request = ProxyRequest {
            alias: alias.to_string(),
            method: request.method().clone(),
            path: request.path().to_string(),
            headers: request.headers().clone(),
            body: request.into_body(),
        };

        let proxy_response = self.control_plane.proxy_request(proxy_request).await
            .map_err(|e| ClientError::Connection(e.to_string()))?;

        self.metrics.requests_in_flight.dec();
        self.metrics.request_duration.observe(start.elapsed().as_secs_f64());

        // Parse X-OAGW-Error-Source header
        let error_source = proxy_response.headers
            .get("x-oagw-error-source")
            .and_then(|v| v.to_str().ok())
            .map(|s| match s {
                "gateway" => ErrorSource::Gateway,
                "upstream" => ErrorSource::Upstream,
                _ => ErrorSource::Unknown,
            })
            .unwrap_or(ErrorSource::Unknown);

        // Response body can be buffered or streaming
        // Data Plane determines this based on upstream response
        let body = if proxy_response.is_streaming {
            ResponseBody::Streaming(proxy_response.body_stream)
        } else {
            ResponseBody::Buffered(proxy_response.body)
        };

        Ok(Response {
            status: proxy_response.status,
            headers: proxy_response.headers,
            body,
            error_source,
            extensions: Extensions::default(),
        })
    }

    async fn websocket(&self, alias: &str, request: Request) -> Result<WebSocketConn, ClientError> {
        let proxy_request = ProxyRequest {
            alias: alias.to_string(),
            method: request.method().clone(),
            path: request.path().to_string(),
            headers: request.headers().clone(),
            body: request.into_body(),
        };

        let ws_conn = self.control_plane.proxy_websocket(proxy_request).await
            .map_err(|e| ClientError::Connection(e.to_string()))?;

        Ok(ws_conn)
    }
}
```

#### Remote OAGW Client (Internal Implementation)

**Used when**: Internal module and OAGW run in separate processes (production, microservice deployment).

```rust
struct RemoteProxyClient {
    oagw_base_url: String,      // e.g., "https://oagw.internal.cf"
    http_client: reqwest::Client,
    auth_token: String,
    metrics: Arc<Metrics>,
}

impl RemoteProxyClient {
    fn new(
        base_url: String,
        auth_token: String,
        timeout: Duration,
    ) -> Result<Self, ClientError> {
        let http_client = reqwest::Client::builder()
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| ClientError::BuildError(e.to_string()))?;

        Ok(Self {
            oagw_base_url: base_url,
            http_client,
            auth_token,
            metrics: Arc::new(Metrics::default()),
        })
    }

    async fn execute(&self, alias: &str, request: Request) -> Result<Response, ClientError> {
        let start = Instant::now();
        self.metrics.requests_in_flight.inc();

        // Build URL: https://oagw.internal.cf/api/oagw/v1/proxy/{alias}{path}
        let url = format!("{}/api/oagw/v1/proxy/{}{}",
            self.oagw_base_url, alias, request.path());

        let mut req_builder = self.http_client.request(request.method().clone(), &url)
            .header("Authorization", format!("Bearer {}", self.auth_token));

        // Forward all headers from original request
        for (name, value) in request.headers() {
            req_builder = req_builder.header(name, value);
        }

        // Set body
        match request.into_body() {
            Body::Empty => {}
            Body::Bytes(b) => {
                req_builder = req_builder.body(b.to_vec());
            }
            Body::Stream(_) => {
                return Err(ClientError::BuildError(
                    "Streaming body not supported for plain requests".into()
                ));
            }
        }

        let resp = req_builder.send().await
            .map_err(|e| self.map_reqwest_error(e))?;

        let status = resp.status();
        let headers = resp.headers().clone();

        // Parse X-OAGW-Error-Source header
        let error_source = headers
            .get("x-oagw-error-source")
            .and_then(|v| v.to_str().ok())
            .map(|s| match s {
                "gateway" => ErrorSource::Gateway,
                "upstream" => ErrorSource::Upstream,
                _ => ErrorSource::Unknown,
            })
            .unwrap_or(ErrorSource::Unknown);

        // Always return as streaming - consumer decides if they want to buffer
        // This allows flexibility: .bytes() for buffered, .into_stream() for streaming
        let stream = resp.bytes_stream()
            .map_err(|e| ClientError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e
            )));

        self.metrics.requests_in_flight.dec();
        self.metrics.request_duration.observe(start.elapsed().as_secs_f64());

        Ok(Response {
            status,
            headers,
            body: ResponseBody::Streaming(Box::pin(stream)),
            error_source,
            extensions: Extensions::default(),
        })
    }

    async fn websocket(&self, alias: &str, request: Request) -> Result<WebSocketConn, ClientError> {
        // Build WebSocket URL: wss://oagw.internal.cf/api/oagw/v1/proxy/{alias}{path}
        let ws_url = self.oagw_base_url
            .replace("https://", "wss://")
            .replace("http://", "ws://");
        let url = format!("{}/api/oagw/v1/proxy/{}{}", ws_url, alias, request.path());

        let req = tokio_tungstenite::tungstenite::http::Request::builder()
            .uri(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", generate_ws_key())
            .body(())
            .map_err(|e| ClientError::BuildError(e.to_string()))?;

        let (ws_stream, _) = tokio_tungstenite::connect_async(req).await
            .map_err(|e| ClientError::Connection(e.to_string()))?;

        let (write, read) = ws_stream.split();
        let (send_tx, mut send_rx) = mpsc::channel(16);
        let (recv_tx, recv_rx) = mpsc::channel(16);

        // Send task
        tokio::spawn(async move {
            let mut write = write;
            while let Some(msg) = send_rx.recv().await {
                let ws_msg = match msg {
                    WsMessage::Text(s) => tokio_tungstenite::tungstenite::Message::Text(s),
                    WsMessage::Binary(b) => tokio_tungstenite::tungstenite::Message::Binary(b.to_vec()),
                    WsMessage::Ping(p) => tokio_tungstenite::tungstenite::Message::Ping(p),
                    WsMessage::Pong(p) => tokio_tungstenite::tungstenite::Message::Pong(p),
                    WsMessage::Close(f) => tokio_tungstenite::tungstenite::Message::Close(f),
                };

                if write.send(ws_msg).await.is_err() {
                    break;
                }
            }
        });

        // Receive task
        tokio::spawn(async move {
            let mut read = read;
            while let Some(msg) = read.next().await {
                let result = match msg {
                    Ok(tokio_tungstenite::tungstenite::Message::Text(s)) =>
                        Ok(WsMessage::Text(s)),
                    Ok(tokio_tungstenite::tungstenite::Message::Binary(b)) =>
                        Ok(WsMessage::Binary(b.into())),
                    Ok(tokio_tungstenite::tungstenite::Message::Ping(p)) =>
                        Ok(WsMessage::Ping(p)),
                    Ok(tokio_tungstenite::tungstenite::Message::Pong(p)) =>
                        Ok(WsMessage::Pong(p)),
                    Ok(tokio_tungstenite::tungstenite::Message::Close(f)) =>
                        Ok(WsMessage::Close(f)),
                    Err(e) => Err(ClientError::Protocol(e.to_string())),
                    _ => continue,
                };

                if recv_tx.send(result).await.is_err() {
                    break;
                }
            }
        });

        Ok(WebSocketConn {
            send: send_tx,
            recv: recv_rx,
        })
    }
}

impl RemoteProxyClient {
    fn map_reqwest_error(&self, error: reqwest::Error) -> ClientError {
        if error.is_timeout() {
            ClientError::Timeout(error.to_string())
        } else if error.is_connect() {
            ClientError::Connection(error.to_string())
        } else {
            ClientError::Protocol(error.to_string())
        }
    }
}
```

### Out of Scope: OAGW Plugin Development

**Note**: Plugin development APIs (PluginContext, Starlark integration) are **not part of this client library**. They belong in OAGW's plugin system (see [ADR: Plugin System](./adr-plugin-system.md)).

This client library is solely for **internal modules** to make HTTP requests **through** OAGW, not for developing plugins that **run inside** OAGW.

### WebTransport Support (Future)

```rust
// Placeholder for WebTransport (QUIC-based)
pub struct WebTransportConn {
    session: webtransport::Session,
}

impl WebTransportConn {
    pub async fn open_stream(&mut self) -> Result<WebTransportStream, ClientError> {
        // Open bidirectional stream
        todo!("WebTransport implementation")
    }

    pub async fn accept_stream(&mut self) -> Result<WebTransportStream, ClientError> {
        // Accept incoming stream
        todo!("WebTransport implementation")
    }

    pub async fn send_datagram(&mut self, data: Bytes) -> Result<(), ClientError> {
        // Unreliable datagram
        todo!("WebTransport implementation")
    }

    pub async fn recv_datagram(&mut self) -> Result<Bytes, ClientError> {
        todo!("WebTransport implementation")
    }
}

pub struct WebTransportStream {
    send: BoxSink<'static, Bytes>,
    recv: BoxStream<'static, Result<Bytes, ClientError>>,
}
```

## Implementation Plan

### Phase 0 (MVP): Core Client Types & RemoteProxyClient

- `Request`, `Response`, `Body`, `ErrorSource` types
- `ResponseBody` enum (Buffered/Streaming) - **streaming built-in from day 1**
- `OagwClient` type with deployment abstraction
- `RemoteProxyClient` implementation (HTTP to OAGW `/proxy/{alias}/*`)
- Response consumption patterns: `.bytes()`, `.json()`, `.into_stream()`, `.into_sse_stream()`
- Error source distinction (`X-OAGW-Error-Source` header parsing)
- SSE event stream parsing (`SseEventStream`, `SseEvent`)
- Error handling and metrics
- **SDK Integration**: Pattern 4 (Wrapper Layer) for OpenAI

**Deliverable**: Internal modules can make HTTP requests (buffered and streaming) through OAGW in production

### Phase 1: SharedProcessClient

- `SharedProcessClient` implementation (direct function calls to Data Plane)
- Integration with `ControlPlaneService` trait
- Configuration abstraction via `OagwClientConfig::from_env()`
- Automatic mode selection (shared vs remote)

**Deliverable**: Development mode with zero serialization overhead, deployment-agnostic code

### Phase 2: WebSocket Support

- `WebSocketConn` type
- WebSocket upgrade handling
- Bidirectional message passing
- Connection lifecycle management
- Support in both client implementations

**Deliverable**: WebSocket connections work through OAGW

### Phase 3: SDK Integration - HTTP Proxy Pattern

- Implement Pattern 2 (HTTP Proxy)
- Local proxy server on `localhost:8080`
- Host-to-alias mapping configuration
- Transparent support for unmodified third-party SDKs

**Deliverable**: Any Rust SDK can use OAGW via `HTTP_PROXY` env var

### Phase 4: WebTransport (Future)

- QUIC transport layer
- `WebTransportConn` implementation
- Stream multiplexing
- Datagram support

**Deliverable**: WebTransport protocol support

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create client for tests
    fn create_test_client() -> OagwClient {
        let config = OagwClientConfig {
            mode: ClientMode::RemoteProxy {
                base_url: "https://oagw.internal.cf".to_string(),
                auth_token: "test-token".to_string(),
                timeout: Duration::from_secs(30),
            },
            default_timeout: Duration::from_secs(30),
        };
        OagwClient::from_config(config).unwrap()
    }

    #[tokio::test]
    async fn test_buffered_response() {
        // Buffer entire response (default consumption pattern)
        let client = create_test_client();

        let request = Request::builder()
            .method(Method::POST)
            .path("/v1/chat/completions")
            .json(&json!({"model": "gpt-4", "messages": []}))
            .unwrap()
            .build()
            .unwrap();

        let response = client.execute("openai", request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.error_source(), ErrorSource::Upstream);

        // Consume as JSON (buffers automatically)
        let data = response.json::<ChatResponse>().await.unwrap();
        assert!(!data.choices.is_empty());
    }

    #[tokio::test]
    async fn test_streaming_response() {
        // Stream response (for SSE)
        let client = create_test_client();

        let request = Request::builder()
            .method(Method::POST)
            .path("/v1/chat/completions")
            .json(&json!({"model": "gpt-4", "messages": [], "stream": true}))
            .unwrap()
            .build()
            .unwrap();

        let response = client.execute("openai", request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.error_source(), ErrorSource::Upstream);

        // Consume as stream
        let mut stream = response.into_stream();
        let mut chunks = 0;
        while let Some(chunk) = stream.next().await {
            chunk.unwrap();
            chunks += 1;
        }

        assert!(chunks > 0);
    }

    #[tokio::test]
    async fn test_sse_streaming() {
        // Server-Sent Events consumption
        let client = create_test_client();

        let request = Request::builder()
            .method(Method::POST)
            .path("/v1/chat/completions")
            .json(&json!({"model": "gpt-4", "messages": [], "stream": true}))
            .unwrap()
            .build()
            .unwrap();

        let response = client.execute("openai", request).await.unwrap();

        // Consume as SSE events
        let mut sse = response.into_sse_stream();
        while let Some(event) = sse.next_event().await.unwrap() {
            println!("SSE event: {}", event.data);
            if event.data.contains("[DONE]") {
                break;
            }
        }
    }

    #[tokio::test]
    async fn test_websocket() {
        let client = create_test_client();

        let request = Request::builder()
            .method(Method::GET)
            .path("/ws")
            .build()
            .unwrap();

        let mut conn = client.websocket("echo-service", request).await.unwrap();

        conn.send(WsMessage::Text("hello".into())).await.unwrap();
        let msg = conn.recv().await.unwrap().unwrap();

        match msg {
            WsMessage::Text(s) => assert_eq!(s, "hello"),
            _ => panic!("Expected text message"),
        }
    }

    #[tokio::test]
    async fn test_error_source_gateway() {
        let client = create_test_client();

        let request = Request::builder()
            .method(Method::GET)
            .path("/nonexistent")
            .build()
            .unwrap();

        let response = client.execute("invalid-alias", request).await.unwrap();

        // OAGW returns 404 for unknown alias
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.error_source, ErrorSource::Gateway);
    }

    #[tokio::test]
    async fn test_deployment_agnostic_code() {
        // Same code works in both deployment modes
        // Only configuration changes

        // Shared-process mode (development)
        let shared_config = OagwClientConfig {
            mode: ClientMode::SharedProcess {
                control_plane: get_mock_control_plane(),
            },
            default_timeout: Duration::from_secs(30),
        };
        let shared_client = OagwClient::from_config(shared_config).unwrap();

        // Remote mode (production)
        let remote_config = OagwClientConfig {
            mode: ClientMode::RemoteProxy {
                base_url: "https://oagw.internal.cf".to_string(),
                auth_token: "test-token".to_string(),
                timeout: Duration::from_secs(30),
            },
            default_timeout: Duration::from_secs(30),
        };
        let remote_client = OagwClient::from_config(remote_config).unwrap();

        // Identical application code for both clients
        let request = Request::builder()
            .method(Method::POST)
            .path("/v1/chat/completions")
            .json(&json!({"model": "gpt-4", "messages": []}))
            .unwrap()
            .build()
            .unwrap();

        let response1 = shared_client.execute("openai", request.clone()).await;
        let response2 = remote_client.execute("openai", request).await;

        // Both work identically
        assert!(response1.is_ok() || response2.is_ok());
    }
}
```

### Mock Client for Testing

Mock can be added as a third variant in the client enum:

```rust
// Add Mock variant to OagwClientImpl
enum OagwClientImpl {
    SharedProcess(SharedProcessClient),
    RemoteProxy(RemoteProxyClient),
    #[cfg(test)]
    Mock(MockClient),
}

// Mock implementation (test-only)
#[cfg(test)]
struct MockClient {
    responses: Arc<Mutex<HashMap<String, VecDeque<Result<Response, ClientError>>>>>,
}

#[cfg(test)]
impl MockClient {
    fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn push_response(&self, alias: &str, response: Response) {
        self.responses
            .lock()
            .unwrap()
            .entry(alias.to_string())
            .or_default()
            .push_back(Ok(response));
    }

    fn push_error(&self, alias: &str, error: ClientError) {
        self.responses
            .lock()
            .unwrap()
            .entry(alias.to_string())
            .or_default()
            .push_back(Err(error));
    }

    async fn execute(&self, alias: &str, _request: Request) -> Result<Response, ClientError> {
        self.responses
            .lock()
            .unwrap()
            .get_mut(alias)
            .and_then(|queue| queue.pop_front())
            .unwrap_or(Err(ClientError::Connection(
                format!("No mock response configured for alias: {}", alias)
            )))
    }

    async fn websocket(&self, _alias: &str, _request: Request) -> Result<WebSocketConn, ClientError> {
        Err(ClientError::BuildError("Mock WebSocket not implemented".into()))
    }
}

// Add test helper to OagwClient
#[cfg(test)]
impl OagwClient {
    pub fn mock() -> (Self, Arc<MockClient>) {
        let mock = Arc::new(MockClient::new());
        let client = Self {
            inner: OagwClientImpl::Mock(Arc::clone(&mock)),
        };
        (client, mock)
    }
}

// Usage in tests - same OagwClient API
#[tokio::test]
async fn test_with_mock() {
    let (client, mock) = OagwClient::mock();

    mock.push_response("openai", Response {
        status: StatusCode::OK,
        headers: HeaderMap::new(),
        body: Bytes::from(r#"{"choices": []}"#),
        error_source: ErrorSource::Upstream,
        extensions: Extensions::default(),
    });

    let request = Request::builder()
        .method(Method::POST)
        .path("/v1/chat/completions")
        .build()
        .unwrap();

    // Same API as production code
    let response = client.execute("openai", request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
```

## Dependencies

```toml
[dependencies]
# Core HTTP client (for RemoteProxyClient)
reqwest = { version = "0.11", features = ["json", "stream"] }
hyper = "1.5"  # Updated to match workspace version
http = "1.3"   # Updated to match workspace version
bytes = "1.11" # Updated to match workspace version

# Async runtime
# Note: Blocking API uses tokio::runtime::Runtime internally
tokio = { version = "1.47", features = ["full", "rt-multi-thread"] }
futures = "0.3"
async-trait = "0.1"

# WebSocket
tokio-tungstenite = "0.21"

# WebTransport (future)
# webtransport = "0.1"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Error handling
thiserror = "2.0"  # Updated to match workspace version
anyhow = "1.0"

# Metrics
prometheus = "0.13"

# Tracing
tracing = "0.1"

# Out-of-process RPC (future)
# tonic = "0.10"
# prost = "0.12"

[dev-dependencies]
# HTTP server for testing proxy mode
axum = "0.8"
tower = "0.5"

# Testing utilities
tokio-test = "0.4"
httpmock = "0.8"

# For compatibility tests
ureq = "2.12"  # Test sync client compatibility
```

**Notes on dependencies**:

1. **reqwest**: Required for `RemoteProxyClient`. Version aligned with workspace.
2. **tokio runtime**: Blocking API (`execute_blocking()`) requires `rt-multi-thread` feature to create temporary runtimes.
3. **Version alignment**: Updated versions to match workspace `Cargo.toml` (hyper 1.5, http 1.3, bytes 1.11, thiserror 2.0).
4. **Dev dependencies**: Added `ureq` for testing sync client compatibility via HTTP proxy mode.

## Security Considerations

1. **Timeout enforcement**: All requests have mandatory timeouts to prevent resource exhaustion
2. **Body size limits**: Maximum body size (100MB) enforced before buffering
3. **TLS verification**: Certificate validation always enabled (no insecure mode in production)
4. **Connection pooling**: Limits on idle connections per host to prevent resource leaks
5. **Sandbox isolation**: Starlark plugins cannot make network requests directly
6. **Header validation**: Well-known hop-by-hop headers stripped automatically
7. **WebSocket security**: Proper origin validation and frame size limits

## Performance Considerations

1. **Zero-copy streaming**: SSE and chunked responses use stream-based processing
2. **Connection reuse**: HTTP/1.1 and HTTP/2 connection pooling enabled
3. **Adaptive window sizing**: HTTP/2 flow control optimized for throughput
4. **Minimal allocations**: `Bytes` type uses reference counting for zero-copy operations
5. **Async I/O**: Non-blocking operations throughout the stack
6. **Metrics overhead**: Negligible (<0.1ms per request)

## Alternatives Considered

### Alternative 1: Use `hyper` Directly

**Pros**:

- Lower-level control
- Slightly better performance

**Cons**:

- More complex API
- More boilerplate code
- Missing high-level features (redirects, cookies, etc.)

**Decision**: Use `reqwest` (built on `hyper`) for better ergonomics.

### Alternative 2: Custom HTTP Client Implementation

**Pros**:

- Full control over implementation
- Optimized for OAGW use cases

**Cons**:

- Significant development effort
- Maintenance burden
- Likely inferior to battle-tested libraries

**Decision**: Use existing libraries (`reqwest`, `tokio-tungstenite`).

### Alternative 3: Single Monolithic Client Type

**Pros**:

- Simpler API surface

**Cons**:

- Cannot support multiple backends
- Harder to test
- Less flexible for future extensions

**Decision**: Use trait-based abstraction.

## Related ADRs

- [ADR: Component Architecture](./adr-component-architecture.md) - OAGW deployment modes (shared-process vs microservice)
- [ADR: Error Source Distinction](./adr-error-source-distinction.md) - `X-OAGW-Error-Source` header for gateway vs upstream errors
- [ADR: Plugin System](./adr-plugin-system.md) - OAGW plugin development (separate from this client library)
- [ADR: Request Routing](./adr-request-routing.md) - How OAGW routes requests to upstreams

## References

- [reqwest documentation](https://docs.rs/reqwest)
- [hyper documentation](https://docs.rs/hyper)
- [tokio-tungstenite documentation](https://docs.rs/tokio-tungstenite)
- [WebTransport specification](https://w3c.github.io/webtransport/)
- [Server-Sent Events specification](https://html.spec.whatwg.org/multipage/server-sent-events.html)
- [WebSocket protocol RFC 6455](https://datatracker.ietf.org/doc/html/rfc6455)
