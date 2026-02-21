# ADR-0003: Universal Lazy Typed REST Clients for OoP Modules

## Executive Summary

This proposal outlines a migration from gRPC to REST as the default transport for out-of-process (OoP) modules and introduces a **universal lazy typed client layer** for OoP module communication in ModKit. The implementation is structured in phases:

1. **Phase 1 - REST as default transport**: Establish REST as the default OoP transport; gRPC becomes opt-in
2. **Phase 2 - ClientDescriptor trait**: SDK-defined metadata binding compile-time types to runtime resolution
3. **Phase 3 - ClientProvider**: Lazy resolution, caching, backoff, and reconnection infrastructure
4. **Phase 4 - Macro extension**: `clients = [...]` auto-registers lazy clients into `ClientHub`
5. **Phase 5 - Soft OoP deps**: Dependencies on OoP modules don't block startup
6. **Phase 6 - Registry extension**: Soft OoP dep resolution:  

**Current pattern** (problematic):
```rust
// Consumer must wire client manually in init() - FAILS if OoP module is not ready
calculator_sdk::wire_client(hub, &*directory).await?;
```

**Proposed pattern**:
```rust
#[modkit::module(
    name = "calculator_gateway",
    clients = [calculator_sdk::CalculatorClientDescriptor],  // REST by default
)]
// No wire_client() needed - lazy client auto-registered
```

---

## Problem Statement

The current OoP client wiring pattern has several issues:

1. **Eager wiring is fragile**: Consumer modules call `wire_client()` in `init()`, which fails if the OoP dependency is not yet available.
2. **Startup coupling**: The entire module fails to start if any OoP dependency is temporarily unavailable.
3. **Boilerplate duplication**: Each SDK repeats the same resolve/connect/cache logic.
4. **No graceful degradation**: Missing dependencies cause module-level failures instead of per-operation failures (HTTP 424).
5. **gRPC complexity**: Binary protobuf payloads are hard to debug; requires specialized tooling.

### Current Pattern (calculator_gateway example)

```rust
// Current: Consumer must wire client manually, and it happens eagerly
pub async fn wire_client(hub: &ClientHub, resolver: &dyn DirectoryClient) -> Result<()> {
    let endpoint = resolver.resolve_grpc_service(SERVICE_NAME).await?;  // Fails if OoP not ready
    let client = CalculatorGrpcClient::connect(&endpoint.uri).await?;   // Fails if network issue
    hub.register::<dyn CalculatorClientV1>(Arc::new(client));
    Ok(())
}
```

---

## Proposed Solution

### Architecture Overview

```text
┌─────────────────────────────────────────────────────────────────────────┐
│                           SDK Crate (calculator-sdk)                    │
├─────────────────────────────────────────────────────────────────────────┤
│  CalculatorClientDescriptor                                             │
│    - MODULE_NAME: "calculator"                                          │
│    - Api: dyn CalculatorClientV1                                        │
│    - Transport: Rest (default) | Grpc (opt-in)                          │
│    - Availability Policy: Optional (default)                            │
├─────────────────────────────────────────────────────────────────────────┤
│  LazyCalculatorClient                                                   │
│    - Implements CalculatorClientV1                                      │
│    - Delegates to ClientProvider for lazy connection                    │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                           ModKit (libs/modkit)                          │
├─────────────────────────────────────────────────────────────────────────┤
│  ClientProvider (transport-agnostic interface)                          │
│    - Lazy resolution via DirectoryClient                                │
│    - Endpoint/connection caching with eviction on error                 │
│    - Backoff/rate-limiting for reconnects                               │
│    - Transport middleware (timeouts, retries, tracing)                  │
├─────────────────────────────────────────────────────────────────────────┤
│  RestClientProvider (default) | GrpcClientProvider (feature = "grpc")   │
│    - Transport-specific implementations                                 │
├─────────────────────────────────────────────────────────────────────────┤
│  #[modkit::module] macro extension                                      │
│    - clients = [CalculatorClientDescriptor]                             │
│    - Auto-registers LazyClient into ClientHub                           │
│    - Auto-injects MODULE_NAME from each descriptor into deps            │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                      Consumer Module (calculator_gateway)               │
├─────────────────────────────────────────────────────────────────────────┤
│  #[modkit::module(                                                      │
│      name = "calculator_gateway",                                       │
│      capabilities = [rest],                                             │
│      clients = [calculator_sdk::CalculatorClientDescriptor],            │
│      // deps auto-injected: ["calculator"] from descriptor              │
│  )]                                                                     │
│                                                                         │
│  // No wire_client() call needed!                                       │
│  // Client is always available from ClientHub                           │
│  let calc = hub.get::<dyn CalculatorClientV1>()?;                       │
│  calc.add(ctx, a, b).await?;  // Lazy connect on first call             │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Detailed Implementation

### Phase 1: REST as Default Transport

**Rationale**: REST is simpler to debug, requires no code generation, and is sufficient for most OoP calls. gRPC remains available for streaming or high-throughput use cases.

| Factor          | REST                            | gRPC                         |
|-----------------|---------------------------------|------------------------------|
| Debuggability   | ✅ curl, browser, any HTTP tool | ❌ Requires specialized tools |
| Simplicity      | ✅ JSON, standard HTTP          | ❌ Protobuf, code generation  |
| Browser support | ✅ Native                       | ❌ Requires gRPC-Web proxy    |
| API reuse       | ✅ Same as public REST API      | ❌ Separate interface         |
| Streaming       | ❌ Requires SSE/WebSocket       | ✅ Native support             |
| Performance     | ⚠️ JSON overhead                | ✅ Binary, efficient          |

#### 1.1 Transport Enum

**Location**: `libs/modkit/src/clients/transport.rs`

```rust
/// Transport protocol for OoP communication.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Transport {
    /// REST with JSON serialization (default).
    #[default]
    Rest,
    /// gRPC with protobuf serialization (opt-in, requires feature = "grpc").
    #[cfg(feature = "grpc")]
    Grpc,
}
```

#### 1.2 REST Service Discovery Infrastructure

REST service discovery requires extending the existing `ModuleInstance`, `ModuleManager`, and `DirectoryClient` to track and resolve REST endpoints.

##### 1.2.1 Extend `ModuleInstance` to track REST endpoints

**Location**: `libs/modkit/src/runtime/module_manager.rs`

```rust
/// Represents a single instance of a module
#[derive(Debug)]
pub struct ModuleInstance {
    pub module: String,
    pub instance_id: Uuid,
    pub control: Option<Endpoint>,
    pub grpc_services: HashMap<String, Endpoint>,
    pub rest_endpoint: Option<Endpoint>,  // NEW: REST base URL for this instance
    pub version: Option<String>,
    inner: Arc<parking_lot::RwLock<InstanceRuntimeState>>,
}

impl ModuleInstance {
    // ... existing methods ...

    /// Set the REST endpoint for this instance
    pub fn with_rest_endpoint(mut self, ep: Endpoint) -> Self {
        self.rest_endpoint = Some(ep);
        self
    }
}
```

##### 1.2.2 Extend `ModuleManager` with REST discovery

**Location**: `libs/modkit/src/runtime/module_manager.rs`

```rust
impl ModuleManager {
    /// Pick a REST endpoint for a module using round-robin selection.
    /// Returns (module_name, instance, endpoint) if found.
    #[must_use]
    pub fn pick_rest_endpoint_round_robin(
        &self,
        module_name: &str,
    ) -> Option<(String, Arc<ModuleInstance>, Endpoint)> {
        let instances_entry = self.inner.get(module_name)?;
        let instances = instances_entry.value();

        // Filter to instances with REST endpoints and healthy/ready state
        let candidates: Vec<_> = instances
            .iter()
            .filter(|inst| {
                inst.rest_endpoint.is_some()
                    && matches!(inst.state(), InstanceState::Healthy | InstanceState::Ready)
            })
            .cloned()
            .collect();

        if candidates.is_empty() {
            return None;
        }

        let len = candidates.len();
        let rr_key = format!("rest:{}", module_name);
        let mut counter = self.rr_counters.entry(rr_key).or_insert(0);
        let idx = *counter % len;
        *counter = (*counter + 1) % len;

        candidates.get(idx).map(|inst| {
            (
                module_name.to_owned(),
                inst.clone(),
                inst.rest_endpoint.clone().expect("filtered above"),
            )
        })
    }
}
```

##### 1.2.3 Extend `DirectoryClient` trait

**Location**: `cf_system_sdks/src/directory.rs` (upstream crate — requires update)

```rust
#[async_trait]
pub trait DirectoryClient: Send + Sync {
    /// Resolve REST endpoint for a module (default for OoP).
    async fn resolve_rest_service(&self, module_name: &str) -> Result<RestEndpoint>;

    /// Resolve gRPC endpoint for a module (opt-in).
    async fn resolve_grpc_service(&self, service_name: &str) -> Result<ServiceEndpoint>;

    // ... existing methods (list_instances, register_instance, etc.) ...
}

/// REST endpoint for a module
#[derive(Debug, Clone)]
pub struct RestEndpoint {
    /// Base URL for the module's REST API (e.g., "http://calculator:8080")
    pub base_url: String,
}

impl RestEndpoint {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self { base_url: base_url.into() }
    }

    pub fn http(host: &str, port: u16) -> Self {
        Self { base_url: format!("http://{}:{}", host, port) }
    }
}
```

##### 1.2.4 Implement `resolve_rest_service` in `LocalDirectoryClient`

**Location**: `libs/modkit/src/directory.rs`

```rust
#[async_trait]
impl DirectoryClient for LocalDirectoryClient {
    async fn resolve_rest_service(&self, module_name: &str) -> Result<RestEndpoint> {
        if let Some((_, _, ep)) = self.mgr.pick_rest_endpoint_round_robin(module_name) {
            return Ok(RestEndpoint::new(ep.uri));
        }

        anyhow::bail!("REST service not found or no healthy instances: {module_name}")
    }

    // ... existing methods unchanged ...
}
```

##### 1.2.5 Extend `RegisterInstanceInfo` for REST registration

**Location**: `cf_system_sdks/src/directory.rs`

```rust
/// Information needed to register a module instance
#[derive(Debug, Clone)]
pub struct RegisterInstanceInfo {
    pub module: String,
    pub instance_id: String,
    pub grpc_services: Vec<(String, ServiceEndpoint)>,
    pub rest_endpoint: Option<RestEndpoint>,  // NEW
    pub version: Option<String>,
}
```

##### 1.2.6 OoP Module REST Registration

When an OoP module starts, it registers its REST endpoint with the DirectoryService:

**Location**: `libs/modkit/src/bootstrap/oop.rs`

```rust
async fn register_with_directory(
    directory: &dyn DirectoryClient,
    module_name: &str,
    instance_id: Uuid,
    rest_port: u16,
    grpc_services: Vec<(String, ServiceEndpoint)>,
) -> Result<()> {
    let info = RegisterInstanceInfo {
        module: module_name.to_owned(),
        instance_id: instance_id.to_string(),
        grpc_services,
        rest_endpoint: Some(RestEndpoint::http("0.0.0.0", rest_port)),
        version: Some(env!("CARGO_PKG_VERSION").to_owned()),
    };

    directory.register_instance(info).await
}
```

##### 1.2.7 Design Decisions

| Decision | Rationale |
|----------|-----------|
| **One REST endpoint per instance** | Unlike gRPC (multiple services per instance), REST modules expose a single base URL with path-based routing |
| **Module-name based resolution** | REST discovery uses `module_name` (e.g., "calculator"), not service name |
| **Reuse existing health tracking** | REST endpoints use the same `InstanceState` and heartbeat mechanism as gRPC |
| **Symmetric API** | `resolve_rest_service()` mirrors `resolve_grpc_service()` for consistency |

##### 1.2.8 Migration Notes

1. **`cf_system_sdks` must be updated first** — Add `RestEndpoint`, extend `RegisterInstanceInfo`, and add `resolve_rest_service()` to `DirectoryClient`
2. **`ModuleInstance` is extended** — Existing code continues to work; `rest_endpoint` defaults to `None`
3. **OoP modules must register REST endpoints** — Update bootstrap to include REST port in registration

---

### Phase 2: ClientDescriptor Trait

**Location**: `libs/modkit/src/clients/descriptor.rs`

```rust
//! Client descriptor traits for typed OoP client metadata.

use std::time::Duration;

/// Availability policy for OoP clients.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ClientAvailabilityPolicy {
    /// Client is optional; operations fail gracefully with SDK error (maps to HTTP 424).
    #[default]
    Optional,
    /// Client is required; module readiness may depend on availability.
    Required,
}

/// Configuration for client behavior.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Transport protocol (REST default, gRPC opt-in).
    pub transport: Transport,
    /// Connection timeout for initial connect.
    pub connect_timeout: Duration,
    /// Request timeout for individual calls.
    pub request_timeout: Duration,
    /// Maximum backoff duration between reconnect attempts.
    pub max_backoff: Duration,
    /// Availability policy.
    pub availability_policy: ClientAvailabilityPolicy,
    /// Circuit breaker configuration.
    pub circuit_breaker: CircuitBreakerConfig,
    /// Optional fallback behavior when circuit is open.
    pub fallback: FallbackStrategy,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            transport: Transport::Rest,
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(30),
            max_backoff: Duration::from_secs(60),
            availability_policy: ClientAvailabilityPolicy::Optional,
        }
    }
}

impl ClientConfig {
    /// Create a REST client config (default).
    pub fn rest() -> Self {
        Self::default()
    }

    /// Create a gRPC client config (opt-in).
    #[cfg(feature = "grpc")]
    pub fn grpc() -> Self {
        Self {
            transport: Transport::Grpc,
            ..Self::default()
        }
    }
}

/// Descriptor for an OoP client, defined in SDK crates.
///
/// This trait binds compile-time type information to runtime metadata
/// needed for lazy client resolution and registration.
pub trait ClientDescriptor: Send + Sync + 'static {
    /// The SDK API trait type (e.g., `dyn CalculatorClientV1`).
    type Api: ?Sized + Send + Sync + 'static;

    /// Module name for dependency graph and Directory resolution.
    const MODULE_NAME: &'static str;

    /// Client configuration (transport, timeouts, backoff, availability).
    fn config() -> ClientConfig {
        ClientConfig::default()
    }
}
```

---

### Phase 3: ClientProvider Infrastructure

#### 3.1 RestClientProvider (Default)

**Location**: `libs/modkit/src/clients/rest_provider.rs`

```rust
//! Universal lazy REST client provider (default transport).

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use tokio::sync::Semaphore;

use crate::client_hub::ClientHub;
use crate::directory::DirectoryClient;
use crate::clients::descriptor::ClientConfig;
use modkit_http::HttpClient;

/// Error type for REST provider operations.
#[derive(Debug, thiserror::Error)]
pub enum RestProviderError {
    #[error("service not found in directory: {module_name}")]
    ServiceNotFound { module_name: &'static str },

    #[error("HTTP request failed: {0}")]
    HttpError(#[source] modkit_http::HttpError),

    #[error("directory resolution failed: {0}")]
    DirectoryError(#[source] anyhow::Error),

    #[error("service temporarily unavailable (backoff active)")]
    Backoff { retry_after: Duration },
}

struct CachedEndpoint {
    base_url: String,
    resolved_at: Instant,
}

struct ProviderState {
    cached: Option<CachedEndpoint>,
    last_failure: Option<Instant>,
    failure_count: u32,
}

/// Universal lazy REST client provider.
///
/// Handles:
/// - Lazy endpoint resolution via DirectoryClient
/// - Base URL caching with automatic eviction on errors
/// - Exponential backoff for reconnection attempts
/// - Rate limiting to prevent thundering herds
pub struct RestClientProvider {
    module_name: &'static str,
    config: ClientConfig,
    hub: Arc<ClientHub>,
    http_client: HttpClient,
    state: RwLock<ProviderState>,
    resolve_semaphore: Semaphore,
}

impl RestClientProvider {
    pub fn new(
        module_name: &'static str,
        config: ClientConfig,
        hub: Arc<ClientHub>,
    ) -> Self {
        let http_client = HttpClient::builder()
            .timeout(config.request_timeout)
            .connect_timeout(config.connect_timeout)
            .build();

        Self {
            module_name,
            config,
            hub,
            http_client,
            state: RwLock::new(ProviderState {
                cached: None,
                last_failure: None,
                failure_count: 0,
            }),
            resolve_semaphore: Semaphore::new(1),
        }
    }

    /// Get the base URL for the service, resolving lazily.
    pub async fn get_base_url(&self) -> Result<String, RestProviderError> {
        // Fast path: return cached endpoint
        {
            let state = self.state.read();
            if let Some(ref cached) = state.cached {
                return Ok(cached.base_url.clone());
            }

            // Check backoff
            if let Some(last_failure) = state.last_failure {
                let backoff = self.calculate_backoff(state.failure_count);
                let elapsed = last_failure.elapsed();
                if elapsed < backoff {
                    return Err(RestProviderError::Backoff {
                        retry_after: backoff - elapsed,
                    });
                }
            }
        }

        // Slow path: acquire semaphore and resolve
        let _permit = self.resolve_semaphore.acquire().await
            .expect("semaphore is never closed");

        // Double-check after acquiring semaphore
        {
            let state = self.state.read();
            if let Some(ref cached) = state.cached {
                return Ok(cached.base_url.clone());
            }
        }

        self.resolve_internal().await
    }

    /// Get the HTTP client for making requests.
    pub fn http_client(&self) -> &HttpClient {
        &self.http_client
    }

    /// Evict the cached endpoint (call on transport errors).
    pub fn evict(&self) {
        let mut state = self.state.write();
        state.cached = None;
        state.last_failure = Some(Instant::now());
        state.failure_count = state.failure_count.saturating_add(1);
        tracing::warn!(
            module = self.module_name,
            failure_count = state.failure_count,
            "Evicted cached REST endpoint"
        );
    }

    /// Reset failure state (call on successful request).
    pub fn reset_failures(&self) {
        let mut state = self.state.write();
        if state.failure_count > 0 {
            state.failure_count = 0;
            state.last_failure = None;
            tracing::debug!(module = self.module_name, "Reset failure state after success");
        }
    }

    async fn resolve_internal(&self) -> Result<String, RestProviderError> {
        let directory = self
            .hub
            .get::<dyn DirectoryClient>()
            .map_err(|e| RestProviderError::DirectoryError(e.into()))?;

        let endpoint = directory
            .resolve_rest_service(self.module_name)
            .await
            .map_err(RestProviderError::DirectoryError)?;

        tracing::debug!(
            module = self.module_name,
            base_url = %endpoint.base_url,
            "Resolved REST endpoint"
        );

        {
            let mut state = self.state.write();
            state.cached = Some(CachedEndpoint {
                base_url: endpoint.base_url.clone(),
                resolved_at: Instant::now(),
            });
            state.failure_count = 0;
            state.last_failure = None;
        }

        Ok(endpoint.base_url)
    }

    fn calculate_backoff(&self, failure_count: u32) -> Duration {
        let base = Duration::from_millis(100);
        let max = self.config.max_backoff;
        let backoff = base.saturating_mul(2u32.saturating_pow(failure_count.min(10)));
        backoff.min(max)
    }
}
```

#### 3.2 GrpcClientProvider (Optional)

**Location**: `libs/modkit/src/clients/grpc_provider.rs`

> Feature-gated behind `feature = "grpc"` for streaming/high-throughput use cases.

```rust
#[cfg(feature = "grpc")]
//! Lazy gRPC client provider (optional transport).

// Implementation follows same pattern as RestClientProvider
// but manages tonic::transport::Channel instead of base URL
```

#### 3.3 Retry Policy and Idempotency

**Location**: `libs/modkit/src/clients/retry.rs`

The ADR distinguishes between two retry scenarios:

1. **Endpoint resolution retries** — Handled by `RestClientProvider` backoff (inherently idempotent, read-only lookups)
2. **HTTP request retries** — Requires explicit idempotency handling for mutating operations

##### Retry Policy Configuration

```rust
/// Retry policy for transient failures.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum retry attempts (0 = no retries).
    pub max_retries: u32,
    /// Status codes that trigger a retry (e.g., 502, 503, 504).
    pub retryable_status_codes: Vec<u16>,
    /// Whether to auto-generate idempotency keys for non-GET requests.
    pub use_idempotency_keys: bool,
    /// Base delay between retries (exponential backoff applied).
    pub retry_base_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            retryable_status_codes: vec![502, 503, 504],
            use_idempotency_keys: true,
            retry_base_delay: Duration::from_millis(100),
        }
    }
}
```

Add to `ClientConfig`:

```rust
pub struct ClientConfig {
    // ... existing fields ...
    
    /// Retry policy for transient failures.
    pub retry_policy: RetryPolicy,
}
```

##### Idempotency Key Strategy

For non-idempotent HTTP methods (POST, PATCH), the lazy client generates an `Idempotency-Key` header:

```rust
impl LazyCalculatorClient {
    async fn create_calculation(&self, ctx: &SecurityContext, input: CreateInput) -> Result<Calculation, CalculatorError> {
        let base_url = self.provider.get_base_url().await?;
        let url = format!("{}/api/v1/calculations", base_url);
        
        // Generate idempotency key for POST request
        let idempotency_key = Uuid::new_v4().to_string();
        
        self.execute_with_retry(|| async {
            self.provider.http_client()
                .post(&url)
                .header("Idempotency-Key", &idempotency_key)  // Same key for all retries
                .json(&input)
                .send()
                .await
        }).await
    }
}
```

##### Idempotency Classification by HTTP Method

| Method | Idempotent? | Retry Strategy |
|--------|-------------|----------------|
| GET, HEAD, OPTIONS | ✅ Yes | Retry on 5xx/timeout |
| PUT, DELETE | ✅ Usually | Retry on 5xx/timeout |
| POST, PATCH | ❌ No | Retry only with `Idempotency-Key` header |

##### Server-Side Contract

OoP modules receiving requests with `Idempotency-Key` header **must**:

1. Store `(idempotency_key, tenant_id) → response` mapping with TTL (recommended: 24 hours)
2. Return cached response if key+tenant combination seen before
3. Process request normally if key is new

```rust
// Example server-side middleware (in OoP module)
async fn idempotency_middleware(
    State(cache): State<IdempotencyCache>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    let Some(key) = headers.get("Idempotency-Key") else {
        return next.run(request).await;
    };
    
    // Check for cached response
    if let Some(cached) = cache.get(&key).await {
        return cached;
    }
    
    // Process request and cache response
    let response = next.run(request).await;
    cache.set(&key, &response, Duration::from_secs(86400)).await;
    response
}
```

##### Scope Note

This ADR focuses on **client-side** idempotency (generating keys, retry logic). Server-side idempotency handling is the responsibility of each OoP module and should follow the contract above. A future ADR may standardize server-side idempotency middleware in ModKit.

---

#### 3.4 Circuit Breaking and Fallback Strategy

**Location**: `libs/modkit/src/clients/circuit_breaker.rs`

Circuit breaking prevents cascading failures by temporarily stopping requests to a failing OoP module, allowing it time to recover.

##### Circuit Breaker States

```text
     ┌────────────────────────────────────────────────────────────┐
     │                                                            │
     ▼                                                            │
┌─────────┐   failure_threshold  ┌──────┐   reset_timeout   ┌─────────────┐
│ CLOSED  │ ──────────────────▶  │ OPEN │ ────────────────▶ │ HALF-OPEN   │
│(normal) │   reached            │(fail │   elapsed         │(probe)      │
└─────────┘                      │ fast)│                   └──────┬──────┘
     ▲                           └──────┘                          │
     │                               ▲                             │
     │                               │ probe fails                 │
     │                               └─────────────────────────────┤
     │                                                             │
     │                          probe succeeds                     │
     └─────────────────────────────────────────────────────────────┘
```

##### Circuit Breaker Configuration

```rust
/// Circuit breaker configuration.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening the circuit.
    pub failure_threshold: u32,
    /// Duration to keep circuit open before allowing a probe request.
    pub reset_timeout: Duration,
    /// Number of successful probes required to close the circuit.
    pub success_threshold: u32,
    /// Whether circuit breaker is enabled.
    pub enabled: bool,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            reset_timeout: Duration::from_secs(30),
            success_threshold: 2,
            enabled: true,
        }
    }
}
```

Add to `ClientConfig`:

```rust
pub struct ClientConfig {
    // ... existing fields ...
    
    /// Circuit breaker configuration.
    pub circuit_breaker: CircuitBreakerConfig,
    /// Optional fallback behavior when circuit is open.
    pub fallback: FallbackStrategy,
}
```

##### Circuit Breaker Implementation

```rust
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: parking_lot::RwLock<CircuitState>,
    failure_count: AtomicU32,
    success_count: AtomicU32,
    last_failure_time: AtomicU64,  // Unix timestamp millis
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: parking_lot::RwLock::new(CircuitState::Closed),
            failure_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
            last_failure_time: AtomicU64::new(0),
        }
    }

    /// Check if request should be allowed.
    pub fn allow_request(&self) -> bool {
        if !self.config.enabled {
            return true;
        }

        let state = *self.state.read();
        match state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if reset timeout has elapsed
                let last_failure = self.last_failure_time.load(Ordering::Relaxed);
                let elapsed = Duration::from_millis(
                    now_millis().saturating_sub(last_failure)
                );
                if elapsed >= self.config.reset_timeout {
                    // Transition to half-open
                    *self.state.write() = CircuitState::HalfOpen;
                    self.success_count.store(0, Ordering::Relaxed);
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,  // Allow probe requests
        }
    }

    /// Record a successful request.
    pub fn record_success(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
        
        let state = *self.state.read();
        if state == CircuitState::HalfOpen {
            let successes = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;
            if successes >= self.config.success_threshold {
                *self.state.write() = CircuitState::Closed;
                tracing::info!("Circuit breaker closed after successful probes");
            }
        }
    }

    /// Record a failed request.
    pub fn record_failure(&self) {
        self.last_failure_time.store(now_millis(), Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        
        let state = *self.state.read();
        match state {
            CircuitState::Closed => {
                let failures = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
                if failures >= self.config.failure_threshold {
                    *self.state.write() = CircuitState::Open;
                    tracing::warn!(
                        threshold = self.config.failure_threshold,
                        "Circuit breaker opened after consecutive failures"
                    );
                }
            }
            CircuitState::HalfOpen => {
                // Probe failed, reopen circuit
                *self.state.write() = CircuitState::Open;
                tracing::warn!("Circuit breaker reopened after probe failure");
            }
            CircuitState::Open => {}  // Already open
        }
    }

    pub fn state(&self) -> CircuitState {
        *self.state.read()
    }
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
```

##### Fallback Strategy

When the circuit is open, the client can apply a fallback strategy:

```rust
/// Fallback behavior when circuit breaker is open.
#[derive(Debug, Clone, Default)]
pub enum FallbackStrategy {
    /// Return error immediately (fail fast). Default behavior.
    #[default]
    FailFast,
    /// Return a cached response if available.
    CachedResponse {
        /// Maximum age of cached response to use.
        max_age: Duration,
    },
    /// Return a static default value (SDK must implement).
    StaticDefault,
    /// Call an alternative service.
    AlternativeService {
        /// Module name of the fallback service.
        fallback_module: &'static str,
    },
}
```

##### Integration with RestClientProvider

```rust
impl RestClientProvider {
    pub async fn execute<F, T, E>(&self, request_fn: F) -> Result<T, E>
    where
        F: FnOnce(&str) -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: From<RestProviderError>,
    {
        // Check circuit breaker
        if !self.circuit_breaker.allow_request() {
            tracing::debug!(module = self.module_name, "Circuit breaker is open");
            return self.apply_fallback().await;
        }

        let base_url = self.get_base_url().await?;
        
        match request_fn(&base_url).await {
            Ok(response) => {
                self.circuit_breaker.record_success();
                self.reset_failures();
                Ok(response)
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                self.evict();
                Err(e)
            }
        }
    }

    async fn apply_fallback<T, E>(&self) -> Result<T, E>
    where
        E: From<RestProviderError>,
    {
        match &self.config.fallback {
            FallbackStrategy::FailFast => {
                Err(RestProviderError::CircuitOpen {
                    module_name: self.module_name,
                }.into())
            }
            FallbackStrategy::CachedResponse { max_age } => {
                // Implementation depends on response caching layer
                todo!("Return cached response if available and fresh")
            }
            FallbackStrategy::StaticDefault => {
                // SDK must override this behavior
                Err(RestProviderError::CircuitOpen {
                    module_name: self.module_name,
                }.into())
            }
            FallbackStrategy::AlternativeService { fallback_module } => {
                // Resolve and call alternative module
                todo!("Route to fallback module")
            }
        }
    }
}
```

##### Error Type Extension

```rust
pub enum RestProviderError {
    // ... existing variants ...

    #[error("circuit breaker open for module: {module_name}")]
    CircuitOpen { module_name: &'static str },
}
```

##### Circuit Breaker vs Backoff

| Mechanism | Purpose | Scope |
|-----------|---------|-------|
| **Backoff** | Rate-limit reconnection attempts after endpoint resolution failure | Endpoint discovery |
| **Circuit Breaker** | Stop all requests to a failing service, allow recovery | Request execution |

Both work together:
- Backoff prevents hammering the directory service
- Circuit breaker prevents hammering a failing OoP module

##### Observability

Circuit breaker state changes should emit metrics and logs:

```rust
// Metrics (example with prometheus)
circuit_breaker_state.with_label_values(&[module_name]).set(state as i64);
circuit_breaker_failures_total.with_label_values(&[module_name]).inc();
circuit_breaker_opens_total.with_label_values(&[module_name]).inc();

// Structured logging
tracing::warn!(
    module = module_name,
    state = ?new_state,
    failure_count = failures,
    "Circuit breaker state changed"
);
```

---

#### 3.5 Non-Existent Modules and API Version Incompatibility

##### Non-Existent Module Handling

When a lazy client attempts to resolve a module that doesn't exist (never registered, misconfigured, or permanently removed):

```rust
// DirectoryClient returns error
async fn resolve_rest_service(&self, module_name: &str) -> Result<RestEndpoint> {
    if let Some((_, _, ep)) = self.mgr.pick_rest_endpoint_round_robin(module_name) {
        return Ok(RestEndpoint::new(ep.uri));
    }
    
    // Module not found - could be:
    // 1. Module never registered (misconfiguration)
    // 2. Module temporarily unavailable (will retry with backoff)
    // 3. Module permanently removed (configuration error)
    anyhow::bail!("REST service not found or no healthy instances: {module_name}")
}
```

**Behavior:**
1. `RestClientProvider::get_base_url()` returns `RestProviderError::ServiceNotFound`
2. Backoff is applied (same as transient failures)
3. After `max_retries`, circuit breaker opens
4. Lazy client returns SDK-specific error (e.g., `CalculatorError::Unavailable`)
5. REST handler maps to **HTTP 424 Failed Dependency**

**Detection vs Transient Failure:**

The system cannot distinguish between "module doesn't exist" and "module temporarily unavailable" at runtime. This is intentional:
- Avoids hardcoding module existence assumptions
- Allows modules to be deployed/undeployed dynamically
- Same graceful degradation path for both cases

For **startup validation** of required dependencies, use `ClientAvailabilityPolicy::Required`:

```rust
impl ClientDescriptor for CalculatorClientDescriptor {
    // ...
    fn config() -> ClientConfig {
        ClientConfig {
            availability_policy: ClientAvailabilityPolicy::Required,
            ..ClientConfig::rest()
        }
    }
}
```

With `Required` policy, the module's readiness probe will fail until the dependency is resolvable.

##### API Version Incompatibility

When the OoP module's API version is incompatible with the client SDK:

**Detection Points:**

| Detection Point | Error Type | Handling |
|-----------------|------------|----------|
| **Response parsing** | `ParseError` (missing/extra fields) | SDK error, HTTP 502 |
| **HTTP 404 on endpoint** | Endpoint doesn't exist in target version | SDK error, HTTP 424 |
| **HTTP 400 Bad Request** | Request schema mismatch | SDK error, HTTP 502 |
| **Explicit version header** | `X-API-Version` mismatch | SDK error, HTTP 424 |

**Version Header Strategy (Recommended):**

SDKs should include an API version header, and OoP modules should validate it:

```rust
// Client-side (LazyCalculatorClient)
impl LazyCalculatorClient {
    const API_VERSION: &'static str = "v1";

    async fn add(&self, ctx: &SecurityContext, a: i64, b: i64) -> Result<i64, CalculatorError> {
        let base_url = self.provider.get_base_url().await?;
        let url = format!("{}/api/v1/calculator/add", base_url);
        
        let response = self.provider.http_client()
            .post(&url)
            .header("X-API-Version", Self::API_VERSION)
            .header("x-tenant-id", ctx.tenant_id_str())
            .json(&serde_json::json!({ "a": a, "b": b }))
            .send()
            .await?;

        // Check for version mismatch response
        if response.status() == http::StatusCode::NOT_ACCEPTABLE {
            return Err(CalculatorError::VersionMismatch {
                expected: Self::API_VERSION.to_string(),
                actual: response.headers()
                    .get("X-API-Version")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("unknown")
                    .to_string(),
            });
        }
        // ... rest of handling
    }
}
```

```rust
// Server-side middleware (OoP module)
async fn version_check_middleware(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    const SUPPORTED_VERSIONS: &[&str] = &["v1", "v1.1"];
    
    if let Some(client_version) = headers.get("X-API-Version") {
        let version = client_version.to_str().unwrap_or("");
        if !SUPPORTED_VERSIONS.contains(&version) {
            return Response::builder()
                .status(StatusCode::NOT_ACCEPTABLE)
                .header("X-API-Version", SUPPORTED_VERSIONS.join(", "))
                .body(format!(
                    "API version '{}' not supported. Supported: {:?}",
                    version, SUPPORTED_VERSIONS
                ))
                .unwrap();
        }
    }
    
    next.run(request).await
}
```

**Error Type Extension:**

```rust
pub enum LazyClientError {
    // ... existing variants ...

    #[error("API version mismatch: client={expected}, server={actual}")]
    VersionMismatch {
        expected: String,
        actual: String,
    },

    #[error("module not found: {module_name}")]
    ModuleNotFound {
        module_name: &'static str,
    },
}
```

**HTTP Status Code Mapping:**

| Error | HTTP Status | Rationale |
|-------|-------------|-----------|
| Module not found | 424 Failed Dependency | Dependency unavailable |
| Version mismatch | 424 Failed Dependency | Dependency incompatible |
| Parse error (schema drift) | 502 Bad Gateway | Upstream returned invalid response |
| Request rejected (400) | 502 Bad Gateway | Upstream rejected our request |

##### Version Compatibility Matrix

SDKs should document their compatibility:

```rust
/// Calculator SDK v2.0
/// 
/// ## API Compatibility
/// 
/// | SDK Version | Module Versions Supported |
/// |-------------|---------------------------|
/// | 2.0.x       | calculator v2.0+          |
/// | 1.5.x       | calculator v1.5 - v1.9    |
/// | 1.0.x       | calculator v1.0 - v1.4    |
pub struct CalculatorClientDescriptor;
```

---

#### 3.6 LazyClientError

**Location**: `libs/modkit/src/clients/error.rs`

```rust
/// Error returned by lazy clients when the OoP dependency is unavailable.
#[derive(Debug, thiserror::Error)]
pub enum LazyClientError {
    #[error("service unavailable: {module_name}")]
    Unavailable {
        module_name: &'static str,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("module not found: {module_name}")]
    ModuleNotFound {
        module_name: &'static str,
    },

    #[error("API version mismatch: client={expected}, server={actual}")]
    VersionMismatch {
        expected: String,
        actual: String,
    },

    #[error("request failed: {0}")]
    RequestFailed(String),

    #[error("response parsing failed: {0}")]
    ParseError(#[source] serde_json::Error),
}

impl LazyClientError {
    /// Returns true if this error indicates the service is temporarily unavailable.
    /// REST handlers should map this to HTTP 424 Failed Dependency.
    pub fn is_dependency_unavailable(&self) -> bool {
        matches!(
            self,
            LazyClientError::Unavailable { .. }
                | LazyClientError::ModuleNotFound { .. }
                | LazyClientError::VersionMismatch { .. }
        )
    }

    /// Returns true if this error indicates a permanent incompatibility.
    pub fn is_permanent(&self) -> bool {
        matches!(self, LazyClientError::VersionMismatch { .. })
    }
}
```

---

#### 3.7 Initialization Order and Circular Dependencies

##### DirectoryClient Bootstrap Guarantee

The `DirectoryClient` is **always available** before any lazy client attempts resolution. This is guaranteed by the bootstrap sequence:

**OoP Module Bootstrap** (`libs/modkit/src/bootstrap/oop.rs`):
```rust
// 1. Connect to directory service FIRST (before module init)
let directory_client = DirectoryGrpcClient::connect(&opts.directory_endpoint).await?;
let directory_api: Arc<dyn DirectoryClient> = Arc::new(directory_client);

// 2. Inject DirectoryClient into ClientHub via RunOptions
let run_options = RunOptions {
    clients: vec![ClientRegistration::new::<dyn DirectoryClient>(directory_api)],
    // ...
};

// 3. Only then run module lifecycle (init → start → ready)
run(run_options).await
```

**Host Runtime Bootstrap** (`libs/modkit/src/bootstrap/run.rs`):
```rust
// For in-process host, DirectoryClient is the LocalDirectoryClient
// which is created during HostRuntime construction, before any module init
```

**Initialization Order:**
```
1. Bootstrap connects to DirectoryService (OoP) or creates LocalDirectoryClient (host)
2. DirectoryClient registered in ClientHub
3. Module registry discovered
4. HostRuntime created with ClientHub containing DirectoryClient
5. Modules initialized (init phase) — lazy clients created but NOT resolved
6. Modules started (start phase)
7. First API call triggers lazy resolution — DirectoryClient guaranteed present
```

##### Why Lazy Clients Never Fail on Missing DirectoryClient

Lazy clients don't resolve during `init()` or `start()`. Resolution happens on **first use**:

```rust
impl LazyCalculatorClient {
    async fn add(&self, ctx: &SecurityContext, a: i64, b: i64) -> Result<i64, CalculatorError> {
        // Resolution happens HERE, not during construction
        let base_url = self.provider.get_base_url().await?;
        // ...
    }
}
```

By the time any API call is made, the module is already in `Running` state, which means:
- Bootstrap completed successfully
- `DirectoryClient` is in `ClientHub`
- HTTP server is accepting requests

##### Circular Module Dependencies

**Scenario:** Module A depends on Module B, and Module B depends on Module A.

```
┌──────────┐         ┌──────────┐
│ Module A │ ──────▶ │ Module B │
│          │ ◀────── │          │
└──────────┘         └──────────┘
```

**With lazy clients, this works:**

1. Both modules start independently (no blocking on dependency availability)
2. Module A's first call to B triggers lazy resolution
3. Module B's first call to A triggers lazy resolution
4. As long as both are registered in the directory, calls succeed

**Failure modes:**
- If A calls B before B is registered → backoff + retry (eventually succeeds)
- If A calls B and B is permanently down → circuit breaker opens, HTTP 424

##### Multi-Exec / Parallel Startup

In multi-exec scenarios (multiple processes starting simultaneously):

```
Process 1 (Host)          Process 2 (OoP Module)
─────────────────         ─────────────────────
1. Start                  1. Start
2. Create LocalDirectory  2. Connect to DirectoryService
3. Start gRPC hub         3. Wait for gRPC hub endpoint
4. Spawn OoP modules      4. Register with directory
5. Modules init           5. Module init
6. Modules start          6. Module start
```

**Key guarantee:** OoP modules wait for the host's gRPC hub to be ready before connecting:

```rust
// OoP bootstrap waits for directory endpoint
let directory_endpoint = std::env::var(MODKIT_DIRECTORY_ENDPOINT_ENV)?;
let directory_client = DirectoryGrpcClient::connect(&directory_endpoint).await?;
```

The `MODKIT_DIRECTORY_ENDPOINT_ENV` is only set by the host **after** the gRPC hub is bound and ready.

##### Race Condition Mitigations

| Race Condition | Mitigation |
|----------------|------------|
| Lazy client resolves before target module registered | Backoff + retry; circuit breaker if persistent |
| DirectoryClient not in ClientHub | Impossible — bootstrap registers it before module init |
| Multiple processes racing to register | Directory handles concurrent registrations; round-robin picks any healthy instance |
| Module A calls B while B is still in `init()` | B not yet registered; A's call backs off until B reaches `start()` and registers |

##### Required vs Optional Dependencies

For **critical** circular dependencies where both must be available at startup:

```rust
impl ClientDescriptor for CalculatorClientDescriptor {
    fn config() -> ClientConfig {
        ClientConfig {
            availability_policy: ClientAvailabilityPolicy::Required,
            ..ClientConfig::rest()
        }
    }
}
```

With `Required` policy:
- Module's readiness probe fails until dependency is resolvable
- Kubernetes/orchestrator won't route traffic until both are ready
- Prevents serving requests that would immediately fail

---

### Phase 4: SDK Crate Updates (calculator-sdk example)

#### 4.1 Descriptor

**Location**: `calculator-sdk/src/descriptor.rs`

```rust
use modkit::clients::descriptor::{ClientDescriptor, ClientConfig};
use crate::api::CalculatorClientV1;

/// Descriptor for the Calculator client (REST by default).
pub struct CalculatorClientDescriptor;

impl ClientDescriptor for CalculatorClientDescriptor {
    type Api = dyn CalculatorClientV1;
    const MODULE_NAME: &'static str = "calculator";

    fn config() -> ClientConfig {
        ClientConfig::rest()  // Default: REST transport
    }
}

// Optional: gRPC descriptor for high-throughput use cases
#[cfg(feature = "grpc")]
pub struct CalculatorGrpcClientDescriptor;

#[cfg(feature = "grpc")]
impl ClientDescriptor for CalculatorGrpcClientDescriptor {
    type Api = dyn CalculatorClientV1;
    const MODULE_NAME: &'static str = "calculator";

    fn config() -> ClientConfig {
        ClientConfig::grpc()  // Opt-in: gRPC transport
    }
}
```

#### 4.2 Lazy Client Implementation

**Location**: `calculator-sdk/src/lazy_client.rs`

```rust
use std::sync::Arc;
use async_trait::async_trait;
use modkit::clients::rest_provider::RestClientProvider;
use modkit_security::SecurityContext;

use crate::api::{CalculatorClientV1, CalculatorError};

/// Lazy client for Calculator service (REST transport).
pub struct LazyCalculatorClient {
    provider: Arc<RestClientProvider>,
}

impl LazyCalculatorClient {
    pub fn new(provider: Arc<RestClientProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl CalculatorClientV1 for LazyCalculatorClient {
    async fn add(&self, ctx: &SecurityContext, a: i64, b: i64) -> Result<i64, CalculatorError> {
        let base_url = self.provider.get_base_url().await.map_err(|e| {
            tracing::warn!(error = %e, "Calculator service unavailable");
            CalculatorError::Unavailable {
                message: format!("Calculator service unavailable: {}", e),
            }
        })?;

        let url = format!("{}/api/v1/calculator/add", base_url);
        let response = self.provider.http_client()
            .post(&url)
            .json(&serde_json::json!({ "a": a, "b": b }))
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() || e.is_timeout() {
                    self.provider.evict();
                }
                CalculatorError::Unavailable {
                    message: format!("HTTP request failed: {}", e),
                }
            })?;

        if !response.status().is_success() {
            return Err(map_http_error(response.status(), &response.text().await.unwrap_or_default()));
        }

        let result: AddResponse = response.json().await.map_err(|e| {
            CalculatorError::Internal { message: format!("Failed to parse response: {}", e) }
        })?;

        self.provider.reset_failures();
        Ok(result.result)
    }

    // ... other methods follow the same pattern ...
}

#[derive(serde::Deserialize)]
struct AddResponse { result: i64 }

/// Maps HTTP status codes to SDK errors.
/// Note: Signature should use http::StatusCode to match modkit_http::HttpClient shown above.
fn map_http_error(status: http::StatusCode, body: &str) -> CalculatorError {
    match status.as_u16() {
        400 => CalculatorError::InvalidArgument { message: body.to_string() },
        404 => CalculatorError::NotFound { message: body.to_string() },
        503 => CalculatorError::Unavailable { message: body.to_string() },
        _ => CalculatorError::Internal { message: format!("HTTP {}: {}", status, body) },
    }
}
```

---

### Phase 5: Module Macro Extension

**Location**: `libs/modkit-macros/src/module.rs`

The `#[modkit::module]` macro is extended to support `clients = [...]`:

```rust
#[modkit::module(
    name = "calculator_gateway",
    capabilities = [rest],
    clients = [calculator_sdk::CalculatorClientDescriptor],
    // Note: deps is auto-injected from clients; no need to specify manually.
)]
pub struct CalculatorGateway;
```

**Generated code** (simplified):

```rust
impl CalculatorGateway {
    fn __register_lazy_clients(ctx: &ModuleCtx) -> anyhow::Result<()> {
        use modkit::clients::descriptor::{ClientDescriptor, Transport};

        type D = calculator_sdk::CalculatorClientDescriptor;
        let config = D::config();

        let lazy_client: Arc<<D as ClientDescriptor>::Api> = match config.transport {
            Transport::Rest => {
                let provider = Arc::new(RestClientProvider::new(
                    D::MODULE_NAME,
                    config,
                    ctx.client_hub(),
                ));
                Arc::new(calculator_sdk::LazyCalculatorClient::new(provider))
            }
            #[cfg(feature = "grpc")]
            Transport::Grpc => {
                let provider = Arc::new(GrpcClientProvider::new(
                    D::MODULE_NAME,
                    config,
                    ctx.client_hub(),
                ));
                Arc::new(calculator_sdk::LazyCalculatorGrpcClient::new(provider))
            }
        };

        ctx.client_hub().register::<<D as ClientDescriptor>::Api>(lazy_client);
        Ok(())
    }
}
```

---

### Phase 6: Registry Extension for Soft OoP Deps

**Location**: `libs/modkit/src/registry.rs`

```rust
impl ModuleRegistry {
    /// Resolve dependencies, treating unknown deps as potential OoP soft deps.
    pub fn resolve_dependencies_with_oop(
        &self,
        module_name: &str,
        deps: &[&str],
        config: &AppConfig,
    ) -> Result<ResolvedDeps, RegistryError> {
        let mut hard_deps = Vec::new();
        let mut soft_deps = Vec::new();

        for dep in deps {
            if self.has_module(dep) {
                hard_deps.push(*dep);  // In-process → topo-sort
            } else if config.is_oop_module(dep) {
                soft_deps.push(*dep);  // OoP → no topo-sort, lazy resolution
            } else {
                return Err(RegistryError::UnknownDependency {
                    module: module_name.to_string(),
                    dependency: dep.to_string(),
                });
            }
        }

        Ok(ResolvedDeps { hard_deps, soft_deps })
    }
}

pub struct ResolvedDeps {
    pub hard_deps: Vec<&'static str>,
    pub soft_deps: Vec<&'static str>,
}
```

---

## Consumer Module Changes

### Before

```rust
#[modkit::module(name = "calculator_gateway", capabilities = [rest], deps = ["calculator"])]
pub struct CalculatorGateway;

impl modkit::Module for CalculatorGateway {
    async fn init(&self, ctx: &ModuleCtx) -> Result<()> {
        // Must wire client manually - FAILS if calculator not ready
        let directory = ctx.client_hub().get::<dyn DirectoryClient>()?;
        calculator_sdk::wire_client(ctx.client_hub(), &*directory).await?;
        // ...
    }
}
```

### After

```rust
#[modkit::module(
    name = "calculator_gateway",
    capabilities = [rest],
    clients = [calculator_sdk::CalculatorClientDescriptor],
)]
pub struct CalculatorGateway;

impl modkit::Module for CalculatorGateway {
    async fn init(&self, ctx: &ModuleCtx) -> Result<()> {
        // No wire_client() needed! LazyCalculatorClient is auto-registered.
        let service = Arc::new(Service::new(ctx.client_hub()));
        ctx.client_hub().register::<Service>(service);
        Ok(())
    }
}
```

---

## Error Handling and HTTP 424

Lazy clients return typed errors that map to HTTP 424 Failed Dependency:

```rust
impl From<ServiceError> for Problem {
    fn from(err: ServiceError) -> Self {
        match err {
            ServiceError::DependencyUnavailable { service, source } => {
                Problem::failed_dependency()
                    .with_detail(format!("{} unavailable: {}", service, source))
            }
            ServiceError::RemoteError(msg) => {
                Problem::bad_gateway().with_detail(msg)
            }
            ServiceError::Internal(msg) => {
                Problem::internal_server_error().with_detail(msg)
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("dependency unavailable: {service}")]
    DependencyUnavailable {
        service: &'static str,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("remote error: {0}")]
    RemoteError(String),
    #[error("internal error: {0}")]
    Internal(String),
}
```

---

## Implementation Timeline

### Week 1-2: Phase 1 (REST as Default)
1. Add `Transport` enum to `libs/modkit/src/clients/transport.rs`
2. Extend `DirectoryClient` with `resolve_rest_service()` method
3. Update config structures for transport selection

### Week 2-3: Phase 2-3 (Descriptor + Provider)
1. Add `ClientDescriptor` trait to `libs/modkit/src/clients/descriptor.rs`
2. Implement `RestClientProvider` in `libs/modkit/src/clients/rest_provider.rs`
3. Add `LazyClientError` type
4. (Optional) Implement `GrpcClientProvider` behind feature flag
5. Unit tests for providers (mock DirectoryClient)

### Week 3-4: Phase 4 (SDK Updates)
1. Add `CalculatorClientDescriptor` to calculator-sdk (REST by default)
2. Implement `LazyCalculatorClient` with REST transport
3. Integration tests with mock HTTP server

### Week 4-5: Phase 5 (Macro Extension)
1. Extend `#[modkit::module]` to parse `clients = [...]`
2. Generate lazy client registration code with transport selection
3. Auto-augment `deps` with module names from descriptors

### Week 5-6: Phase 6 (Registry + Migration)
1. Implement soft OoP dep resolution in registry
2. Update calculator_gateway example
3. Rename `docs/modkit_unified_system/09_oop_grpc_sdk_pattern.md` to `09_oop_sdk_pattern.md`
4. Add migration guide

---

## Testing Strategy

### Unit Tests
- `RestClientProvider`: endpoint caching, backoff, eviction
- `GrpcClientProvider` (feature-gated): channel caching, backoff, eviction
- `LazyCalculatorClient`: error mapping, context propagation
- Registry: soft dep resolution

### Integration Tests
- Startup with unavailable OoP → module starts successfully
- First REST call triggers lazy endpoint resolution
- HTTP error → backoff → retry
- Successful call → failure state reset

### E2E Tests
- calculator_gateway starts without calculator OoP
- REST call returns 424 when calculator unavailable
- REST call succeeds after calculator becomes available

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Macro complexity | Start with manual lazy client impl; add codegen later |
| Breaking existing SDKs | Backward-compatible: `wire_client()` still works |
| Performance overhead | Provider uses fast-path caching; no overhead on hot path |
| Debugging difficulty | Detailed tracing in provider and lazy client |
| JSON overhead vs protobuf | Acceptable for most use cases; gRPC available for high-throughput |

---

## Success Criteria

1. **No eager wiring**: Consumer modules do not call `wire_client()` in `init()`
2. **Graceful startup**: Modules start even if OoP dependencies are unavailable
3. **Per-operation degradation**: Missing OoP → HTTP 424 for affected endpoints only
4. **Single source of truth**: `clients = [...]` declares all OoP dependencies
5. **REST by default**: All OoP clients use REST transport unless explicitly configured for gRPC
6. **Consistent behavior**: All clients (REST or gRPC) use the same provider infrastructure

---

## Appendix: File Structure

```text
libs/modkit/src/
├── clients/
│   ├── mod.rs              # Module exports
│   ├── transport.rs        # Transport enum
│   ├── descriptor.rs       # ClientDescriptor trait, ClientConfig
│   ├── rest_provider.rs    # RestClientProvider (default)
│   ├── grpc_provider.rs    # GrpcClientProvider (feature = "grpc")
│   └── error.rs            # LazyClientError, ProviderError
├── lib.rs                  # Add `pub mod clients;`
└── ...

libs/modkit-macros/src/
├── module.rs               # Extended to parse `clients = [...]`
└── ...

examples/oop-modules/calculator/calculator-sdk/src/
├── descriptor.rs           # CalculatorClientDescriptor
├── lazy_client.rs          # LazyCalculatorClient
├── lib.rs                  # Updated exports
└── ...
```
