# Feature: Traffic Resilience

- [ ] `p1` - **ID**: `cpt-cf-oagw-featstatus-traffic-resilience`

- [ ] `p1` - `cpt-cf-oagw-feature-traffic-resilience`

## 1. Feature Context

### 1.1 Overview

Rate limiting and circuit breaker capabilities that protect OAGW and upstream services from overload and cascade failures. Rate limiting enforces configurable request quotas per upstream/route with token bucket algorithm. Circuit breaker automatically stops forwarding requests to degraded upstreams.

### 1.2 Purpose

External APIs are often paid and rate-limited by the provider. Without gateway-level rate limiting, tenants risk cost overruns and provider-side throttling. Without circuit breakers, a single degraded upstream can consume connection pool resources and degrade the entire gateway.

Addresses PRD requirements: `cpt-cf-oagw-fr-rate-limiting`, `cpt-cf-oagw-fr-circuit-breaker`.

### 1.3 Actors

| Actor | Role in Feature |
|-------|-----------------|
| `cpt-cf-oagw-actor-platform-operator` | Configures rate limits and circuit breaker thresholds on upstreams/routes |
| `cpt-cf-oagw-actor-tenant-admin` | Sets tenant-level rate limits subject to hierarchical min-merge |
| `cpt-cf-oagw-actor-app-developer` | Receives 429/503 responses when limits are exceeded or circuit is open |

### 1.4 References

- **PRD**: [PRD.md](../PRD.md)
- **Design**: [DESIGN.md](../DESIGN.md)
- **Requirements**: `cpt-cf-oagw-fr-rate-limiting`, `cpt-cf-oagw-fr-circuit-breaker`
- **Design elements**: `cpt-cf-oagw-adr-rate-limiting`, `cpt-cf-oagw-adr-circuit-breaker`, `cpt-cf-oagw-adr-concurrency-control`, `cpt-cf-oagw-adr-backpressure-queueing`
- **Dependencies**: `cpt-cf-oagw-feature-proxy-auth` (rate limiting and circuit breaker execute within the proxy pipeline)

## 2. Actor Flows (CDSL)

### Rate Limited Request Flow

- [ ] `p1` - **ID**: `cpt-cf-oagw-flow-rate-limited-request`

**Actor**: `cpt-cf-oagw-actor-app-developer`

**Success Scenarios**:
- Request is within rate limit; proceeds through proxy pipeline normally
- Rate limit counter is decremented

**Error Scenarios**:
- Rate limit exceeded with reject strategy: 429 with Retry-After header
- Rate limit exceeded with queue strategy: request is queued for later execution
- Rate limit exceeded with degrade strategy: request proceeds with degraded QoS

**Steps**:
1. [ ] - `p1` - Proxy pipeline resolves upstream and route (per proxy flow) - `inst-rl-1`
2. [ ] - `p1` - Evaluate rate limit for the applicable scope (global, tenant, user, or IP) - `inst-rl-2`
3. [ ] - `p1` - Compute effective rate limit: apply route limit first, then upstream limit - `inst-rl-3`
4. [ ] - `p1` - **IF** hierarchical configuration active, apply min-merge with ancestor enforced limits - `inst-rl-4`
5. [ ] - `p1` - Attempt to acquire token from token bucket - `inst-rl-5`
6. [ ] - `p1` - **IF** token acquired - `inst-rl-6`
   1. [ ] - `p1` - Continue proxy pipeline - `inst-rl-6a`
7. [ ] - `p1` - **IF** token not available and strategy = reject - `inst-rl-7`
   1. [ ] - `p1` - **RETURN** 429 RateLimitExceeded with Retry-After header and X-OAGW-Error-Source: gateway - `inst-rl-7a`
8. [ ] - `p1` - **IF** token not available and strategy = queue - `inst-rl-8`
   1. [ ] - `p1` - Enqueue request in bounded queue for later execution - `inst-rl-8a`
9. [ ] - `p1` - **IF** token not available and strategy = degrade - `inst-rl-9`
   1. [ ] - `p1` - Forward request with degraded quality of service - `inst-rl-9a`

### Circuit Breaker Trip Flow

- [ ] `p2` - **ID**: `cpt-cf-oagw-flow-circuit-breaker-trip`

**Actor**: `cpt-cf-oagw-actor-app-developer`

**Success Scenarios**:
- Circuit is closed; request proceeds normally
- Circuit is half-open; probe request is allowed through

**Error Scenarios**:
- Circuit is open: 503 CircuitBreakerOpen returned immediately without upstream call

**Steps**:
1. [ ] - `p2` - Proxy pipeline reaches circuit breaker check (after auth, before upstream call) - `inst-cb-1`
2. [ ] - `p2` - Check circuit breaker state for the target upstream - `inst-cb-2`
3. [ ] - `p2` - **IF** state = CLOSED - `inst-cb-3`
   1. [ ] - `p2` - Allow request through to upstream - `inst-cb-3a`
   2. [ ] - `p2` - **IF** upstream responds with error, increment failure counter - `inst-cb-3b`
   3. [ ] - `p2` - **IF** failure count exceeds threshold, transition to OPEN - `inst-cb-3c`
4. [ ] - `p2` - **IF** state = OPEN - `inst-cb-4`
   1. [ ] - `p2` - **IF** recovery window has not elapsed - `inst-cb-4a`
      1. [ ] - `p2` - **RETURN** 503 CircuitBreakerOpen with Retry-After and X-OAGW-Error-Source: gateway - `inst-cb-4a1`
   2. [ ] - `p2` - **IF** recovery window elapsed, transition to HALF_OPEN - `inst-cb-4b`
5. [ ] - `p2` - **IF** state = HALF_OPEN - `inst-cb-5`
   1. [ ] - `p2` - Allow single probe request through - `inst-cb-5a`
   2. [ ] - `p2` - **IF** probe succeeds, transition to CLOSED, reset failure counter - `inst-cb-5b`
   3. [ ] - `p2` - **IF** probe fails, transition back to OPEN, reset recovery window - `inst-cb-5c`

## 3. Processes / Business Logic (CDSL)

### Token Bucket Rate Limiter Algorithm

- [ ] `p1` - **ID**: `cpt-cf-oagw-algo-token-bucket`

**Input**: Rate limit configuration (rate, window, capacity, cost, scope), request context

**Output**: Token acquired (proceed) or token denied (reject/queue/degrade)

**Steps**:
1. [ ] - `p1` - Determine bucket key from scope: global = upstream_id, tenant = upstream_id+tenant_id, user = upstream_id+principal_id, IP = upstream_id+client_ip - `inst-tb-1`
2. [ ] - `p1` - Look up or create token bucket for key - `inst-tb-2`
3. [ ] - `p1` - Refill tokens based on elapsed time since last refill: tokens += elapsed_seconds * (rate / window_seconds) - `inst-tb-3`
4. [ ] - `p1` - Cap tokens at burst capacity - `inst-tb-4`
5. [ ] - `p1` - **IF** tokens >= request cost - `inst-tb-5`
   1. [ ] - `p1` - Deduct cost from tokens - `inst-tb-5a`
   2. [ ] - `p1` - **RETURN** token acquired - `inst-tb-5b`
6. [ ] - `p1` - Compute retry_after_seconds = (cost - tokens) / (rate / window_seconds) - `inst-tb-6`
7. [ ] - `p1` - **RETURN** token denied with retry_after_seconds - `inst-tb-7`

### Rate Limit Effective Computation Algorithm

- [ ] `p1` - **ID**: `cpt-cf-oagw-algo-rate-limit-effective`

**Input**: Route rate_limit config, upstream rate_limit config, ancestor enforced rate_limits (if hierarchical)

**Output**: Effective rate limit to apply

**Steps**:
1. [ ] - `p1` - Start with route rate_limit (most specific) - `inst-rle-1`
2. [ ] - `p1` - **IF** route rate_limit is null, use upstream rate_limit - `inst-rle-2`
3. [ ] - `p1` - **IF** both present, effective = min(route.rate, upstream.rate) - `inst-rle-3`
4. [ ] - `p1` - **FOR EACH** ancestor with sharing=enforce rate_limit - `inst-rle-4`
   1. [ ] - `p1` - effective.rate = min(effective.rate, ancestor.rate) - `inst-rle-4a`
5. [ ] - `p1` - **RETURN** effective rate limit configuration - `inst-rle-5`

### Circuit Breaker State Evaluation Algorithm

- [ ] `p2` - **ID**: `cpt-cf-oagw-algo-circuit-breaker-eval`

**Input**: Upstream circuit breaker configuration, current state, upstream response

**Output**: Updated circuit breaker state

**Steps**:
1. [ ] - `p2` - **IF** upstream response is success (2xx/3xx) - `inst-cbe-1`
   1. [ ] - `p2` - Reset consecutive failure counter - `inst-cbe-1a`
   2. [ ] - `p2` - **IF** state = HALF_OPEN, transition to CLOSED - `inst-cbe-1b`
2. [ ] - `p2` - **IF** upstream response is error (5xx) or timeout - `inst-cbe-2`
   1. [ ] - `p2` - Increment consecutive failure counter - `inst-cbe-2a`
   2. [ ] - `p2` - **IF** failure_count >= threshold and state = CLOSED - `inst-cbe-2b`
      1. [ ] - `p2` - Transition to OPEN, record open_at timestamp - `inst-cbe-2b1`
      2. [ ] - `p2` - Emit metric: oagw_circuit_breaker_transitions_total{from=CLOSED, to=OPEN} - `inst-cbe-2b2`
   3. [ ] - `p2` - **IF** state = HALF_OPEN - `inst-cbe-2c`
      1. [ ] - `p2` - Transition back to OPEN, reset recovery window - `inst-cbe-2c1`
3. [ ] - `p2` - **RETURN** updated state - `inst-cbe-3`

## 4. States (CDSL)

### Circuit Breaker State Machine

- [ ] `p2` - **ID**: `cpt-cf-oagw-state-circuit-breaker`

**States**: CLOSED, OPEN, HALF_OPEN

**Initial State**: CLOSED

**Transitions**:
1. [ ] - `p2` - **FROM** CLOSED **TO** OPEN **WHEN** consecutive failure count exceeds configured threshold - `inst-cbs-1`
2. [ ] - `p2` - **FROM** OPEN **TO** HALF_OPEN **WHEN** recovery window duration elapses - `inst-cbs-2`
3. [ ] - `p2` - **FROM** HALF_OPEN **TO** CLOSED **WHEN** probe request succeeds - `inst-cbs-3`
4. [ ] - `p2` - **FROM** HALF_OPEN **TO** OPEN **WHEN** probe request fails - `inst-cbs-4`

**Effects**:
- CLOSED: all requests forwarded normally; failures tracked
- OPEN: all requests immediately rejected with 503 CircuitBreakerOpen; no upstream calls made
- HALF_OPEN: single probe request allowed; all other requests rejected with 503

**Metrics**:
- `oagw_circuit_breaker_state{host}` gauge: 0=CLOSED, 1=HALF_OPEN, 2=OPEN
- `oagw_circuit_breaker_transitions_total{host, from_state, to_state}` counter

## 5. Definitions of Done

### Implement Token Bucket Rate Limiter

- [ ] `p1` - **ID**: `cpt-cf-oagw-dod-rate-limiter`

The system **MUST** implement token bucket rate limiting at upstream and route levels with configurable rate, window, burst capacity, cost, and scope (global/tenant/user/IP). The system **MUST** return 429 with Retry-After header when the reject strategy is active. Rate limit evaluation **MUST** not buffer request bodies before the decision.

**Implements**:
- `cpt-cf-oagw-flow-rate-limited-request`
- `cpt-cf-oagw-algo-token-bucket`
- `cpt-cf-oagw-algo-rate-limit-effective`

**Touches**:
- API: `{METHOD} /api/oagw/v1/proxy/{alias}/*`
- DB: `oagw_upstream.rate_limit`, `oagw_route.rate_limit`
- Entities: `DataPlaneService`, `RateLimiter`

### Implement Circuit Breaker

- [ ] `p2` - **ID**: `cpt-cf-oagw-dod-circuit-breaker`

The system **MUST** implement circuit breaker as a core gateway resilience capability (not a plugin) with three states (CLOSED, OPEN, HALF_OPEN). When the circuit is open, the system **MUST** return 503 CircuitBreakerOpen with Retry-After header. Half-open probe gating **MUST** be atomic to avoid multi-node probe floods.

**Implements**:
- `cpt-cf-oagw-flow-circuit-breaker-trip`
- `cpt-cf-oagw-algo-circuit-breaker-eval`
- `cpt-cf-oagw-state-circuit-breaker`

**Touches**:
- API: `{METHOD} /api/oagw/v1/proxy/{alias}/*`
- Entities: `DataPlaneService`, `CircuitBreaker`

### Implement Rate Limit Metrics

- [ ] `p1` - **ID**: `cpt-cf-oagw-dod-rate-limit-metrics`

The system **MUST** expose Prometheus metrics for rate limiting: `oagw_rate_limit_exceeded_total{host, path}` counter and `oagw_rate_limit_usage_ratio{host, path}` gauge.

**Implements**:
- `cpt-cf-oagw-flow-rate-limited-request`

**Touches**:
- Entities: `RateLimiter`, metrics endpoint

### Implement Circuit Breaker Metrics

- [ ] `p2` - **ID**: `cpt-cf-oagw-dod-circuit-breaker-metrics`

The system **MUST** expose Prometheus metrics for circuit breaker: `oagw_circuit_breaker_state{host}` gauge and `oagw_circuit_breaker_transitions_total{host, from_state, to_state}` counter. State changes **MUST** be logged as audit events.

**Implements**:
- `cpt-cf-oagw-state-circuit-breaker`

**Touches**:
- Entities: `CircuitBreaker`, metrics endpoint

## 6. Acceptance Criteria

- [ ] Rate limit with reject strategy returns 429 with Retry-After header when limit exceeded
- [ ] Rate limit with queue strategy enqueues request when limit exceeded
- [ ] Rate limit with degrade strategy forwards request with degraded QoS when limit exceeded
- [ ] Token bucket correctly refills tokens based on elapsed time and configured rate/window
- [ ] Per-request cost is deducted from token bucket
- [ ] Rate limit scope correctly partitions buckets (global vs tenant vs user vs IP)
- [ ] Effective rate limit is min(route, upstream, ancestor_enforced) when hierarchy is active
- [ ] Rate limit decision does not buffer request body
- [ ] Circuit breaker in CLOSED state forwards all requests and tracks failures
- [ ] Circuit breaker transitions to OPEN when failure threshold is exceeded
- [ ] Circuit breaker in OPEN state returns 503 immediately without upstream call
- [ ] Circuit breaker transitions to HALF_OPEN after recovery window elapses
- [ ] Single probe request is allowed in HALF_OPEN; success transitions to CLOSED
- [ ] Failed probe in HALF_OPEN transitions back to OPEN
- [ ] Rate limit metrics (exceeded counter, usage ratio gauge) are exposed at /metrics
- [ ] Circuit breaker metrics (state gauge, transitions counter) are exposed at /metrics
- [ ] Circuit breaker state changes are logged as audit events

## 7. Not Applicable Sections

**Compliance (COMPL)**: Not applicable because rate limiting and circuit breaker are infrastructure resilience mechanisms that do not handle regulated data.

**Privacy (COMPL)**: Not applicable because rate limit counters use opaque scope keys (tenant_id, principal_id hash) without storing PII.

**User-Facing Architecture (UX)**: Not applicable because this is a backend infrastructure capability with no user-facing frontend.
