# Plugin Execution Flow

## Overview

Plugins execute in Data Plane during proxy request processing. Three plugin types execute in strict order: Auth → Guards → Transform → Proxy Call → Transform.

## Plugin Types

### AuthPlugin
- **Purpose**: Inject authentication credentials
- **Execution**: Once per request, before guards
- **Can terminate**: No (always succeeds or errors)
- **Examples**: API key, OAuth2, Bearer token

### GuardPlugin
- **Purpose**: Validate and enforce policies
- **Execution**: After auth, before transform
- **Can terminate**: Yes (reject request with error)
- **Examples**: Timeout, CORS, rate limiting

### TransformPlugin
- **Purpose**: Modify request/response/error data
- **Execution**: Before and after proxy call
- **Can terminate**: No (mutations only)
- **Examples**: Logging, metrics, request ID

## Execution Order

```
Incoming Request
  ↓
1. Auth Plugin (credential injection)
  ↓
2. Guard Plugins (validation, can reject)
  ↓
3. Transform Plugins (modify request)
  ↓
4. HTTP call to external service
  ↓
5. Transform Plugins (modify response)
  ↓
Response to Client
```

## Example: Full Plugin Chain

### Configuration

**Upstream config:**
```json
{
  "alias": "openai",
  "auth": {
    "plugin": "gts.x.core.oagw.plugin.auth.v1~x.core.bearer_token.v1",
    "config": { "secret_ref": "gts.x.core.cred.v1~abc123..." }
  },
  "plugins": {
    "guards": [
      "gts.x.core.oagw.plugin.guard.v1~x.core.timeout.v1"
    ],
    "transforms": [
      "gts.x.core.oagw.plugin.transform.v1~x.core.request_id.v1",
      "gts.x.core.oagw.plugin.transform.v1~x.core.logging.v1"
    ]
  }
}
```

**Route config:**
```json
{
  "match": {
    "http": {
      "methods": ["POST"],
      "path": "/v1/chat/completions"
    }
  },
  "plugins": {
    "guards": [
      "gts.x.core.oagw.plugin.guard.v1~x.core.cors.v1"
    ]
  },
  "rate_limit": {
    "sustained": { "rate": 100, "window": "minute" }
  }
}
```

### Request

```http
POST /api/oagw/v1/proxy/openai/v1/chat/completions HTTP/1.1
Host: oagw.example.com
Authorization: Bearer <tenant-token>
Origin: https://example.com
Content-Type: application/json

{"model": "gpt-4", "messages": [{"role": "user", "content": "Hello"}]}
```

### Execution Steps

#### Phase 1: Auth Plugin

**Plugin**: BearerTokenAuthPlugin
**Input**: Request context with no upstream auth

**Actions**:
1. Retrieve secret from `cred_store` using `secret_ref`
2. Extract API key: `sk-proj-abc123...`
3. Inject into request headers

**Output**: Request context updated
```
Headers:
  + Authorization: Bearer sk-proj-abc123...
```

#### Phase 2: Guard Plugins (Merged from Upstream + Route)

Execution order: upstream guards, then route guards.

**Plugin 1**: TimeoutGuardPlugin (from upstream)
**Input**: Request context

**Actions**:
1. Check configured timeout: `30s`
2. Check elapsed time since request start: `50ms`
3. Remaining budget: `29.95s` → OK

**Output**: `GuardDecision::Allow`

---

**Plugin 2**: CorsGuardPlugin (from route)
**Input**: Request context

**Actions**:
1. Check `Origin` header: `https://example.com`
2. Validate against allowed origins
3. Not a preflight (not OPTIONS) → skip preflight logic

**Output**: `GuardDecision::Allow`

---

**Plugin 3**: RateLimitGuardPlugin (from route rate_limit config)
**Input**: Request context

**Actions**:
1. Check tenant rate limit quota
2. Current: 87 requests in window
3. Limit: 100 requests/minute
4. Remaining: 13 → Allow
5. Consume 1 token

**Output**: `GuardDecision::Allow`
**Side effect**: Rate limit counter incremented

#### Phase 3: Transform Plugins (Pre-Request)

**Plugin 1**: RequestIdTransformPlugin
**Input**: Request context

**Actions**:
1. Generate request ID: `req_abc123...`
2. Add to headers
3. Store in context for logging

**Output**: Request context updated
```
Headers:
  + X-Request-ID: req_abc123...
```

---

**Plugin 2**: LoggingTransformPlugin
**Input**: Request context

**Actions**:
1. Log request start
2. Record: method, path, tenant_id, request_id

**Output**: No mutation (logging side effect only)

**Log entry**:
```json
{
  "timestamp": "2026-02-09T12:00:00.123Z",
  "level": "info",
  "msg": "proxy_request_start",
  "tenant_id": "...",
  "request_id": "req_abc123...",
  "method": "POST",
  "path": "/v1/chat/completions",
  "upstream_alias": "openai"
}
```

#### Phase 4: HTTP Call to Upstream

**Outbound request**:
```http
POST /v1/chat/completions HTTP/1.1
Host: api.openai.com
Authorization: Bearer sk-proj-abc123...
X-Request-ID: req_abc123...
Content-Type: application/json

{"model": "gpt-4", "messages": [{"role": "user", "content": "Hello"}]}
```

**Upstream response**:
```http
HTTP/1.1 200 OK
Content-Type: application/json
X-Request-ID: req_abc123...

{"id": "chatcmpl-...", "choices": [...]}
```

#### Phase 5: Transform Plugins (Post-Response)

**Plugin 1**: RequestIdTransformPlugin
**Input**: Response context

**Actions**:
1. Verify request ID propagated: ✓
2. No additional mutation needed

**Output**: No change

---

**Plugin 2**: LoggingTransformPlugin
**Input**: Response context

**Actions**:
1. Log request completion
2. Record: status, duration, response size

**Output**: No mutation

**Log entry**:
```json
{
  "timestamp": "2026-02-09T12:00:01.456Z",
  "level": "info",
  "msg": "proxy_request_complete",
  "tenant_id": "...",
  "request_id": "req_abc123...",
  "status": 200,
  "duration_ms": 1333,
  "response_bytes": 2048,
  "upstream_alias": "openai"
}
```

### Final Response

```http
HTTP/1.1 200 OK
Content-Type: application/json
X-Request-ID: req_abc123...

{"id": "chatcmpl-...", "choices": [...]}
```

## Guard Plugin Rejection Example

### Scenario: Timeout Exceeded

**Plugin**: TimeoutGuardPlugin
**Input**: Request context where `elapsed_time > timeout`

**Actions**:
1. Check configured timeout: `30s`
2. Check elapsed time: `31.2s`
3. Budget exceeded → Reject

**Output**: `GuardDecision::Reject`

**Result**: Request terminates immediately, no upstream call

**Error Response**:
```http
HTTP/1.1 408 Request Timeout
X-OAGW-Error-Source: gateway
Content-Type: application/problem+json

{
  "type": "gts.x.core.errors.err.v1~x.oagw.guard.timeout.v1",
  "title": "Request Timeout",
  "status": 408,
  "detail": "Request timeout budget exceeded (30s)",
  "instance": "/api/oagw/v1/proxy/openai/v1/chat/completions",
  "timeout_seconds": 30,
  "elapsed_seconds": 31.2
}
```

**Plugin chain stops**: Transform plugins (pre-request) never execute.

### Scenario: Rate Limit Exceeded

**Plugin**: RateLimitGuardPlugin
**Input**: Request context

**Actions**:
1. Check rate limit: 100 requests/minute
2. Current: 100 requests in window
3. Limit exceeded → Reject

**Output**: `GuardDecision::Reject`

**Error Response**:
```http
HTTP/1.1 429 Too Many Requests
X-OAGW-Error-Source: gateway
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 0
X-RateLimit-Reset: 1706889660
Retry-After: 15
Content-Type: application/problem+json

{
  "type": "gts.x.core.errors.err.v1~x.oagw.guard.rate_limit.v1",
  "title": "Rate Limit Exceeded",
  "status": 429,
  "detail": "Rate limit exceeded: 100 requests per minute",
  "instance": "/api/oagw/v1/proxy/openai/v1/chat/completions",
  "retry_after_seconds": 15
}
```

## Transform Plugin Error Handling

Transform plugins operate on error responses too:

```
Upstream returns 500 error
  ↓
Transform plugins (post-error) execute
  ↓
  - LoggingTransformPlugin: logs error
  - RequestIdTransformPlugin: preserves request ID
  ↓
Error response returned to client
```

### Example: Upstream Error

**Upstream response**:
```http
HTTP/1.1 500 Internal Server Error
Content-Type: application/json

{"error": {"message": "Internal server error"}}
```

**Transform execution**:
```
LoggingTransformPlugin:
  → Log error response

RequestIdTransformPlugin:
  → Ensure X-Request-ID present in response
```

**Final response**:
```http
HTTP/1.1 500 Internal Server Error
X-OAGW-Error-Source: upstream
X-Request-ID: req_abc123...
Content-Type: application/json

{"error": {"message": "Internal server error"}}
```

Note: `X-OAGW-Error-Source: upstream` indicates error from external service.

## Plugin Configuration Merging

Plugins are merged from multiple layers:

1. **Upstream** config: Base plugins
2. **Route** config: Additional plugins
3. **Tenant** config: Override plugins (future)

**Merge strategy**: Union (all plugins execute), upstream first.

### Example

**Upstream plugins**:
```json
{
  "guards": ["timeout"],
  "transforms": ["request_id", "logging"]
}
```

**Route plugins**:
```json
{
  "guards": ["cors"],
  "transforms": ["metrics"]
}
```

**Effective chain**:
```
Guards: [timeout, cors]
Transforms (pre): [request_id, logging, metrics]
Transforms (post): [request_id, logging, metrics]
```

## Built-in vs External Plugins

### Built-in Plugins (Native Rust)

Included in `oagw-cp` crate:
- `ApiKeyAuthPlugin`
- `BearerTokenAuthPlugin`
- `TimeoutGuardPlugin`
- `CorsGuardPlugin`
- `RateLimitGuardPlugin`
- `LoggingTransformPlugin`
- `MetricsTransformPlugin`
- `RequestIdTransformPlugin`

Performance: Native code, zero overhead.

### External Plugins (Modkit Modules)

Separate modules implementing plugin traits from `oagw-sdk`:
- `cf-oagw-plugin-oauth2-pkce` (custom auth)
- `cf-oagw-plugin-jwt-auth` (JWT validation)
- `cf-oagw-plugin-custom-guard` (tenant-specific rules)

Registered during CP initialization via modkit.

## Performance Characteristics

| Plugin Type | Typical Latency |
|-------------|-----------------|
| Auth (cred_store lookup) | 1-2ms |
| Guard (simple validation) | <100μs |
| Guard (rate limit check) | <500μs (local) |
| Transform (header mutation) | <50μs |
| Transform (logging) | <100μs |

Total plugin overhead: ~2-3ms for typical chain.

## Related ADRs

- [ADR: Plugin System](../docs/adr-plugin-system.md)
- [ADR: State Management](../docs/adr-state-management.md)
- [ADR: Component Architecture](../docs/adr-component-architecture.md)
