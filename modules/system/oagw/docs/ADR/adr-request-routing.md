# ADR: Request Routing

- **Status**: Accepted
- **Date**: 2026-02-09
- **Deciders**: OAGW Team

## Context and Problem Statement

With three components (API Handler, Data Plane, Control Plane), we need to define how requests are routed between them. Different request types have different requirements:

- Management operations (CRUD for upstreams/routes/plugins) modify configuration
- Proxy operations execute calls to external services

**Key Question**: Which component handles which operations?

## Decision

**Path-Based Routing**: API Handler routes requests based on URL path.

### Routing Rules

| Path Pattern               | Routed To     | Purpose        |
|----------------------------|---------------|----------------|
| `/api/oagw/v1/upstreams/*` | Control Plane    | Upstream CRUD  |
| `/api/oagw/v1/routes/*`    | Control Plane    | Route CRUD     |
| `/api/oagw/v1/plugins/*`   | Control Plane    | Plugin CRUD    |
| `/api/oagw/v1/proxy/*`     | Data Plane | Proxy requests |

### Request Flows

**Management Operations** (e.g., `POST /upstreams`):

```
Client
→ API Handler (auth, rate limit)
→ Control Plane (validate, write DB, invalidate cache)
→ Response
```

**Proxy Operations** (e.g., `GET /proxy/openai/v1/chat/completions`):

```
Client
→ API Handler (auth, rate limit)
→ Data Plane (orchestrate)
  → Control Plane (resolve upstream config)
  → Control Plane (resolve route config)
  → CP: Execute plugins (auth, guard, transform)
  → CP: HTTP call to external service
→ Response
```

## Rationale

**Why DP handles management operations**:

- DP owns configuration data and database access
- Direct path: no need to go through CP
- DP can immediately invalidate caches after writes
- Shorter path reduces latency for config operations

**Why CP handles proxy operations**:

- CP orchestrates request execution
- CP calls DP for config resolution (separation of concerns)
- CP executes plugins (auth, guard, transform)
- CP makes HTTP calls to external services

## Consequences

### Positive

- Clear separation: DP = data management, CP = request execution
- Shorter path for management operations (API → DP direct)
- CP remains focused on proxy logic
- DP can optimize cache invalidation during writes

### Negative

- CP depends on DP for every proxy request (cache misses)
- Mitigated by CP L1 cache for hot configs

## Alternatives Considered

### Alternative: CP Handles Everything

All requests go to CP, which calls DP as needed.

**Rejected**: Management operations don't need CP's orchestration logic. Direct path to DP is simpler and faster.

## Related ADRs

- [ADR: Component Architecture](./adr-component-architecture.md)
- [ADR: Control Plane Caching](./adr-data-plane-caching.md)
- [ADR: State Management](./adr-state-management.md)
