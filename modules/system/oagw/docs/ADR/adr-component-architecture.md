# ADR: Component Architecture

- **Status**: Superseded
- **Date**: 2026-02-09
- **Deciders**: OAGW Team
- **Superseded By**: Single-module implementation with internal trait-based service isolation (2026-02-17). The original multi-crate design was simplified during implementation to a single `oagw` crate with DDD-Light layering (`domain/infra/api`). The CP/DP separation is preserved as internal domain traits, not separate crates.

## Context and Problem Statement

OAGW is being designed as a greenfield project without existing code. We need to establish the architectural foundation for how components are organized and how separation of concerns is achieved.

**Key Requirements**:

- Clear separation of concerns between configuration management and request execution
- Testability of each concern in isolation
- Minimize latency through efficient communication patterns

## Decision Drivers

- Testability: Each service should be testable in isolation via trait mocking
- Separation of concerns: Configuration management vs request execution
- Performance: Direct in-process function calls with zero serialization overhead
- Maintainability: Clear boundaries and responsibilities via domain trait interfaces
- Simplicity: Avoid multi-crate coordination overhead when not needed

## Decision

OAGW is implemented as a single module (`oagw` crate) with internal service isolation via domain traits and DDD-Light layering:

### Internal Services

**1. Control Plane (`ControlPlaneService` trait)**

- **Location**: Trait in `domain/services/mod.rs`, impl in `domain/services/management.rs`
- **Responsibility**: Manage configuration data
- **Functions**:
    - CRUD operations for upstreams and routes
    - Alias resolution for proxy requests
    - Tenant-scoped repository access
- **Dependencies**: `UpstreamRepository`, `RouteRepository` (domain traits)

**2. Data Plane (`DataPlaneService` trait)**

- **Location**: Trait in `domain/services/mod.rs`, impl in `infra/proxy/service.rs`
- **Responsibility**: Orchestrate proxy requests to external services
- **Functions**:
    - Call Control Plane for config resolution (upstream, route)
    - Execute auth plugins (credential injection via `AuthPluginRegistry`)
    - Build and send HTTP requests to upstream services
    - Strip sensitive/hop-by-hop headers from responses
- **Dependencies**: `ControlPlaneService`, `AuthPluginRegistry`, `CredentialRepository`, `reqwest::Client`

### Module Structure

```text
modules/system/oagw/
├── oagw-sdk/              # Public API: ServiceGatewayClientV1 trait, models, errors
└── oagw/                  # Single module crate
    └── src/
        ├── api/rest/      # Transport layer (handlers, routes, DTOs)
        ├── domain/        # Business logic (traits, models, errors)
        │   └── services/  # ControlPlaneService + DataPlaneService
        └── infra/         # Infrastructure (proxy, storage, plugins)
```

### Internal Communication

All services communicate via in-process trait method calls. There is no inter-service RPC or serialization:

- REST handlers call `ControlPlaneService` or `DataPlaneService` directly
- `DataPlaneServiceImpl` holds an `Arc<dyn ControlPlaneService>` for config resolution
- Services are wired together during ModKit module initialization in `module.rs`

## Consequences

### Positive

- **Clear separation of concerns**: CP and DP have well-defined responsibilities via trait boundaries
- **Testability**: Services are tested in isolation via trait mocking (e.g., `MockControlPlaneService`)
- **Performance**: Zero overhead — direct Rust function calls, no serialization or RPC
- **Simplicity**: Single crate eliminates multi-crate coordination, versioning, and build complexity
- **Maintainability**: DDD-Light layering (`domain/infra/api`) enforced by dylint linters

### Negative

- **No independent scaling**: CP and DP cannot be scaled separately (acceptable for current workload)
- **Single deployment unit**: All concerns must be deployed together

### Risks

- **Future scaling needs**: If CP and DP need independent scaling, extraction into separate crates would require refactoring. Mitigated by clean trait boundaries — the domain traits already define the split points.

## Alternatives Considered

### Alternative 1: Multi-Crate Architecture (Original Design)

Three separate library crates (`oagw`, `oagw-cp`, `oagw-dp`) with modkit wiring for both single-executable and microservice deployment.

**Pros**:

- Independent scaling of CP and DP
- Microservice deployment option

**Cons**:

- Multi-crate coordination overhead (dependency management, versioning)
- Microservice mode not needed for current scale
- Additional complexity without clear benefit

**Not adopted**: The single-module approach provides the same testability and separation of concerns with less complexity. The trait boundaries are preserved, making future extraction straightforward if needed.

### Alternative 2: Monolithic Service (No Internal Separation)

Single crate with no CP/DP trait distinction — all logic in handlers.

**Pros**:

- Simplest possible structure

**Cons**:

- Hard to test proxy orchestration independently
- No clear boundary between config management and request execution
- Mixing concerns makes maintenance harder

**Rejected**: Trait-based separation provides essential testability and maintainability.

## Related ADRs

- [ADR: Request Routing](./adr-request-routing.md) - How requests flow through handlers to services
- [ADR: Control Plane Caching](./adr-data-plane-caching.md) - Multi-layer cache strategy
- [ADR: State Management](./adr-state-management.md) - State distribution patterns

## References

- Modkit framework documentation (module lifecycle and dependency injection)
- CyberFabric module patterns: `tenant_resolver`, `types_registry`
