# Proxy Operations Flow

## Overview

Proxy operations execute API calls to external services. Requests are routed from API Handler → Data Plane → Control Plane (config resolution) → Data Plane (execution).

## Request Flow

```
Client Request
  → API Handler (auth, rate limit)
    → Data Plane (orchestrate)
      → Control Plane (resolve upstream config)
      → Control Plane (resolve route config)
      → CP: Execute plugins (auth, guard, transform)
      → CP: HTTP call to external service
    → Response
```

## Component Responsibilities

### API Handler
- Validates Bearer token
- Applies inbound rate limiting
- Routes `/api/oagw/v1/proxy/*` requests to Data Plane

### Data Plane
- Orchestrates proxy request execution
- Checks L1 cache for hot configs
- Calls DP for config resolution (on cache miss)
- Executes plugin chain:
  1. Auth plugin (inject credentials)
  2. Guard plugins (validate, enforce policies)
  3. Transform plugins (modify request)
  4. HTTP call to external service
  5. Transform plugins (modify response)
- Returns response to client

### Control Plane
- Resolves upstream by alias (tenant hierarchy walk)
- Resolves route by (upstream_id, method, path)
- Returns config from L1 cache → L2 cache → DB
- Serves as authoritative config source

## Example: Proxy Request to OpenAI

### Request

```http
POST /api/oagw/v1/proxy/openai/v1/chat/completions HTTP/1.1
Host: oagw.example.com
Authorization: Bearer <tenant-token>
Content-Type: application/json

{
  "model": "gpt-4",
  "messages": [{"role": "user", "content": "Hello"}]
}
```

### Flow Steps

#### 1. API Handler Processing

```
API Handler receives request
  → Extract alias from path: "openai"
  → Extract path suffix: "/v1/chat/completions"
  → Validate Bearer token → 401 if invalid
  → Check rate limit → 429 if exceeded
  → Route to Data Plane
```

#### 2. Data Plane: Upstream Resolution

```
CP receives request
  → Check L1 cache: upstream:{tenant_id}:openai
    ├─ HIT: Use cached config (~1μs)
    └─ MISS: Call DP.resolve_upstream(tenant_id, "openai")
         → DP checks L1 cache (~1μs)
         → DP checks L2 cache (Redis, ~1-2ms)
         → DP queries DB (~5-10ms)
         → DP returns upstream config
         → CP caches in L1 for next request
```

Resolved upstream:
```json
{
  "id": "gts.x.core.oagw.upstream.v1~7c9e6679...",
  "alias": "openai",
  "server": {
    "endpoints": [
      { "scheme": "https", "host": "api.openai.com", "port": 443 }
    ]
  },
  "protocol": "gts.x.core.oagw.protocol.v1~x.core.http.v1",
  "auth": {
    "plugin": "gts.x.core.oagw.plugin.auth.v1~x.core.bearer_token.v1",
    "config": { "secret_ref": "gts.x.core.cred.v1~abc123..." }
  }
}
```

#### 3. Data Plane: Route Resolution

```
CP calls DP.resolve_route(upstream_id, "POST", "/v1/chat/completions")
  → Check cache (same L1/L2/DB flow)
  → Match route by method + path prefix
```

Resolved route:
```json
{
  "id": "gts.x.core.oagw.route.v1~def456...",
  "upstream_id": "gts.x.core.oagw.upstream.v1~7c9e6679...",
  "match": {
    "http": {
      "methods": ["POST"],
      "path": "/v1/chat/completions"
    }
  },
  "plugins": {
    "guards": ["gts.x.core.oagw.plugin.guard.v1~x.core.timeout.v1"]
  }
}
```

#### 4. Data Plane: Plugin Execution

```
CP builds plugin chain from merged config:
  1. Auth Plugin: Bearer token injection
  2. Guard Plugin: Timeout enforcement (30s)
  3. Transform Plugin: Request ID propagation

Execute auth plugin:
  → Retrieve secret from cred_store
  → Inject Authorization header

Execute guard plugins:
  → Check timeout budget
  → Validate request constraints

Execute transform plugins (pre-request):
  → Add X-Request-ID header
  → Add X-OAGW-Tenant-ID header (internal)
```

#### 5. Data Plane: HTTP Call

```
CP makes HTTP request to external service:
  → Target: https://api.openai.com:443/v1/chat/completions
  → Method: POST
  → Headers:
      - Authorization: Bearer sk-... (from auth plugin)
      - Content-Type: application/json
      - X-Request-ID: req_abc123... (from transform)
  → Body: (original request body)
```

#### 6. External Service Response

```
External service returns:
  → Status: 200 OK
  → Headers: Content-Type, X-Request-ID
  → Body: {"id": "chatcmpl-...", "choices": [...]}
```

#### 7. Data Plane: Response Transform

```
CP executes transform plugins (post-response):
  → Strip internal headers (X-OAGW-*)
  → Add observability headers if configured
  → Return to client
```

### Response

```http
HTTP/1.1 200 OK
Content-Type: application/json
X-Request-ID: req_abc123...

{
  "id": "chatcmpl-...",
  "object": "chat.completion",
  "created": 1706889600,
  "model": "gpt-4",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Hello! How can I help you today?"
      },
      "finish_reason": "stop"
    }
  ]
}
```

## Caching Behavior

### Cold Path (Cache Miss)
```
Request → CP L1 miss → DP L1 miss → DP L2 miss → DB query
Total latency: ~10-15ms (config resolution) + upstream call
```

### Warm Path (CP L1 Hit)
```
Request → CP L1 hit → Execute plugins → Upstream call
Config resolution: <1μs
```

### Hot Path (After Multiple Requests)
```
All config in CP L1 cache
No DP calls needed
Total overhead: <1ms (plugins + HTTP call setup)
```

## Cache Invalidation Impact

When upstream config is updated:
1. DP flushes L1/L2 immediately
2. API Handler notifies CP to flush L1
3. Next request: CP L1 miss → calls DP → gets fresh config
4. Subsequent requests: served from CP L1 again

Invalidation window: typically <100ms for CP notification.

## Error Scenarios

### Upstream Not Found

```http
HTTP/1.1 404 Not Found
X-OAGW-Error-Source: gateway
Content-Type: application/problem+json

{
  "type": "gts.x.core.errors.err.v1~x.oagw.routing.upstream_not_found.v1",
  "title": "Upstream Not Found",
  "status": 404,
  "detail": "No upstream found with alias 'openai' for tenant",
  "instance": "/api/oagw/v1/proxy/openai/v1/chat/completions"
}
```

### Upstream Disabled

```http
HTTP/1.1 503 Service Unavailable
X-OAGW-Error-Source: gateway
Content-Type: application/problem+json

{
  "type": "gts.x.core.errors.err.v1~x.oagw.routing.upstream_disabled.v1",
  "title": "Upstream Disabled",
  "status": 503,
  "detail": "Upstream 'openai' is disabled",
  "instance": "/api/oagw/v1/proxy/openai/v1/chat/completions",
  "upstream_id": "gts.x.core.oagw.upstream.v1~7c9e6679..."
}
```

### Route Not Found

```http
HTTP/1.1 404 Not Found
X-OAGW-Error-Source: gateway
Content-Type: application/problem+json

{
  "type": "gts.x.core.errors.err.v1~x.oagw.routing.route_not_found.v1",
  "title": "Route Not Found",
  "status": 404,
  "detail": "No route matches POST /v1/unknown/path",
  "instance": "/api/oagw/v1/proxy/openai/v1/unknown/path"
}
```

### Guard Plugin Rejection

```http
HTTP/1.1 408 Request Timeout
X-OAGW-Error-Source: gateway
Content-Type: application/problem+json

{
  "type": "gts.x.core.errors.err.v1~x.oagw.guard.timeout.v1",
  "title": "Request Timeout",
  "status": 408,
  "detail": "Request timeout budget exceeded (30s)",
  "instance": "/api/oagw/v1/proxy/openai/v1/chat/completions"
}
```

### Upstream Error (Passthrough)

```http
HTTP/1.1 500 Internal Server Error
X-OAGW-Error-Source: upstream
Content-Type: application/json

{
  "error": {
    "message": "Internal server error",
    "type": "server_error"
  }
}
```

Note: `X-OAGW-Error-Source: upstream` indicates error from external service, not gateway.

## Performance Characteristics

| Scenario | Config Resolution | Plugin Execution | Total Overhead |
|----------|-------------------|------------------|----------------|
| Cold path (DB query) | 10-15ms | 1-2ms | ~15-20ms |
| Warm path (L2 cache) | 1-2ms | 1-2ms | ~3-5ms |
| Hot path (L1 cache) | <1μs | 1-2ms | ~2-3ms |

External service call time is not included (varies by upstream).

## Related ADRs

- [ADR: Component Architecture](../docs/adr-component-architecture.md)
- [ADR: Request Routing](../docs/adr-request-routing.md)
- [ADR: Control Plane Caching](../docs/adr-data-plane-caching.md)
- [ADR: State Management](../docs/adr-state-management.md)
- [ADR: Plugin System](../docs/adr-plugin-system.md)
