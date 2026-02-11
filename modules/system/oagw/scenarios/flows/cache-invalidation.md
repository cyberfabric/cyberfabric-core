# Cache Invalidation Flow

## Overview

Cache invalidation ensures consistency across Control Plane and Data Plane caches when configuration changes. Writes trigger immediate invalidation in DP L1/L2, followed by CP L1
notification.

## Cache Architecture

### Control Plane Caches

- **L1 (In-Memory)**: LRU cache, 10k entries, <1μs access
- **L2 (Redis, Optional)**: Shared across DP instances, ~1-2ms access
- **Database**: Source of truth, ~5-10ms access

### Data Plane Cache

- **L1 (In-Memory)**: LRU cache, 1000 entries, <1μs access
- **No L2**: CP depends on DP for config resolution

## Invalidation Trigger: Config Write

When a management operation modifies configuration (upstream, route, plugin), invalidation flows through multiple layers.

## Scenario 1: Update Upstream Config

### Initial State

**CP L1 cache**:

```
upstream:{tenant_id}:openai → {...config...} (cached 5 minutes ago)
```

**DP L1 cache**:

```
upstream:{tenant_id}:openai → {...config...} (cached 10 minutes ago)
```

**DP L2 cache (Redis)**:

```
upstream:{tenant_id}:openai → {...config...} (TTL: 3 minutes remaining)
```

### Management Request

```http
PUT /api/oagw/v1/upstreams/gts.x.core.oagw.upstream.v1~7c9e6679... HTTP/1.1
Host: oagw.example.com
Authorization: Bearer <tenant-token>
Content-Type: application/json

{
  "server": {
    "endpoints": [
      { "scheme": "https", "host": "api.openai.com", "port": 443 }
    ]
  },
  "protocol": "gts.x.core.oagw.protocol.v1~x.core.http.v1",
  "alias": "openai",
  "rate_limit": {
    "sustained": { "rate": 50, "window": "minute" }
  }
}
```

### Invalidation Flow

```
Client → API Handler
  ↓
API Handler → Control Plane
  ↓
┌─────────────────────────────────────┐
│ Control Plane Processing               │
├─────────────────────────────────────┤
│ 1. Validate request                 │
│ 2. Write to database                │
│    UPDATE oagw_upstream             │
│    SET config = ...                 │
│    WHERE id = ...                   │
│                                     │
│ 3. Invalidate DP L1 cache           │
│    DELETE upstream:{tenant}:openai  │
│                                     │
│ 4. Invalidate DP L2 cache (Redis)  │
│    DEL upstream:{tenant}:openai     │
│                                     │
│ 5. Return success                   │
└─────────────────────────────────────┘
  ↓
API Handler receives success
  ↓
API Handler → Data Plane (notification)
  ↓
┌─────────────────────────────────────┐
│ Data Plane Processing            │
├─────────────────────────────────────┤
│ 1. Receive invalidation message     │
│    {                                │
│      "cache_key": "upstream:...",   │
│      "tenant_id": "...",            │
│      "upstream_id": "..."           │
│    }                                │
│                                     │
│ 2. Invalidate CP L1 cache           │
│    DELETE upstream:{tenant}:openai  │
│                                     │
│ 3. Acknowledge                      │
└─────────────────────────────────────┘
  ↓
API Handler → Client (200 OK)
```

### Post-Invalidation State

**CP L1 cache**:

```
(empty - invalidated)
```

**DP L1 cache**:

```
(empty - invalidated)
```

**DP L2 cache (Redis)**:

```
(empty - invalidated)
```

**Database**:

```
upstream config = updated value (source of truth)
```

### Next Proxy Request

```
Client → API Handler → Data Plane
  ↓
CP checks L1 cache: MISS
  ↓
CP calls DP.resolve_upstream("openai")
  ↓
DP checks L1 cache: MISS
  ↓
DP checks L2 cache: MISS
  ↓
DP queries database: HIT (fresh config)
  ↓
DP populates L1 and L2 with fresh config
  ↓
DP returns to CP
  ↓
CP populates L1 with fresh config
  ↓
CP uses fresh config for request
```

**Result**: Next request uses updated rate limit (50/min instead of 100/min).

## Scenario 2: Create New Route

### Management Request

```http
POST /api/oagw/v1/routes HTTP/1.1
Host: oagw.example.com
Authorization: Bearer <tenant-token>
Content-Type: application/json

{
  "upstream_id": "gts.x.core.oagw.upstream.v1~7c9e6679...",
  "match": {
    "http": {
      "methods": ["POST"],
      "path": "/v1/completions"
    }
  }
}
```

### Invalidation Flow

```
Control Plane:
  1. Insert into oagw_route table
  2. No cache to invalidate (new resource)
  3. Return 201 Created

No CP notification needed (route not cached yet)
```

**Result**: Next request matching this route will query DP, get fresh route, cache it.

## Scenario 3: Delete Upstream (with cascade)

### Management Request

```http
DELETE /api/oagw/v1/upstreams/gts.x.core.oagw.upstream.v1~7c9e6679...?cascade=true HTTP/1.1
Host: oagw.example.com
Authorization: Bearer <tenant-token>
```

### Invalidation Flow

```
Control Plane:
  1. Find all routes for upstream
     SELECT id FROM oagw_route WHERE upstream_id = ...
     Result: [route1, route2, route3]

  2. Delete routes (cascade)
     DELETE FROM oagw_route WHERE upstream_id = ...

  3. Delete upstream
     DELETE FROM oagw_upstream WHERE id = ...

  4. Invalidate all affected cache keys:
     DP L1/L2:
       - upstream:{tenant}:openai
       - route:{upstream_id}:POST:/v1/chat/completions
       - route:{upstream_id}:POST:/v1/completions
       - route:{upstream_id}:GET:/v1/models

  5. Return 204 No Content

API Handler:
  Notify CP to invalidate all affected keys
```

**Result**: All cached config for this upstream and its routes invalidated.

## Scenario 4: Multi-Instance Environment

### Setup

3 Control Plane instances behind load balancer, shared Redis L2 cache.
2 Data Plane instances.

### Update Flow

```
Client → API Handler → DP Instance 1
  ↓
DP1:
  1. Write to database (shared)
  2. Flush DP1 L1 cache (local)
  3. Flush Redis L2 cache (shared)
  4. Return success
  ↓
API Handler → Broadcast to all CP instances
  ↓
CP1: Flush L1 cache (local)
CP2: Flush L1 cache (local)
```

### Next Request to Different Instance

```
Client → API Handler → CP2
  ↓
CP2 L1: MISS (just flushed)
  ↓
CP2 → DP Instance 2 (load balanced)
  ↓
DP2 L1: MISS (different instance)
  ↓
DP2 L2 (Redis): MISS (shared, was flushed)
  ↓
DP2 Database: HIT (shared, has fresh data)
  ↓
DP2 populates L1 and L2
  ↓
DP2 → CP2
  ↓
CP2 populates L1
  ↓
Fresh config used
```

**Result**: Invalidation works across multiple instances via shared L2 and DB.

## Invalidation Timing

| Event               | Latency         |
|---------------------|-----------------|
| DP L1 flush         | <1ms (local)    |
| DP L2 flush (Redis) | 1-2ms (network) |
| CP notification     | 10-50ms (async) |
| CP L1 flush         | <1ms (local)    |

**Total window**: ~50-100ms from write to all caches invalidated.

**Staleness window**: Worst case 100ms for CP to receive notification and flush.

## Consistency Guarantees

### Strong Consistency in DP

- L1 flush before returning success
- L2 flush before returning success
- Next DP read sees fresh data

### Eventual Consistency in CP

- Notification is asynchronous
- CP may serve stale data for ~50-100ms
- Acceptable for MVP (config changes are rare)

### Future: Immediate Consistency

For strict consistency requirements:

1. CP waits for DP write + invalidation to complete
2. CP synchronously flushes its L1 before proxy call
3. Adds latency, only enable if needed

## Cache Key Patterns

### Upstream

```
upstream:{tenant_id}:{alias}
```

Invalidated on:

- `PUT /upstreams/{id}`
- `DELETE /upstreams/{id}`

### Route

```
route:{upstream_id}:{method}:{path}
```

Invalidated on:

- `PUT /routes/{id}`
- `DELETE /routes/{id}`
- `DELETE /upstreams/{id}` (cascade)

### Plugin

```
plugin:{plugin_id}
```

Invalidated on:

- `PUT /plugins/{id}` (immutable, rare)
- `DELETE /plugins/{id}`

## Failure Scenarios

### Redis Unavailable During Invalidation

```
DP writes to database
  ↓
DP flushes L1: SUCCESS
  ↓
DP tries to flush L2 (Redis): FAILURE
  ↓
DP logs error, continues
  ↓
Returns success (L1 flushed, DB updated)
```

**Impact**:

- DP instance L1 is fresh
- Other DP instances L2 cache may be stale (until TTL expires)
- Max staleness: 5 minutes (L2 TTL)

**Mitigation**: L2 TTL prevents unbounded staleness.

### CP Notification Failure

```
DP completes write and flush
  ↓
API Handler sends CP notification: FAILURE
  ↓
API Handler logs error, continues
  ↓
Returns success to client
```

**Impact**:

- CP L1 cache may be stale until TTL or eviction
- CP cache is small (1000 entries), likely to be evicted soon

**Mitigation**:

- CP cache has implicit TTL via LRU eviction
- Config changes are rare, brief staleness acceptable for MVP

### Database Write Fails

```
DP writes to database: FAILURE
  ↓
DP does not flush caches (transaction failed)
  ↓
Returns error to client
```

**Impact**: None (caches remain consistent with database).

## Monitoring

Key metrics for cache invalidation:

- `oagw_cache_invalidation_total{layer="l1|l2|cp"}` - count
- `oagw_cache_invalidation_duration_seconds{layer}` - histogram
- `oagw_cache_invalidation_errors_total{layer, error_type}` - count

Alert on:

- High invalidation error rate (>1% for L2, >5% for CP notification)
- Slow invalidation (>100ms for L2 flush)

## Related ADRs

- [ADR: Control Plane Caching](../docs/adr-data-plane-caching.md)
- [ADR: State Management](../docs/adr-state-management.md)
- [ADR: Component Architecture](../docs/adr-component-architecture.md)
