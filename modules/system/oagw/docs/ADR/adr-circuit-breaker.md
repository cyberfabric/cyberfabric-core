# ADR: Circuit Breaker

- **Status**: Proposed
- **Date**: 2026-02-03
- **Deciders**: TBD

## Context and Problem Statement

OAGW needs a circuit breaker mechanism to prevent cascading failures when upstream services become unhealthy. When an upstream experiences persistent failures (timeouts, 5xx
errors, connection issues), continuing to send requests:

1. Wastes resources (connections, threads, time)
2. Increases latency for end users
3. Can worsen upstream condition (prevents recovery)
4. Lacks fast-fail behavior for better UX

Circuit breaker should be **core functionality** (not a plugin) because:

- Requires distributed state coordination across OAGW nodes
- Needs atomic transitions and coordination with rate limiting
- Should work consistently for all upstreams without configuration overhead
- Deeply integrated with error handling and routing logic

## Decision Drivers

- **Fast failure detection**: Quickly detect unhealthy upstreams (seconds, not minutes)
- **Automatic recovery**: Self-healing without manual intervention
- **Minimal false positives**: Don't trip on transient errors
- **Distributed coordination**: State shared across OAGW nodes
- **Per-upstream isolation**: One upstream's failure doesn't affect others
- **Observability**: Clear metrics and state visibility
- **Graceful degradation**: Fallback strategies when circuit opens

## Circuit Breaker State Machine

```
     CLOSED ──(failure_threshold reached)──► OPEN
        ▲                                      │
        │                              (timeout_seconds)
        │                                      ▼
        └──(success_threshold reached)── HALF-OPEN
                                               │
                              (failure)────────┘
```

**States**:

- **CLOSED**: Normal operation, all requests pass through. Failure counter increments on errors.
- **OPEN**: All requests rejected immediately with `503 CircuitBreakerOpen`. No upstream calls made.
- **HALF-OPEN**: Limited probe requests allowed to test if upstream recovered. Success → CLOSED, Failure → OPEN.

## Configuration Schema

Circuit breaker configuration is a **first-class field** in upstream and route definitions, not a plugin.

### Upstream Schema Addition

```json
{
  "circuit_breaker": {
    "type": "object",
    "properties": {
      "enabled": {
        "type": "boolean",
        "default": true,
        "description": "Enable/disable circuit breaker for this upstream"
      },
      "failure_threshold": {
        "type": "integer",
        "minimum": 1,
        "default": 5,
        "description": "Consecutive failures before opening circuit"
      },
      "success_threshold": {
        "type": "integer",
        "minimum": 1,
        "default": 3,
        "description": "Consecutive successes in half-open before closing circuit"
      },
      "timeout_seconds": {
        "type": "integer",
        "minimum": 1,
        "default": 30,
        "description": "Seconds circuit stays open before entering half-open"
      },
      "half_open_max_requests": {
        "type": "integer",
        "minimum": 1,
        "default": 3,
        "description": "Max concurrent requests allowed in half-open state"
      },
      "failure_conditions": {
        "type": "object",
        "properties": {
          "status_codes": {
            "type": "array",
            "items": { "type": "integer" },
            "default": [ 500, 502, 503, 504 ],
            "description": "HTTP status codes counted as failures"
          },
          "timeout": {
            "type": "boolean",
            "default": true,
            "description": "Count request timeouts as failures"
          },
          "connection_error": {
            "type": "boolean",
            "default": true,
            "description": "Count connection errors as failures"
          }
        }
      },
      "scope": {
        "type": "string",
        "enum": [ "global", "per_endpoint" ],
        "default": "global",
        "description": "Circuit breaker scope: global for entire upstream or per individual endpoint"
      },
      "fallback_strategy": {
        "type": "string",
        "enum": [ "fail_fast", "fallback_endpoint", "cached_response" ],
        "default": "fail_fast",
        "description": "Behavior when circuit is open"
      },
      "fallback_endpoint_id": {
        "type": "string",
        "format": "uuid",
        "description": "Fallback upstream ID when strategy is fallback_endpoint"
      }
    }
  }
}
```

### Configuration Example

```json
{
  "server": {
    "endpoints": [
      { "scheme": "https", "host": "api.openai.com", "port": 443 }
    ]
  },
  "protocol": "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
  "circuit_breaker": {
    "enabled": true,
    "failure_threshold": 5,
    "success_threshold": 3,
    "timeout_seconds": 30,
    "half_open_max_requests": 3,
    "failure_conditions": {
      "status_codes": [ 500, 502, 503, 504 ],
      "timeout": true,
      "connection_error": true
    },
    "scope": "global",
    "fallback_strategy": "fail_fast"
  }
}
```

## Fallback Strategies

When circuit is **OPEN**, OAGW can respond in different ways:

### 1. Fail Fast (Default)

```json
{
  "fallback_strategy": "fail_fast"
}
```

- **Behavior**: Immediately return `503 CircuitBreakerOpen` without calling upstream
- **Use case**: Default behavior, client can handle error and retry with backoff
- **Latency**: <1ms (no network call)

### 2. Fallback Endpoint

```json
{
  "fallback_strategy": "fallback_endpoint",
  "fallback_endpoint_id": "uuid-of-backup-upstream"
}
```

- **Behavior**: Route request to alternative upstream when primary circuit is open
- **Use case**:
    - Multi-region deployments (primary: us-east, fallback: us-west)
    - Backup service providers (primary: OpenAI, fallback: Azure OpenAI)
- **Requirements**: Fallback upstream must be API-compatible
- **Latency**: Normal request latency + routing overhead

### 3. Cached Response

```json
{
  "fallback_strategy": "cached_response"
}
```

- **Behavior**: Return last successful response from cache (if available)
- **Use case**: Read-only APIs where stale data is acceptable (config, metadata)
- **Requirements**: Response caching must be enabled on route
- **Latency**: <10ms (cache lookup)
- **Limitations**: Only for idempotent GET requests

## Distributed State Management

Circuit breaker state must be shared across OAGW instances. Two approaches:

### Option A: Redis-based Shared State (Recommended)

**State keys**:

```
oagw:cb:{tenant_id}:{upstream_id}:state        → "CLOSED" | "OPEN" | "HALF_OPEN"
oagw:cb:{tenant_id}:{upstream_id}:failures     → counter (TTL: rolling window)
oagw:cb:{tenant_id}:{upstream_id}:opened_at    → timestamp
oagw:cb:{tenant_id}:{upstream_id}:half_open_count → counter for concurrent half-open requests
```

**Operations**:

```lua
-- Check circuit state (fast path)
local state = redis.call('GET', state_key)
if state == 'OPEN' then
    local opened_at = redis.call('GET', opened_at_key)
    if (now - opened_at) > timeout_seconds then
        -- Transition to HALF_OPEN
        redis.call('SET', state_key, 'HALF_OPEN')
        redis.call('SET', half_open_count_key, 0)
        return 'HALF_OPEN'
    else
        return 'OPEN'
    end
end
return state or 'CLOSED'
```

**Pros**:

- Strong consistency across nodes
- Atomic operations via Lua scripts
- Fast (<1ms latency)
- Supports distributed counters

**Cons**:

- Dependency on Redis
- Single point of failure (mitigated by Redis HA)

### Option B: Eventually Consistent In-Memory State

Each OAGW node maintains local circuit state. Nodes gossip state changes via pub/sub or multicast.

**Pros**: No external dependency
**Cons**:

- State divergence possible
- Delayed failure detection
- Complex coordination logic

**Decision**: Use Redis-based shared state (Option A) for strong consistency.

## Integration with Error Handling

Circuit breaker evaluates responses and updates state:

```rust
fn handle_upstream_response(response: UpstreamResponse, circuit: &CircuitBreaker) -> Result<Response> {
    let is_failure = match response {
        Ok(resp) if circuit.config.failure_conditions.status_codes.contains(&resp.status) => true,
        Ok(_) => false,
        Err(UpstreamError::Timeout) if circuit.config.failure_conditions.timeout => true,
        Err(UpstreamError::Connection) if circuit.config.failure_conditions.connection_error => true,
        Err(_) => false,
    };

    if is_failure {
        circuit.record_failure().await?;
    } else {
        circuit.record_success().await?;
    }

    response
}
```

## Observability and Metrics

Circuit breaker exposes metrics:

```
oagw_circuit_breaker_state{upstream_id, tenant_id} → 0=CLOSED, 1=HALF_OPEN, 2=OPEN
oagw_circuit_breaker_failures_total{upstream_id, tenant_id}
oagw_circuit_breaker_state_changes_total{upstream_id, tenant_id, from_state, to_state}
oagw_circuit_breaker_rejected_requests_total{upstream_id, tenant_id}
oagw_circuit_breaker_half_open_successes_total{upstream_id, tenant_id}
oagw_circuit_breaker_half_open_failures_total{upstream_id, tenant_id}
```

## Error Response

When circuit is open:

```json
{
  "error": {
    "type": "gts.x.core.errors.err.v1~x.oagw.circuit_breaker.open.v1",
    "status": 503,
    "code": "CIRCUIT_BREAKER_OPEN",
    "message": "Circuit breaker is open for upstream api.openai.com",
    "details": {
      "upstream_id": "uuid",
      "state": "OPEN",
      "opened_at": "2026-02-03T10:45:00Z",
      "retry_after_seconds": 15
    }
  }
}
```

Headers:

```
Retry-After: 15
X-Circuit-State: OPEN
```

## Hierarchical Configuration

Circuit breaker configuration follows same inheritance rules as rate limits:

```json
{
  "circuit_breaker": {
    "sharing": "inherit", // or "enforce" or "private"
    "enabled": true,
    "failure_threshold": 10,
    "timeout_seconds": 60
  }
}
```

- **private**: Descendant tenants define their own circuit breaker config
- **inherit**: Descendant can override if needed
- **enforce**: Descendant must use ancestor's config (cannot disable or weaken thresholds)

## Implementation Notes

1. **Atomic state transitions**: Use Redis WATCH/MULTI/EXEC or Lua scripts for atomic state changes
2. **Graceful degradation**: If Redis unavailable, default to CLOSED state (fail open)
3. **Per-endpoint granularity**: When `scope: per_endpoint`, maintain separate circuit state for each endpoint in upstream
4. **Manual override**: Admin API to manually open/close circuits for maintenance
5. **Warm-up period**: After deployment, circuit starts in CLOSED with reduced sensitivity

## Consequences

### Positive

- Fast failure detection and automatic recovery
- Prevents cascading failures
- Reduces wasted resources on unhealthy upstreams
- Better user experience (fast 503 vs long timeout)
- Core functionality with consistent behavior

### Negative

- No Adds complexity to request handling path
- No Dependency on Redis for distributed state
- No False positives possible during upstream maintenance
- No Additional monitoring/alerting needed

### Neutral

- Circuit breaker state is shared globally per upstream (not per-route)
- Manual intervention needed to override automatic behavior
- Requires careful tuning of thresholds per upstream

## Alternatives Considered

### Alternative 1: Circuit Breaker as Plugin

**Rejected**: Plugins cannot access distributed state or coordinate across nodes efficiently. Circuit breaker needs tight integration with core routing logic.

### Alternative 2: No Circuit Breaker (Client Responsibility)

**Rejected**: Clients may not implement proper circuit breaking. OAGW is better positioned to detect upstream health across all tenants.

### Alternative 3: Health Check Based

Instead of passive circuit breaker (reactive), use active health checks (proactive).

**Decision**: Combine both - circuit breaker for fast failure detection + optional active health checks for faster recovery detection.

## Related ADRs

- [ADR: Rate Limiting](./adr-rate-limiting.md) - Circuit breaker and rate limiting share distributed state infrastructure
- [ADR: Error Source Distinction](./adr-error-source-distinction.md) - Circuit breaker errors must be distinguishable from upstream errors

## References

- [Netflix Hystrix](https://github.com/Netflix/Hystrix/wiki/How-it-Works)
- [Martin Fowler: Circuit Breaker](https://martinfowler.com/bliki/CircuitBreaker.html)
- [AWS: Circuit Breaker Pattern](https://aws.amazon.com/builders-library/using-circuit-breakers-to-protect-services/)
- [Envoy Circuit Breaking](https://www.envoyproxy.io/docs/envoy/latest/intro/arch_overview/upstream/circuit_breaking)
