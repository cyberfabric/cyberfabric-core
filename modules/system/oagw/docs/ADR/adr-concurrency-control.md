# ADR: Concurrency Control

- **Status**: Proposed
- **Date**: 2026-02-03
- **Deciders**: OAGW Team

## Context and Problem Statement

OAGW needs concurrency control to limit the number of simultaneous in-flight requests to protect:

1. **Upstream services** from being overwhelmed by too many concurrent connections
2. **OAGW itself** from resource exhaustion (memory, file descriptors, threads)
3. **Tenant isolation** ensuring one tenant cannot monopolize upstream capacity

Concurrency control differs from rate limiting:

- **Rate limiting**: Controls requests per time window (e.g., 1000 req/min)
- **Concurrency limiting**: Controls simultaneous active requests (e.g., max 50 concurrent)

Both are needed: rate limiting prevents long-term quota violations, concurrency limiting prevents burst overload.

## Decision Drivers

- Prevent resource exhaustion from slow clients/upstreams
- Fair capacity sharing across tenants
- Low overhead tracking (<100ns per request)
- Graceful degradation when limits reached
- Observable metrics (current in-flight, limit utilization)
- Works with streaming requests (counted until completion)

## Concurrency Limiting Scope

**Three levels of concurrency limits:**

1. **Upstream-level**: Total concurrent requests to an upstream (all routes combined)
2. **Route-level**: Concurrent requests to a specific route
3. **Tenant-level**: Concurrent requests from a tenant (across all upstreams)

**Hierarchy**: All three limits are independent checks. A request must pass all applicable limits.

```
Request → [Tenant Limit] → [Upstream Limit] → [Route Limit] → Execute
           └─ 503 if exceeded ┘                                 
```

## Configuration Schema

### Upstream Concurrency Config

Add to `Upstream` type:

```json
{
  "concurrency_limit": {
    "sharing": "private",
    "max_concurrent": 100,
    "per_tenant_max": 20,
    "strategy": "reject"
  }
}
```

**Fields**:

- `sharing`: `"private"` | `"inherit"` | `"enforce"` (same semantics as rate_limit)
- `max_concurrent`: Total concurrent requests across all tenants (global limit)
- `per_tenant_max`: Max concurrent per individual tenant (fairness limit)
- `strategy`: `"reject"` | `"queue"` (queue behavior defined in ADR: Backpressure)

### Route Concurrency Config

Add to `Route` type:

```json
{
  "concurrency_limit": {
    "max_concurrent": 50
  }
}
```

**Fields**:

- `max_concurrent`: Max concurrent requests to this route

**Note**: Routes do not have `per_tenant_max` - use upstream-level for tenant isolation.

### Tenant-Global Concurrency

Configured at tenant level (not per-upstream):

```json
{
  "tenant_id": "uuid-123",
  "global_concurrency_limit": 200
}
```

**Scope**: Sum of all in-flight requests from this tenant across all upstreams.

## Merge Strategy (Hierarchical Configuration)

When descendant tenant binds to ancestor's upstream:

| Ancestor Sharing | Descendant Specifies | Effective Limit                        |
|------------------|----------------------|----------------------------------------|
| `private`        | —                    | Descendant must provide limit          |
| `inherit`        | No                   | Use ancestor's limit                   |
| `inherit`        | Yes                  | `min(ancestor, descendant)` (stricter) |
| `enforce`        | Any                  | `min(ancestor, descendant)` (stricter) |

**Rationale**: Always enforce the stricter limit (same as rate limiting) to prevent descendants from bypassing parent's capacity constraints.

## Implementation Strategy

### Semaphore-Based Limiting

Use in-memory semaphores for fast local checks:

```
struct ConcurrencyLimiter {
    max_concurrent: usize,
    in_flight: AtomicUsize,
}

impl ConcurrencyLimiter {
    fn try_acquire() -> Result<Permit, ConcurrencyLimitExceeded> {
        let current = self.in_flight.fetch_add(1, Ordering::Relaxed);
        if current >= self.max_concurrent {
            self.in_flight.fetch_sub(1, Ordering::Relaxed);
            return Err(ConcurrencyLimitExceeded);
        }
        Ok(Permit { limiter: self })
    }
}

struct Permit<'a> {
    limiter: &'a ConcurrencyLimiter,
}

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        self.limiter.in_flight.fetch_sub(1, Ordering::Relaxed);
    }
}
```

**Permit Pattern**: RAII guard ensures in-flight counter decrements even on error/panic.

### Request Lifecycle

```
1. Acquire tenant-global permit
2. Acquire upstream permit
3. Acquire route permit
4. Execute request
5. Permits auto-released on completion/error/timeout
```

**Streaming Requests**: Permit held until stream completes or client disconnects.

### Distributed Coordination

**Local-Only Limiting** (Phase 1):

- Each OAGW node tracks in-flight independently
- Effective limit = `max_concurrent / node_count` (configured)
- Simple, low latency, no distributed state

**Distributed Limiting** (Phase 2):

- Use Redis or shared counter for accurate global limiting
- Increased latency (~1-5ms for distributed check)
- Required for strict enforcement

**Recommendation**: Start with local-only. Add distributed coordination only if needed.

## Error Handling

### New Error Type

```json
{
  "type": "gts.x.core.errors.err.v1~x.oagw.concurrency_limit.exceeded.v1",
  "title": "Concurrency Limit Exceeded",
  "status": 503,
  "detail": "Upstream api.openai.com has reached max concurrent requests (100/100)",
  "instance": "/api/oagw/v1/proxy/api.openai.com/v1/chat",
  "upstream_id": "uuid-123",
  "host": "api.openai.com",
  "limit_type": "upstream",
  "current_in_flight": 100,
  "max_concurrent": 100,
  "retry_after_seconds": 1,
  "trace_id": "01J..."
}
```

**HTTP Headers**:

```http
HTTP/1.1 503 Service Unavailable
X-OAGW-Error-Source: gateway
Retry-After: 1
Content-Type: application/problem+json
```

**Retriable**: Yes (client should retry with backoff)

## Metrics

### Core Metrics

```promql
# Current in-flight requests (gauge)
oagw_requests_in_flight{host, level} gauge
# level: "upstream", "route", "tenant"

# Concurrency limit rejections (counter)
oagw_concurrency_limit_exceeded_total{host, level} counter

# Concurrency utilization (0.0 to 1.0)
oagw_concurrency_usage_ratio{host, level} gauge
# = in_flight / max_concurrent

# Concurrency limit configuration (gauge)
oagw_concurrency_limit_max{host, level} gauge
```

### Per-Tenant Tracking (Optional)

```promql
# Per-tenant in-flight (only if monitoring enabled)
oagw_tenant_requests_in_flight{tenant_id} gauge

# Note: High cardinality - enable only for monitoring/debugging
```

## Connection Pool Sizing

Concurrency limits should align with HTTP client connection pool size:

```json
{
  "upstream": {
    "concurrency_limit": {
      "max_concurrent": 100
    },
    "http_client": {
      "connection_pool": {
        "max_connections": 100,
        "max_idle_connections": 20,
        "idle_timeout": "90s"
      }
    }
  }
}
```

**Guidelines**:

- `max_connections` ≥ `max_concurrent` (to avoid blocking on pool exhaustion)
- `max_idle_connections` = 20-30% of `max_concurrent` (balance latency vs resources)
- Monitor `oagw_upstream_connections{state="waiting"}` for pool contention

## Interaction with Other Systems

### Rate Limiting

**Independent checks**: Both rate limit and concurrency limit must pass:

```
Request → [Rate Limiter] → [Concurrency Limiter] → Execute
           └─ 429 if exceeded    └─ 503 if exceeded
```

**Order**: Check rate limit first (cheaper, rejects quota violations early).

### Circuit Breaker

When circuit breaker is **OPEN**:

- Requests rejected immediately (no concurrency permit acquired)
- In-flight counter not affected

When circuit breaker is **HALF-OPEN**:

- Limited probe requests still count against concurrency limit
- Ensures probes don't overwhelm recovering upstream

### Backpressure/Queueing

When `strategy: "queue"` is set:

- Failed `try_acquire()` adds request to queue (see ADR: Backpressure)
- When permit released, queue consumer acquires it

## Database Schema Updates

### Upstream Table

```sql
ALTER TABLE oagw_upstream
    ADD COLUMN concurrency_limit JSONB;

-- Example value:
-- {
--   "sharing": "enforce",
--   "max_concurrent": 100,
--   "per_tenant_max": 20,
--   "strategy": "reject"
-- }
```

### Route Table

```sql
ALTER TABLE oagw_route
    ADD COLUMN concurrency_limit JSONB;

-- Example value:
-- {
--   "max_concurrent": 50
-- }
```

### Tenant Table (Global Limit)

```sql
ALTER TABLE tenant
    ADD COLUMN oagw_global_concurrency_limit INTEGER;

-- Default: NULL (no limit)
```

## Configuration Validation

**Rules**:

1. `max_concurrent` must be > 0
2. `per_tenant_max` must be ≤ `max_concurrent`
3. Route limit must be ≤ upstream limit (if both specified)
4. Tenant global limit should be > sum of per-tenant-max across upstreams (warning, not error)

## Defaults

If not specified:

- **Upstream**: No concurrency limit (unlimited)
- **Route**: Inherits upstream limit
- **Tenant**: No global limit

**Recommendation**: Set conservative defaults at system level, allow overrides per upstream.

## Testing Strategy

**Unit Tests**:

- Semaphore acquire/release correctness
- RAII permit drop behavior
- Concurrent access from multiple threads

**Integration Tests**:

- Reject requests when limit reached
- Release permit on timeout/error/completion
- Streaming request lifecycle
- Hierarchical limit enforcement

**Load Tests**:

- Sustain max_concurrent requests without leaks
- Verify metrics accuracy
- Connection pool alignment

## Implementation Phases

**Phase 1: Local Limiting**

- In-memory semaphores
- Upstream and route-level limits
- Error handling and metrics
- Configuration schema

**Phase 2: Tenant Isolation**

- Per-tenant-max enforcement
- Tenant global limit
- Fairness across tenants

**Phase 3: Distributed Coordination** (Optional)

- Redis-based global counters
- Cross-node synchronization

## Decision

**Accepted**: Implement concurrency control with local-only limiting (Phase 1-2).

**Rationale**:

- Protects OAGW and upstreams from overload
- Low overhead (atomic operations only)
- Simple to implement and reason about
- Provides clear error signals to clients
- Complements rate limiting for comprehensive traffic management

**Deferred**: Distributed coordination (Phase 3) until demonstrated need.

## Consequences

**Positive**:

- Prevents resource exhaustion
- Improves stability under load
- Fair capacity sharing
- Clear observability

**Negative**:

- No Additional configuration complexity
- No Potential false rejections during traffic spikes (mitigated by backpressure/queueing)
- No Local-only limiting means limit is approximation across nodes

**Mitigations**:

- Provide sensible defaults (no limit unless specified)
- Use `strategy: "queue"` for smoother degradation (see ADR: Backpressure)
- Monitor metrics to tune limits appropriately

## References

- [ADR: Rate Limiting](./adr-rate-limiting.md) - Time-based rate control
- [ADR: Backpressure and Queueing](./adr-backpressure-queueing.md) - Queue behavior when limits reached
- [ADR: Circuit Breaker](./adr-circuit-breaker.md) - Upstream health protection
- [Envoy Circuit Breaking](https://www.envoyproxy.io/docs/envoy/latest/intro/arch_overview/upstream/circuit_breaking)
- [AWS API Gateway Quotas](https://docs.aws.amazon.com/apigateway/latest/developerguide/limits.html)
