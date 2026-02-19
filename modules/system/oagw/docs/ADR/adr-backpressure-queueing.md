# ADR: Backpressure and Queueing

- **Status**: Proposed
- **Date**: 2026-02-03
- **Deciders**: OAGW Team

## Context and Problem Statement

When concurrency or rate limits are exceeded, OAGW needs a strategy beyond simple rejection. Immediately rejecting requests causes:

1. **Poor user experience**: Clients see errors during traffic spikes
2. **Thundering herd**: Many clients retry simultaneously, worsening the problem
3. **Wasted work**: Client may have already sent request body, connection established
4. **Cascading failures**: Client-side timeouts and retries compound the issue

OAGW needs backpressure mechanisms to gracefully handle overload and signal clients to slow down.

## Decision Drivers

- Smooth degradation during traffic spikes (avoid hard rejections)
- Prevent resource exhaustion (bounded memory/queue size)
- Signal clients to back off (HTTP 503 + Retry-After)
- Work with both rate limiting and concurrency limiting
- Observable queue behavior (depth, wait time, rejections)
- Configurable strategies per upstream/route

## Backpressure Strategies

When a limit (rate or concurrency) is exceeded, three strategies:

### Strategy 1: `reject` (Default)

**Behavior**: Immediately return error to client.

**Use case**: Fast-fail for APIs where retry logic is client's responsibility.

**Response**:

```http
HTTP/1.1 503 Service Unavailable
Retry-After: 1
X-OAGW-Error-Source: gateway

{
  "type": "gts.x.core.errors.err.v1~x.oagw.concurrency_limit.exceeded.v1",
  "title": "Concurrency Limit Exceeded",
  "status": 503,
  "detail": "Upstream api.openai.com at max concurrent requests (100/100)",
  "retry_after_seconds": 1
}
```

**Advantages**:

- No memory overhead
- Predictable latency (no queueing delay)
- Simple to implement

**Disadvantages**:

- No Poor UX during spikes
- No Clients must implement retry logic

### Strategy 2: `queue`

**Behavior**: Enqueue request until capacity available or timeout.

**Use case**: Smooth traffic bursts without rejecting requests.

**Flow**:

```
Request arrives
  ↓
[Try acquire permit]
  ↓
  ├─ Success → Execute immediately
  └─ Failure → [Enqueue with timeout]
               ↓
               ├─ Permit available before timeout → Execute
               └─ Timeout expires → Return 503
```

**Configuration**:

```json
{
  "concurrency_limit": {
    "max_concurrent": 100,
    "strategy": "queue",
    "queue": {
      "max_depth": 500,
      "timeout": "5s",
      "ordering": "fifo"
    }
  }
}
```

**Advantages**:

- Absorbs traffic spikes
- Better UX (fewer errors)
- Automatic retry handling

**Disadvantages**:

- No Memory overhead (queue storage)
- No Increased latency (queueing delay)
- No Risk of timeout cascade (clients timeout before queue timeout)

### Strategy 3: `degrade`

**Behavior**: Apply degraded routing or fallback behavior.

**Use case**: Graceful degradation with circuit breaker or alternative backends.

**Examples**:

- Route to lower-priority endpoint pool
- Return cached response (if available)
- Reduce request priority (if upstream supports)
- Forward to fallback upstream (if configured)

**Configuration**:

```json
{
  "concurrency_limit": {
    "max_concurrent": 100,
    "strategy": "degrade",
    "degrade": {
      "fallback_upstream_id": "uuid-fallback",
      "fallback_response": {
        "status": 503,
        "body": "{\"error\": \"Service temporarily degraded\"}"
      }
    }
  }
}
```

**Advantages**:

- Maintains availability
- Graceful degradation
- No complete service outage

**Disadvantages**:

- Complex configuration
- Requires fallback infrastructure
- May provide degraded results

**Decision**: Implement `reject` and `queue` first. `degrade` is future enhancement.

## Queue Configuration

### Queue Parameters

```json
{
  "queue": {
    "max_depth": 500,
    "timeout": "5s",
    "ordering": "fifo",
    "memory_limit": "10MB"
  }
}
```

**Fields**:

- `max_depth`: Maximum queued requests (per limit scope)
- `timeout`: Max time request can wait in queue
- `ordering`: `"fifo"` | `"priority"` (priority requires client-provided weight)
- `memory_limit`: Estimated memory cap (requests with large bodies rejected)

### Queue Overflow Behavior

When queue is full (`queue_depth >= max_depth`):

1. Check `overflow_strategy`:
    - `"reject"`: Return 503 immediately
    - `"drop_oldest"`: Evict oldest queued request, enqueue new one
    - `"drop_newest"`: Reject new request (default)

**Recommendation**: `drop_newest` (default) - preserves FIFO fairness.

### Timeout Handling

When queued request times out before permit available:

```http
HTTP/1.1 503 Service Unavailable
Retry-After: 2
X-OAGW-Error-Source: gateway

{
  "type": "gts.x.core.errors.err.v1~x.oagw.queue.timeout.v1",
  "title": "Queue Timeout",
  "status": 503,
  "detail": "Request queued for 5s, no capacity available",
  "queue_wait_seconds": 5.2,
  "retry_after_seconds": 2
}
```

## Request Priority (Future Enhancement)

### Priority-Based Queueing

Allow clients to specify request priority:

```http
POST /api/oagw/v1/proxy/api.openai.com/v1/chat
X-OAGW-Priority: 10

{...}
```

**Priority levels**: 0 (lowest) to 100 (highest). Default: 50.

**Ordering**: Higher priority requests dequeued first.

**Configuration**:

```json
{
  "queue": {
    "ordering": "priority",
    "priority": {
      "allow_client_override": false,
      "default_priority": 50,
      "max_priority": 100
    }
  }
}
```

**Security**: `allow_client_override: false` prevents priority abuse. Use authentication context to assign priority.

## Memory Management

### Request Size Estimation

Queue tracks estimated memory per request:

```
memory_estimate = 
    headers_size +
    body_size (if buffered) +
    metadata_overhead (∼200 bytes)
```

**Large Request Handling**:

- Streaming bodies: Not buffered, only metadata queued (∼200 bytes)
- Buffered bodies: Entire request counted against `memory_limit`
- If `memory_limit` exceeded: Reject with `413 Payload Too Large`

### Queue Memory Tracking

```
struct RequestQueue {
    items: VecDeque<QueuedRequest>,
    total_memory: AtomicUsize,
    config: QueueConfig,
}

impl RequestQueue {
    fn try_enqueue(&mut self, req: QueuedRequest) -> Result<(), QueueError> {
        let new_total = self.total_memory.load() + req.estimated_size;
        
        if new_total > self.config.memory_limit {
            return Err(QueueError::MemoryLimitExceeded);
        }
        
        if self.items.len() >= self.config.max_depth {
            return Err(QueueError::QueueFull);
        }
        
        self.items.push_back(req);
        self.total_memory.fetch_add(req.estimated_size, Ordering::Relaxed);
        Ok(())
    }
}
```

## Client Signaling

### Retry-After Header

All backpressure responses include `Retry-After`:

```http
HTTP/1.1 503 Service Unavailable
Retry-After: 2
```

**Calculation**:

```
if concurrency_limited:
    retry_after = estimate_wait_time()  // avg request duration
elif rate_limited:
    retry_after = window_reset_seconds  // time until next token
else:
    retry_after = 1  // default
```

### Exponential Backoff Recommendation

OAGW should document client retry strategy:

```
backoff_seconds = min(
    initial_backoff * (2 ^ retry_count),
    max_backoff
)

// Example: 1s, 2s, 4s, 8s, 16s, 30s (max)
```

**Jitter**: Add ±20% randomization to prevent thundering herd.

## Integration with Concurrency Control

### Permit Acquisition Flow

```
async fn handle_request(req: Request) -> Response {
    // 1. Check rate limit
    rate_limiter.check().await?;
    
    // 2. Try acquire concurrency permit
    let permit = match concurrency_limiter.try_acquire() {
        Ok(p) => p,
        Err(_) if config.strategy == "reject" => {
            return Err(ConcurrencyLimitExceeded);
        }
        Err(_) if config.strategy == "queue" => {
            // Enqueue and wait
            queue.enqueue(req).await?
        }
    };
    
    // 3. Execute request
    let response = upstream_client.send(req).await?;
    
    // 4. Permit auto-released via Drop
    Ok(response)
}
```

### Queue Consumer

Background task continuously consumes queue when permits available:

```
async fn queue_consumer(queue: RequestQueue, limiter: ConcurrencyLimiter) {
    loop {
        // Wait for permit
        let permit = limiter.acquire().await;
        
        // Dequeue next request
        let req = match queue.dequeue().await {
            Some(req) if !req.is_expired() => req,
            Some(req) => {
                req.respond(QueueTimeout);
                continue;
            }
            None => {
                permit.release();
                tokio::time::sleep(Duration::from_millis(10)).await;
                continue;
            }
        };
        
        // Execute request
        tokio::spawn(async move {
            let response = execute_request(req).await;
            req.respond(response);
            // permit auto-released
        });
    }
}
```

## Interaction with Circuit Breaker

When circuit breaker is **OPEN**:

- Queue does not accumulate requests (immediate rejection)
- Prevents queue from filling with requests that will fail anyway

When circuit breaker is **HALF-OPEN**:

- Queue continues operating normally
- Allows probe requests to test recovery

## Metrics

### Queue Metrics

```promql
# Queue depth (gauge)
oagw_queue_depth{host, level} gauge
# level: "upstream", "route", "tenant"

# Queue wait time (histogram)
oagw_queue_wait_duration_seconds{host} histogram

# Queue rejections (counter)
oagw_queue_rejected_total{host, reason} counter
# reason: "queue_full", "timeout", "memory_limit"

# Queue timeouts (counter)
oagw_queue_timeout_total{host} counter

# Queue memory usage (gauge)
oagw_queue_memory_bytes{host} gauge
```

### Backpressure Metrics

```promql
# Backpressure responses (counter)
oagw_backpressure_total{host, strategy, reason} counter
# strategy: "reject", "queue", "degrade"
# reason: "concurrency_limit", "rate_limit"

# Retry-After values (histogram)
oagw_retry_after_seconds{host} histogram
```

## Error Types

### Queue-Specific Errors

```json
{
  "type": "gts.x.core.errors.err.v1~x.oagw.queue.timeout.v1",
  "title": "Queue Timeout",
  "status": 503,
  "detail": "Request queued for 5s, no capacity available",
  "queue_wait_seconds": 5.2,
  "retry_after_seconds": 2
}
```

```json
{
  "type": "gts.x.core.errors.err.v1~x.oagw.queue.full.v1",
  "title": "Queue Full",
  "status": 503,
  "detail": "Request queue full (500/500), try again later",
  "queue_depth": 500,
  "max_depth": 500,
  "retry_after_seconds": 2
}
```

```json
{
  "type": "gts.x.core.errors.err.v1~x.oagw.queue.memory_limit.v1",
  "title": "Queue Memory Limit Exceeded",
  "status": 503,
  "detail": "Request queue memory limit reached (10MB/10MB)",
  "queue_memory_bytes": 10485760,
  "memory_limit_bytes": 10485760,
  "retry_after_seconds": 1
}
```

## Database Schema

Queue configuration stored in upstream/route `concurrency_limit` or `rate_limit` fields:

```json
{
  "concurrency_limit": {
    "max_concurrent": 100,
    "strategy": "queue",
    "queue": {
      "max_depth": 500,
      "timeout": "5s",
      "ordering": "fifo",
      "memory_limit": "10MB",
      "overflow_strategy": "drop_newest"
    }
  }
}
```

No additional database tables needed (in-memory queue only).

## Configuration Validation

**Rules**:

1. `queue.max_depth` must be > 0 and ≤ 10,000 (prevent excessive memory)
2. `queue.timeout` must be > 0 and ≤ 60s (prevent indefinite queueing)
3. `queue.memory_limit` must be > 0 and ≤ 1GB
4. If `strategy: "queue"`, `queue` config must be present
5. If `ordering: "priority"`, `priority` config must be present

## Defaults

If not specified:

```json
{
  "strategy": "reject",
  "queue": {
    "max_depth": 100,
    "timeout": "5s",
    "ordering": "fifo",
    "memory_limit": "100MB",
    "overflow_strategy": "drop_newest"
  }
}
```

## Testing Strategy

**Unit Tests**:

- Queue enqueue/dequeue correctness
- Timeout expiration handling
- Memory limit enforcement
- FIFO ordering

**Integration Tests**:

- Queueing under concurrency limit
- Timeout cascade scenarios
- Queue overflow behavior
- Permit release triggers queue consumer

**Load Tests**:

- Sustain queue at max_depth
- Verify no memory leaks
- Measure queueing latency overhead
- Concurrent enqueue/dequeue

## Security Considerations

### Queue Exhaustion Attack

**Attack**: Malicious client floods OAGW to fill queues.

**Mitigations**:

1. Per-tenant rate limiting (limits requests before queueing)
2. Memory limits (prevents unbounded queue growth)
3. Authentication required (prevents anonymous flooding)
4. Monitor `oagw_queue_depth` for anomalies

### Priority Abuse

**Attack**: Client claims high priority for all requests.

**Mitigations**:

1. `allow_client_override: false` by default
2. Priority assigned by authentication context (tenant tier)
3. Audit high-priority requests

## Implementation Phases

**Phase 1: Basic Queueing**

- `reject` and `queue` strategies
- FIFO ordering
- Timeout handling
- Max depth enforcement
- Metrics

**Phase 2: Memory Management**

- Memory tracking and limits
- Large request handling
- Overflow strategies

**Phase 3: Priority Queueing** (Future)

- Priority-based ordering
- Client priority override (optional)
- Priority fairness algorithms

**Phase 4: Degradation** (Future)

- `degrade` strategy
- Fallback upstream routing
- Cached response fallback

## Decision

**Accepted**: Implement backpressure with `reject` and `queue` strategies (Phase 1-2).

**Rationale**:

- Provides graceful degradation during traffic spikes
- Improves user experience (fewer hard errors)
- Bounded resource usage (queue limits)
- Clear client signaling (Retry-After)
- Complements concurrency control

**Deferred**: Priority queueing and degradation (Phase 3-4) until demonstrated need.

## Consequences

**Positive**:

- Smoother traffic handling during bursts
- Better UX (fewer immediate rejections)
- Automatic retry handling via queueing
- Observable queue behavior

**Negative**:

- Memory overhead for queues
- Increased latency (queueing delay)
- Complexity in queue management
- Risk of timeout cascades (client timeout < queue timeout)

**Mitigations**:

- Memory limits prevent unbounded growth
- Queue timeout < typical client timeout (e.g., 5s queue vs 30s client)
- Monitor queue metrics to detect issues
- Default to `reject` strategy (opt-in to queueing)

## References

- [ADR: Concurrency Control](./adr-concurrency-control.md) - In-flight limits
- [ADR: Rate Limiting](./adr-rate-limiting.md) - Time-based rate control
- [ADR: Circuit Breaker](./adr-circuit-breaker.md) - Upstream health protection
- [Envoy Circuit Breaking](https://www.envoyproxy.io/docs/envoy/latest/intro/arch_overview/upstream/circuit_breaking)
- [AWS Lambda Throttling](https://docs.aws.amazon.com/lambda/latest/dg/invocation-async.html#invocation-async-throttling)
- [Google Cloud Tasks Retry](https://cloud.google.com/tasks/docs/creating-http-target-tasks#retry)
