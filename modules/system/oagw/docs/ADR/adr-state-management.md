# ADR: State Management

- **Status**: Accepted
- **Date**: 2026-02-09
- **Deciders**: OAGW Team

## Context

With Data Plane (CP) calling Control Plane (DP) for config resolution, we need to decide how state is managed:

- Should CP cache frequently-accessed configs?
- Where should rate limiters live?
- How do we balance performance vs consistency?

## Decision

**CP has its own L1 cache** for hot configs + **rate limiters** owned by CP.

### CP State

```rust
pub struct CPState {
    // Small L1 cache for hot configs (1000 entries, LRU)
    hot_cache: Arc<Mutex<LruCache<CacheKey, ConfigValue>>>,

    // Connection to Control Plane
    dp_client: Arc<dyn DataPlaneService>,

    // HTTP client for external services
    http_client: Arc<HttpClient>,

    // Rate limiters (per-upstream, per-route)
    rate_limiters: Arc<RateLimiterRegistry>,
}
```

### DP State

```rust
pub struct DPState {
    // L1: In-memory cache (10k entries, LRU)
    l1_cache: Arc<Mutex<LruCache<CacheKey, ConfigValue>>>,

    // L2: Optional Redis (shared across instances)
    l2_cache: Option<Arc<RedisClient>>,

    // Database connection pool
    db_pool: Arc<DbConnectionPool>,
}
```

### Request Flow with Caching

```
CP receives proxy request
├─ Check CP L1 cache for upstream config
│  ├─ Hit: Use cached config (<1μs)
│  └─ Miss: Call DP.resolve_upstream()
│           ├─ DP checks L1 cache
│           ├─ DP checks L2 cache (if enabled)
│           ├─ DP queries DB
│           └─ CP caches result in L1
│
├─ Check CP L1 cache for route config
│  ├─ Hit: Use cached config
│  └─ Miss: Call DP.resolve_route()
│
├─ Execute auth plugin
├─ Check rate limiter (CP-owned)
├─ Execute guard plugins
├─ Execute transform plugins
├─ HTTP call to external service
└─ Return response
```

### Cache Invalidation

On config write (e.g., `PUT /upstreams/{id}`):

1. DP writes to database
2. DP flushes own L1 and L2 caches
3. DP returns success
4. API Handler notifies CP to flush its L1 cache (or CP periodically syncs)

### CP Cache Size

- Max 1000 entries (small, focused on hot paths)
- LRU eviction
- No TTL (relies on explicit invalidation)
- Configurable via environment variable

### Rate Limiter Location

**CP owns rate limiters** (not DP):

- Per-instance rate limiting for MVP
- CP has full request context (tenant, upstream, route)
- Avoids extra DP call for rate limit check

## Rationale

**Why CP has L1 cache**:

- CP handles every proxy request
- Reduces DP calls for hot configs
- <1μs access time for cached configs
- Small cache (1000 entries) has negligible memory overhead

**Why rate limiters in CP**:

- CP already has request context
- Avoids extra DP call per request
- Per-instance limiting is acceptable for MVP

**Why DP is authoritative cache**:

- DP owns database access
- DP can optimize cache invalidation during writes
- CP L1 is just optimization layer

## Consequences

### Positive

- **Fast path**: CP serves hot configs from L1 (<1μs)
- **Reduced DP calls**: Only for cache misses
- **Simple rate limiting**: No distributed coordination for MVP

### Negative

- **Cache consistency**: CP L1 can temporarily diverge from DP
- **Per-instance rate limiting**: Not globally accurate
- **Trade-offs accepted**: Performance vs strict consistency

### Risks

**Risk**: CP L1 cache becomes stale after config write.
**Mitigation**: Explicit cache invalidation from DP, short-lived cache entries.

**Risk**: Per-instance rate limiting less accurate than distributed.
**Mitigation**: Acceptable for MVP. Future: Add Redis-backed distributed rate limiter as CP extension.

## Alternatives Considered

### Alternative 1: CP Stateless

CP makes DP call for every request (no L1 cache).

**Rejected**: Too many DP calls, adds latency for hot configs.

### Alternative 2: DP Owns Rate Limiters

CP calls DP to check rate limits.

**Rejected**: Extra DP call per request, not worth the overhead for MVP.

## Related ADRs

- [ADR: Component Architecture](./adr-component-architecture.md)
- [ADR: Control Plane Caching](./adr-data-plane-caching.md)
- [ADR: Request Routing](./adr-request-routing.md)
