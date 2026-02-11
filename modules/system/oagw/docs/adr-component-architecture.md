# ADR: Component Architecture

- **Status**: Accepted
- **Date**: 2026-02-09
- **Deciders**: OAGW Team

## Context and Problem Statement

OAGW is being designed as a greenfield project without existing code. We need to establish the architectural foundation for how components are organized, how they communicate, and
how deployment flexibility is achieved.

**Key Requirements**:

- Support both single-executable and microservice deployment modes
- Clear separation of concerns between configuration management and request execution
- Enable independent scaling of different concerns
- Minimize latency through efficient communication patterns

## Decision Drivers

- Testability: Each component should be testable in isolation
- Deployment flexibility: Support multiple deployment topologies without code changes
- Separation of concerns: Configuration management vs request execution
- Performance: Minimize overhead in communication between components
- Maintainability: Clear boundaries and responsibilities

## Decision

OAGW is composed of three distinct components, packaged as separate library crates:

### Components

**1. Main Module (`oagw`)**

- **Responsibility**: Entry point for all HTTP requests, composition root
- **Functions**:
    - Incoming authentication (validate Bearer tokens)
    - Incoming rate limiting (protect gateway from overload)
    - Path-based routing to CP or DP
    - Wires CP + DP services via modkit
- **Routes**:
    - `/api/oagw/v1/proxy/*` → Data Plane
    - `/api/oagw/v1/upstreams/*`, `/routes/*`, `/plugins/*` → Control Plane

**2. Control Plane (`oagw-cp`)**

- **Responsibility**: Manage configuration data with multi-layer caching
- **Functions**:
    - CRUD operations for upstreams/routes/plugins
    - Config resolution with hierarchical tenant inheritance
    - L1 cache (in-memory, 10k entries, LRU)
    - Optional L2 cache (Redis, shared across instances)
    - Database access (source of truth)
    - Cache invalidation on config writes
- **Dependencies**: modkit-db (database), types_registry (schema validation)

**3. Data Plane (`oagw-dp`)**

- **Responsibility**: Orchestrate proxy requests to external services
- **Functions**:
    - Call Control Plane for config resolution (upstream, route)
    - Execute auth plugins (credential injection)
    - Execute guard plugins (validation, rate limiting)
    - Execute transform plugins (request/response mutation)
    - Make HTTP calls to external services
    - L1 cache for hot configs (1000 entries, LRU)
- **Dependencies**: Control Plane (config resolution), cred_store (secret retrieval)

### Module Structure

```
modules/system/oagw/
├── oagw-sdk/          # Public API traits, models, errors, service interfaces
│                      # OAGWClientV1, ControlPlaneService, DataPlaneService
│                      # AuthPlugin, GuardPlugin, TransformPlugin
├── oagw/              # Main module crate (composition root)
├── oagw-cp/           # Control Plane implementation
├── oagw-dp/           # Data Plane implementation
```

### Communication Patterns

**Single-Executable Mode**:

- All components instantiated in same process
- Trait method calls are direct function calls (zero serialization)
- Example: `cp.proxy_request(req)` → direct Rust function call

**Microservice Mode**:

- Components deployed as separate services
- Modkit provides transparent RPC adapters for trait calls
- Example: `cp.proxy_request(req)` → gRPC or HTTP call
- Service discovery and load balancing handled by modkit

### Deployment Abstraction

OAGW code is deployment-agnostic:

- Uses trait interfaces: `ControlPlaneService`, `DataPlaneService`
- Modkit wires components based on deployment configuration
- No hardcoded assumptions about communication transport

## Consequences

### Positive

- **Clear separation of concerns**: Each component has well-defined responsibilities
- **Testability**: Components can be unit tested in isolation with mock implementations
- **Deployment flexibility**: Single-exec for development, microservices for production
- **Independent scaling**: Scale CP for proxy load, scale DP for config operations
- **Performance**: Zero overhead in single-exec mode, transparent RPC in microservice mode
- **Maintainability**: Clear boundaries reduce coupling

### Negative

- **Complexity**: Three components instead of monolith increases coordination complexity
- **Testing overhead**: Need integration tests for component communication
- **Initial development**: More upfront design work for interfaces

### Risks

- **Network latency** (microservice mode): CP → DP calls add latency. Mitigated by CP L1 cache.
- **Service dependencies**: CP depends on DP availability. Mitigated by fail-open with cached config.
- **Deployment coordination**: Multiple services require orchestration. Handled by modkit.

## Alternatives Considered

### Alternative 1: Monolithic Service

Single service handling both configuration and proxy operations.

**Pros**:

- Simpler deployment
- No inter-component communication overhead

**Cons**:

- Cannot scale concerns independently
- Configuration writes and proxy requests compete for resources
- Harder to optimize for different workload characteristics
- Mixing concerns makes testing more complex

**Rejected**: Insufficient scaling flexibility for production workloads.

### Alternative 2: Two Components (No API Handler)

CP and DP, with CP handling both proxy and routing.

**Pros**:

- Fewer components
- Slightly simpler

**Cons**:

- CP becomes responsible for routing logic
- Cannot optimize API Handler separately
- Loses clear entry point abstraction

**Rejected**: API Handler provides valuable abstraction and unified ingress control.

## Related ADRs

- [ADR: Request Routing](./adr-request-routing.md) - How requests flow between components
- [ADR: Control Plane Caching](./adr-data-plane-caching.md) - Multi-layer cache strategy
- [ADR: State Management](./adr-state-management.md) - CP L1 cache and state distribution

## References

- Modkit framework documentation (component wiring and deployment)
- CyberFabric module patterns: `tenant_resolver`, `types_registry`
