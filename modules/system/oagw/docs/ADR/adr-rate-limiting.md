# ADR: Rate Limiting

- **Status**: Accepted
- **Date**: 2026-02-03
- **Deciders**: OAGW Team

## Context and Problem Statement

OAGW needs a rate limiting strategy that addresses:

1. **Configuration**: How to define rate limits (algorithm, parameters, granularity)
2. **Inheritance**: How limits propagate through tenant hierarchy (system → partner → tenant)
3. **Distributed State**: How to share rate limit counters across OAGW nodes

Current state in PRD/DESIGN defines basic rate limit fields (`rate`, `window`, `capacity`, `cost`, `scope`, `strategy`) but lacks:

- Algorithm selection
- Distributed synchronization strategy
- Budget allocation across hierarchy
- Quota management

## Decision Drivers

- Low latency impact (<1ms for rate check)
- Accurate enforcement across distributed nodes
- Hierarchical budget allocation (parent can cap children)
- Fair sharing among tenants
- Burst handling without starving steady traffic
- Observability (remaining quota, reset time)

## Considered Options

### Option 1: Algorithm Selection

| Algorithm          | Pros                                    | Cons                                 | Use Case                |
|--------------------|-----------------------------------------|--------------------------------------|-------------------------|
| **Token Bucket**   | Allows bursts, simple, memory efficient | Burst at window boundary             | Default for most APIs   |
| **Leaky Bucket**   | Smooth output rate                      | No burst tolerance, queue management | Steady-rate backends    |
| **Fixed Window**   | Simple, predictable reset               | 2x burst at boundary                 | Simple quotas           |
| **Sliding Window** | No boundary burst, accurate             | Slightly more compute                | Strict rate enforcement |

**Recommendation**: Token Bucket as default (industry standard: AWS API Gateway, Kong, Envoy). Sliding Window as option for strict enforcement.

### Option 2: Configuration Model

**Option 2A: Flat Configuration (Current)**

```json
{
  "rate_limit": {
    "rate": 1000,
    "window": "minute",
    "capacity": 100,
    "scope": "tenant"
  }
}
```

- Simple but limited
- No algorithm choice
- No burst/sustained distinction

**Option 2B: Token Bucket Configuration**

```json
{
  "rate_limit": {
    "algorithm": "token_bucket",
    "tokens_per_second": 100,
    "bucket_capacity": 500,
    "scope": "tenant"
  }
}
```

- Explicit algorithm
- Clear burst (capacity) vs sustained (tokens_per_second) distinction
- Matches AWS/Kong model

**Option 2C: Dual-Rate Configuration (Recommended)**

```json
{
  "rate_limit": {
    "algorithm": "token_bucket",
    "sustained": { "rate": 100, "window": "second" },
    "burst": { "capacity": 500 },
    "scope": "tenant",
    "strategy": "reject"
  }
}
```

- Separates sustained rate from burst capacity
- Human-readable window units
- Extensible for future algorithms

### Option 3: Hierarchical Inheritance

**Problem**: System operator sets global limit (10,000 req/min). Partner gets allocation (5,000 req/min). Partner's tenants share that allocation.

**Option 3A: Independent Limits (No Sharing)**

Each level sets own limit. No coordination.

```
System: 10,000/min (global cap)
Partner A: 5,000/min (own limit)
  Tenant A1: 1,000/min
  Tenant A2: 1,000/min
```

- Simple
- Partner's tenants can exceed partner's allocation (1000+1000 < 5000, but no enforcement)
- No budget guarantee

**Option 3B: Hierarchical Budget Allocation (Recommended)**

Parent allocates budget to children. Children cannot exceed allocation.

```
System: 10,000/min (total capacity)
├── Partner A: 5,000/min (allocated by system)
│   ├── Tenant A1: 2,000/min (allocated by partner)
│   └── Tenant A2: 1,000/min (allocated by partner)
│   └── (unallocated: 2,000/min - reserved or shared pool)
└── Partner B: 3,000/min (allocated by system)
└── (unallocated: 2,000/min - system reserve)
```

**Enforcement rules**:

- Child limit ≤ parent allocation
- Sum of child allocations ≤ parent allocation (optional: allow overcommit with `overcommit_ratio`)
- Effective limit = `min(own_limit, parent_effective_limit)`

**Option 3C: Shared Pool with Guarantees**

Parent defines guaranteed minimum + shared pool.

```json
{
  "rate_limit": {
    "budget": {
      "total": 5000,
      "guaranteed": {
        "tenant_a1": 1000,
        "tenant_a2": 500
      },
      "shared_pool": 3500
    }
  }
}
```

- Complex but fair
- Guarantees minimum for each tenant
- Shared pool for burst/overflow

### Option 4: Distributed State Synchronization

**Problem**: Multiple OAGW nodes must coordinate rate limit counters.

**Option 4A: Local-Only (No Sync)**

Each node tracks own counters. Effective limit = `configured_limit / node_count`.

- No external dependency
- Inaccurate if traffic not evenly distributed
- Simple

**Option 4B: Centralized Store (Redis)**

All nodes read/write to Redis.

```
Node 1 ──┐
Node 2 ──┼── Redis (atomic INCR + EXPIRE)
Node 3 ──┘
```

- Accurate global enforcement
- Added latency (~1-2ms per request)
- Redis becomes SPOF

**Option 4C: Hybrid Local + Periodic Sync (Recommended)**

Local token bucket with periodic sync to central store.

```
┌─────────────────────────────────────────────────────────┐
│  OAGW Node                                              │
│  ┌─────────────────┐    ┌─────────────────────────────┐ │
│  │ Local Bucket    │◄───│ Sync Thread (every 100ms)   │ │
│  │ - Fast check    │    │ - Push local count          │ │
│  │ - Approximate   │    │ - Pull global count         │ │
│  └─────────────────┘    │ - Adjust local tokens       │ │
│                         └─────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │  Redis/Valkey   │
                    │  (global state) │
                    └─────────────────┘
```

**Sync algorithm**:

1. Each node maintains local token bucket
2. Periodically (configurable, default 100ms):
    - Push: `INCRBY global_counter local_consumed`
    - Pull: `GET global_counter`
    - Adjust local bucket based on global state
3. Local check is fast (in-memory)
4. Global accuracy within sync interval

**Trade-offs**:

- Sync interval short → more accurate, more Redis load
- Sync interval long → less accurate, less Redis load
- Burst can exceed limit by `burst_capacity * node_count` in worst case

**Option 4D: Quota-Based (Envoy-style)**

Central service allocates quota to nodes. Nodes request more when depleted.

```
Node 1: "Give me quota for tenant X"
Quota Service: "Here's 100 tokens, valid for 10s"
Node 1: Uses tokens locally, requests more when low
```

- Most accurate
- Complex implementation
- Requires quota service

## Decision Outcome

**Component Architecture Note**: Rate limiting executes in **Data Plane (CP)**. CP owns rate limiters per-instance for MVP. Configuration is resolved from Control Plane (DP) caches
during upstream/route resolution. See [ADR: State Management](./adr-state-management.md) for component responsibilities.

### 1. Algorithm: Token Bucket (default), Sliding Window (optional)

Token bucket is industry standard, handles bursts well, simple to implement.

### 2. Configuration Model: Dual-Rate (Option 2C)

```json
{
  "rate_limit": {
    "sharing": "enforce",
    "algorithm": "token_bucket",
    "sustained": {
      "rate": 100,
      "window": "second"
    },
    "burst": {
      "capacity": 500
    },
    "scope": "tenant",
    "strategy": "reject",
    "response_headers": true
  }
}
```

**Fields**:

| Field              | Type | Default          | Description                                              |
|--------------------|------|------------------|----------------------------------------------------------|
| `sharing`          | enum | `private`        | `private`, `inherit`, `enforce`                          |
| `algorithm`        | enum | `token_bucket`   | `token_bucket`, `sliding_window`                         |
| `sustained.rate`   | int  | required         | Tokens replenished per window                            |
| `sustained.window` | enum | `second`         | `second`, `minute`, `hour`, `day`                        |
| `burst.capacity`   | int  | `sustained.rate` | Max bucket size (burst allowance)                        |
| `scope`            | enum | `tenant`         | Counter scope: `global`, `tenant`, `user`, `ip`, `route` |
| `strategy`         | enum | `reject`         | `reject` (429), `queue`, `degrade`                       |
| `response_headers` | bool | `true`           | Include `X-RateLimit-*` headers                          |
| `cost`             | int  | `1`              | Tokens consumed per request                              |

### 3. Hierarchical Inheritance: Budget Allocation (Option 3B)

**Schema extension**:

```json
{
  "rate_limit": {
    "sharing": "enforce",
    "budget": {
      "mode": "allocated",
      "total": 5000,
      "overcommit_ratio": 1.0
    },
    "sustained": { "rate": 100, "window": "second" },
    "burst": { "capacity": 500 }
  }
}
```

**Budget modes**:

| Mode        | Description                                              |
|-------------|----------------------------------------------------------|
| `unlimited` | No budget tracking (default for leaf tenants)            |
| `allocated` | Parent allocates fixed budget to children                |
| `shared`    | Children share parent's budget (first-come-first-served) |

**Inheritance rules**:

| Parent Sharing | Child Specifies | Effective Limit                             |
|----------------|-----------------|---------------------------------------------|
| `private`      | any             | Child's limit only                          |
| `inherit`      | none            | Parent's limit                              |
| `inherit`      | own limit       | `min(parent, child)`                        |
| `enforce`      | any             | `min(parent, child)` - cannot exceed parent |

**Budget validation on child creation**:

```
Parent budget: 5000/min
Existing children: A (2000), B (1000)
New child C requests: 3000

If overcommit_ratio = 1.0:
  Sum = 2000 + 1000 + 3000 = 6000 > 5000 → REJECT

If overcommit_ratio = 1.5:
  Sum = 6000 ≤ 5000 * 1.5 = 7500 → ALLOW (with warning)
```

### 4. Distributed State: Hybrid Local + Periodic Sync (Option 4C)

**MVP Implementation**: Per-instance rate limiting in Data Plane (no distributed coordination). Each CP instance maintains local token buckets. Acceptable for MVP since traffic
is typically distributed evenly by load balancer.

**Future (Phase 2)**: Redis-backed distributed rate limiting for strict global enforcement across CP instances.

**Configuration**:

```json
{
  "rate_limit_sync": {
    "enabled": true,
    "backend": "redis",
    "sync_interval_ms": 100,
    "fallback_on_error": "local_only"
  }
}
```

**Behavior**:

- Normal: Local bucket + periodic sync
- Redis unavailable: Fall back to local-only (degraded accuracy)
- High load: Adaptive sync interval (more frequent when near limit)

**Redis key structure**:

```
oagw:ratelimit:{scope}:{identifier}:{window} = {count}
oagw:ratelimit:tenant:uuid-123:minute:2026013015 = 4523
```

**Response headers** (RFC 6585 / draft-ietf-httpapi-ratelimit-headers):

```http
HTTP/1.1 429 Too Many Requests
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 0
X-RateLimit-Reset: 1706626800
Retry-After: 30
```

## Schema Changes

### Upstream Rate Limit (updated)

```json
{
  "rate_limit": {
    "type": "object",
    "properties": {
      "sharing": {
        "type": "string",
        "enum": [ "private", "inherit", "enforce" ],
        "default": "private"
      },
      "algorithm": {
        "type": "string",
        "enum": [ "token_bucket", "sliding_window" ],
        "default": "token_bucket"
      },
      "sustained": {
        "type": "object",
        "properties": {
          "rate": { "type": "integer", "minimum": 1 },
          "window": {
            "type": "string",
            "enum": [ "second", "minute", "hour", "day" ],
            "default": "second"
          }
        },
        "required": [ "rate" ]
      },
      "burst": {
        "type": "object",
        "properties": {
          "capacity": { "type": "integer", "minimum": 1 }
        }
      },
      "budget": {
        "type": "object",
        "properties": {
          "mode": {
            "type": "string",
            "enum": [ "unlimited", "allocated", "shared" ],
            "default": "unlimited"
          },
          "total": { "type": "integer", "minimum": 1 },
          "overcommit_ratio": {
            "type": "number",
            "minimum": 1.0,
            "maximum": 2.0,
            "default": 1.0
          }
        }
      },
      "scope": {
        "type": "string",
        "enum": [ "global", "tenant", "user", "ip", "route" ],
        "default": "tenant"
      },
      "strategy": {
        "type": "string",
        "enum": [ "reject", "queue", "degrade" ],
        "default": "reject"
      },
      "cost": {
        "type": "integer",
        "minimum": 1,
        "default": 1
      },
      "response_headers": {
        "type": "boolean",
        "default": true
      }
    },
    "required": [ "sustained" ]
  }
}
```

## Examples

### Example 1: System → Partner → Tenant Hierarchy

**System level** (global cap):

```json
{
  "rate_limit": {
    "sharing": "enforce",
    "sustained": { "rate": 10000, "window": "minute" },
    "burst": { "capacity": 1000 },
    "budget": { "mode": "allocated", "total": 10000 }
  }
}
```

**Partner level** (allocated 5000/min by system):

```json
{
  "rate_limit": {
    "sharing": "enforce",
    "sustained": { "rate": 5000, "window": "minute" },
    "burst": { "capacity": 500 },
    "budget": { "mode": "allocated", "total": 5000, "overcommit_ratio": 1.2 }
  }
}
```

**Tenant level** (allocated 1000/min by partner):

```json
{
  "rate_limit": {
    "sustained": { "rate": 1000, "window": "minute" },
    "burst": { "capacity": 100 }
  }
}
```

**Effective for tenant**:

- Sustained: `min(system:10000, partner:5000, tenant:1000)` = 1000/min
- Burst: `min(system:1000, partner:500, tenant:100)` = 100

### Example 2: Shared Pool Among Tenants

**Partner with shared pool**:

```json
{
  "rate_limit": {
    "sharing": "inherit",
    "sustained": { "rate": 5000, "window": "minute" },
    "budget": { "mode": "shared", "total": 5000 }
  }
}
```

Tenants A, B, C share 5000/min pool. First-come-first-served. No individual guarantees.

### Example 3: Cost-Based Rate Limiting

Different endpoints have different costs:

```json
{
  "routes": [
    {
      "path": "/v1/chat/completions",
      "rate_limit": { "cost": 10 }
    },
    {
      "path": "/v1/models",
      "rate_limit": { "cost": 1 }
    }
  ]
}
```

Tenant with 1000 tokens/min can call:

- 100 chat completions, or
- 1000 model listings, or
- 50 chat + 500 models

## Implementation Notes

### Token Bucket Algorithm

```rust
struct TokenBucket {
    tokens: f64,
    last_update: Instant,
    capacity: f64,
    refill_rate: f64, // tokens per second
}

impl TokenBucket {
    fn try_acquire(&mut self, cost: u32) -> bool {
        self.refill();
        if self.tokens >= cost as f64 {
            self.tokens -= cost as f64;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity);
        self.last_update = now;
    }
}
```

### Redis Sync Protocol

```rust
// Every sync_interval_ms
async fn sync_with_redis(local: &mut TokenBucket, key: &str, redis: &Redis) {
    // 1. Push local consumption
    let consumed = local.consumed_since_last_sync();
    redis.incrby(key, consumed).await;

    // 2. Pull global state
    let global_count = redis.get(key).await;

    // 3. Adjust local bucket
    // If global is higher than expected, reduce local tokens
    local.adjust_from_global(global_count);
}
```

## Consequences

### Positive

- Clear separation of sustained rate vs burst capacity
- Hierarchical budget prevents child from exceeding parent allocation
- Hybrid sync balances accuracy vs performance
- Standard response headers for client integration

### Negative

- Redis dependency for distributed accuracy
- Complexity in budget allocation validation
- Sync interval introduces accuracy trade-off

### Risks

- Redis failure degrades to local-only (less accurate)
- Overcommit can lead to contention under high load
- Clock skew between nodes affects sliding window accuracy

## Links

- [OAGW Design Document](../DESIGN.md)
- [ADR: Resource Identification](./adr-resource-identification.md)
- [Kong Rate Limiting](https://konghq.com/blog/engineering/how-to-design-a-scalable-rate-limiting-algorithm)
- [Envoy Global Rate Limiting](https://www.envoyproxy.io/docs/envoy/latest/intro/arch_overview/other_features/global_rate_limiting)
- [AWS API Gateway Throttling](https://docs.aws.amazon.com/apigateway/latest/developerguide/api-gateway-request-throttling.html)
- [IETF RateLimit Headers Draft](https://datatracker.ietf.org/doc/draft-ietf-httpapi-ratelimit-headers/)
