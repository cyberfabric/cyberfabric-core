# ADR: Control Plane Caching Strategy

- **Status**: Accepted
- **Date**: 2026-02-09
- **Deciders**: OAGW Team

## Context

Control Plane handles config resolution for Data Plane during proxy requests. Configuration data (upstreams, routes, plugins) is read-heavy and changes infrequently. We need a
caching strategy that:

- Minimizes database load
- Provides fast lookups (<1ms for hot configs)
- Supports both single-exec and microservice modes
- Handles cache invalidation on config writes

## Decision

**Multi-layer caching**: L1 (in-memory) + optional L2 (Redis) + Database

### Cache Layers

**L1 Cache (In-Memory)**:

- Per-instance LRU cache
- Max 10,000 entries
- No TTL (evicted by LRU)
- Access time: <1μs

**L2 Cache (Redis, Optional)**:

- Shared across DP instances (microservice mode)
- TTL: 5 minutes
- Serialization: MessagePack
- Access time: ~1-2ms

**Database (Source of Truth)**:

- PostgreSQL with JSONB support
- Queried only on L1+L2 miss
- Access time: ~5-10ms

### Lookup Flow

```rust
async fn get_config(key: &CacheKey) -> Result<ConfigValue> {
    // Check L1
    if let Some(value) = l1_cache.get(key) {
        return Ok(value);  // <1μs
    }

    // Check L2 (if enabled)
    if let Some(redis) = l2_cache {
        if let Some(value) = redis.get(key).await? {
            l1_cache.insert(key, value.clone());
            return Ok(value);  // ~1-2ms
        }
    }

    // Query DB
    let value = db.query(key).await?;  // ~5-10ms

    // Populate caches
    l1_cache.insert(key, value.clone());
    if let Some(redis) = l2_cache {
        redis.set(key, &value, TTL_5MIN).await?;
    }

    Ok(value)
}
```

### Cache Keys

- `upstream:{tenant_id}:{alias}` → UpstreamConfig
- `route:{upstream_id}:{method}:{path}` → RouteConfig
- `plugin:{plugin_id}` → Plugin definition

### Cache Invalidation

On config write (e.g., `PUT /upstreams/{id}`):

1. DP writes to database
2. DP flushes L1 cache for affected keys
3. DP flushes L2 cache (if enabled) for affected keys
4. DP returns success
5. CP flushes its own L1 cache (notified by DP or periodic sync)

### Deployment Modes

**Single-Exec Mode**:

- L1 only (no Redis needed)
- Single instance, no cache sharing required

**Microservice Mode**:

- L1 + L2 (Redis)
- L2 shared across DP instances
- Reduces DB load from multiple instances

## Rationale

- **L1 for speed**: In-memory access is fastest (~1μs)
- **L2 for sharing**: Redis shares cache across instances in microservice mode
- **Optional L2**: Single-exec mode doesn't need Redis (simpler deployment)
- **Lazy population**: Caches populated on read (no proactive warming)

## Consequences

### Positive

- Fast lookups for hot configs (<1μs L1, ~1-2ms L2)
- Reduced database load (queries only on cache miss)
- Shared cache in microservice mode (L2)
- Simple deployment in single-exec mode (no Redis)

### Negative

- Cache invalidation complexity (must flush L1 and L2)
- Redis dependency in microservice mode
- Potential stale data during cache TTL window

### Risks

**Risk**: Redis unavailability causes L2 miss, increased DB load.
**Mitigation**: L1 cache still active (10k entries), DB connection pool limits concurrent queries.

## Alternatives Considered

### Alternative 1: L1 Only

**Rejected**: In microservice mode, each DP instance hits DB independently (high load).

### Alternative 2: L2 Only (Redis)

**Rejected**: Slower than L1 (serialization overhead), unnecessary for single-exec mode.

### Alternative 3: Write-Through Cache

**Rejected**: Complicates writes, doesn't help read-heavy workload.

## Related ADRs

- [ADR: Component Architecture](./adr-component-architecture.md)
- [ADR: State Management](./adr-state-management.md)
