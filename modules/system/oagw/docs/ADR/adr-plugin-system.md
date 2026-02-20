# ADR: Plugin System

**ID**: `cpt-cf-oagw-adr-plugin-system`

- **Status**: Accepted
- **Date**: 2026-02-09
- **Deciders**: OAGW Team

## Context and Problem Statement

OAGW needs extensibility for request/response processing. Different use cases require different behaviors:

- Authentication: API key, OAuth2, JWT, custom schemes
- Validation: Timeouts, CORS, rate limiting, custom rules
- Transformation: Logging, metrics, request ID, custom headers

## Decision Drivers

- Clear separation of plugin responsibilities (auth, validation, transformation)
- Same trait interface for built-in and external plugins
- Native Rust performance for hot-path operations
- Modkit integration for external plugin modules
- Extensibility without modifying OAGW core

## Considered Options

1. **Three Plugin Types** (Recommended): AuthPlugin, GuardPlugin, TransformPlugin with distinct traits
2. **Single Extension Trait**: One generic trait for all plugin purposes
3. **Starlark Only**: Interpreted scripts for all plugins

## Decision Outcome

**Chosen**: Option 1 — Plugin system with three plugin types, executed by Data Plane:

### Plugin Types

**1. AuthPlugin** (`gts.x.core.oagw.auth_plugin.v1~*`)

- Purpose: Inject authentication credentials
- Execution: Once per request, before guards
- Examples: API key, OAuth2, Bearer token, Basic auth

**2. GuardPlugin** (`gts.x.core.oagw.guard_plugin.v1~*`)

- Purpose: Validate requests and enforce policies (can reject)
- Execution: After auth, before transform
- Examples: Timeout enforcement, CORS validation, rate limiting

**3. TransformPlugin** (`gts.x.core.oagw.transform_plugin.v1~*`)

- Purpose: Modify request/response/error data
- Execution: Before and after proxy call
- Examples: Logging, metrics collection, request ID propagation

### Plugin Traits

```rust
// oagw-sdk/src/plugins.rs

#[async_trait]
pub trait AuthPlugin: Send + Sync {
    fn id(&self) -> &str;
    fn plugin_type(&self) -> &str;
    async fn authenticate(&self, ctx: &mut RequestContext) -> Result<()>;
}

#[async_trait]
pub trait GuardPlugin: Send + Sync {
    fn id(&self) -> &str;
    fn plugin_type(&self) -> &str;
    async fn guard_request(&self, ctx: &RequestContext) -> Result<GuardDecision>;
    async fn guard_response(&self, ctx: &ResponseContext) -> Result<GuardDecision>;
}

#[async_trait]
pub trait TransformPlugin: Send + Sync {
    fn id(&self) -> &str;
    fn plugin_type(&self) -> &str;
    async fn transform_request(&self, ctx: &mut RequestContext) -> Result<()>;
    async fn transform_response(&self, ctx: &mut ResponseContext) -> Result<()>;
    async fn transform_error(&self, ctx: &mut ErrorContext) -> Result<()>;
}
```

### Plugin Execution Order

```
Incoming Request
  → Auth Plugin (credential injection)
  → Guard Plugins (validation, can reject)
  → Transform Plugins (modify request)
  → HTTP call to external service
  → Transform Plugins (modify response)
  → Return to client
```

### Built-in Plugins

Included in `oagw` crate (`infra/plugin/`):

**Auth Plugins**:

- `ApiKeyAuthPlugin`: API key injection (header/query)
- `BasicAuthPlugin`: HTTP Basic authentication
- `BearerTokenAuthPlugin`: Bearer token injection
- `OAuth2ClientCredPlugin`: OAuth2 client credentials flow

**Guard Plugins**:

- `TimeoutGuardPlugin`: Request timeout enforcement
- `CorsGuardPlugin`: CORS preflight validation
- `RateLimitGuardPlugin`: Rate limiting (token bucket)

**Transform Plugins**:

- `LoggingTransformPlugin`: Request/response logging
- `MetricsTransformPlugin`: Prometheus metrics collection
- `RequestIdTransformPlugin`: X-Request-ID propagation

### External Plugins

Separate modkit modules implementing plugin traits:

```rust
// cf-oagw-plugin-oauth2-pkce/src/lib.rs

pub struct OAuth2PkceAuthPlugin {
    // ...
}

#[async_trait]
impl AuthPlugin for OAuth2PkceAuthPlugin {
    fn id(&self) -> &str { "oauth2-pkce" }
    fn plugin_type(&self) -> &str {
        "gts.x.core.oagw.auth_plugin.v1~custom.oauth2.oagw.pkce.v1"
    }
    async fn authenticate(&self, ctx: &mut RequestContext) -> Result<()> {
        // Custom OAuth2 PKCE flow
    }
}
```

### Plugin Loading

```rust
// Data Plane loads plugins during initialization
pub struct ControlPlane {
    auth_plugins: HashMap<String, Arc<dyn AuthPlugin>>,
    guard_plugins: HashMap<String, Arc<dyn GuardPlugin>>,
    transform_plugins: HashMap<String, Arc<dyn TransformPlugin>>,
}

impl ControlPlane {
    pub fn new(external_plugins: Vec<Box<dyn AuthPlugin>>) -> Self {
        let mut auth_plugins = HashMap::new();

        // Register built-in plugins
        auth_plugins.insert("apikey".into(), Arc::new(ApiKeyAuthPlugin));
        auth_plugins.insert("basic".into(), Arc::new(BasicAuthPlugin));

        // Register external plugins from modkit
        for plugin in external_plugins {
            auth_plugins.insert(plugin.id().to_string(), Arc::from(plugin));
        }

        Self { auth_plugins, /* ... */ }
    }
}
```

### Rationale

- **Clear trait boundaries**: Each plugin type has specific purpose
- **Same traits for built-in and external**: No special-casing
- **Modkit integration**: External plugins are modkit modules
- **Native Rust performance**: No WASM overhead for MVP
- **Type safety**: Compile-time guarantees for built-in plugins

### Consequences

### Positive

- Extensibility without modifying OAGW core
- Built-in plugins have zero overhead (native code)
- External plugins integrate via modkit (standard pattern)
- Clear execution order and lifecycle

### Negative

- External plugins require Rust implementation (no scripting languages yet)
- Plugin changes require recompilation (acceptable for MVP)

### Future

- **Starlark plugins**: For simple transforms (p3)
- **WASM plugins**: For sandboxed untrusted code (p3)

### Confirmation

Confirmation will be achieved through:

- Unit tests for each plugin trait (AuthPlugin, GuardPlugin, TransformPlugin)
- Integration tests verifying plugin execution order (auth → guard → transform)
- External plugin loading via modkit module registration

## Pros and Cons of the Options

### Option 1: Three Plugin Types

- **Good**: Clear trait boundaries, type-safe, same interface for built-in and external
- **Bad**: External plugins require Rust implementation, changes require recompilation

### Option 2: Single Extension Trait

- **Good**: Simple, fewer traits to maintain
- **Bad**: Too generic, loses type safety and clear semantics

### Option 3: Starlark Only

- **Good**: Easy scripting, no recompilation needed
- **Bad**: Too slow for hot path operations (auth, guards)

## Alternatives Considered

### Alternative: Single Extension Trait

One generic `Extension` trait for all purposes.

**Rejected**: Too generic, loses type safety and clear semantics.

### Alternative: Starlark Only

Interpreted Starlark scripts for all plugins.

**Rejected**: Too slow for hot path operations (auth, guards).

## Related ADRs

- [ADR: Component Architecture](./adr-component-architecture.md)
