# Feature: Request Proxying & Authentication

- [ ] `p1` - **ID**: `cpt-cf-oagw-featstatus-proxy-auth`

- [ ] `p1` - `cpt-cf-oagw-feature-proxy-auth`

## 1. Feature Context

### 1.1 Overview

End-to-end request proxying through the Data Plane: alias resolution, route matching, configuration merge, credential injection via auth plugins, request transformation, HTTP forwarding to upstream services, and response delivery back to the caller.

### 1.2 Purpose

This is the core value proposition of OAGW — a unified outbound proxy with centralized policy enforcement. Application modules send requests to a single proxy endpoint and OAGW handles upstream resolution, authentication, validation, and forwarding. Modules never manage credentials or connection details directly.

Addresses PRD requirements: `cpt-cf-oagw-fr-request-proxy`, `cpt-cf-oagw-fr-auth-injection`.

### 1.3 Actors

| Actor | Role in Feature |
|-------|-----------------|
| `cpt-cf-oagw-actor-app-developer` | Sends requests to the proxy endpoint to reach external services |
| `cpt-cf-oagw-actor-upstream-service` | External service that receives the forwarded request |
| `cpt-cf-oagw-actor-credential-store` | Provides secret material by UUID reference for credential injection |

### 1.4 References

- **PRD**: [PRD.md](../PRD.md)
- **Design**: [DESIGN.md](../DESIGN.md)
- **Requirements**: `cpt-cf-oagw-fr-request-proxy`, `cpt-cf-oagw-fr-auth-injection`
- **Design elements**: `cpt-cf-oagw-design-oagw`, `cpt-cf-oagw-adr-request-routing`, `cpt-cf-oagw-adr-component-architecture`
- **Dependencies**: `cpt-cf-oagw-feature-core-config-mgmt` (upstreams and routes must exist)

## 2. Actor Flows (CDSL)

### Proxy HTTP Request Flow

- [ ] `p1` - **ID**: `cpt-cf-oagw-flow-proxy-request`

**Actor**: `cpt-cf-oagw-actor-app-developer`

**Success Scenarios**:
- Request is proxied to the correct upstream with credentials injected
- Response from upstream is returned to the caller
- Request is logged with correlation ID for audit trail

**Error Scenarios**:
- Upstream not found by alias (404 RouteNotFound)
- Upstream is disabled (503 upstream_disabled)
- No matching route for method/path (404 RouteNotFound)
- Auth plugin fails to retrieve credentials (401 AuthenticationFailed)
- Guard plugin rejects the request (guard-specific status code)
- Rate limit exceeded (429 RateLimitExceeded)
- Upstream timeout (504 Timeout)
- Upstream error (502 DownstreamError)
- Circuit breaker open (503 CircuitBreakerOpen)

**Steps**:
1. [ ] - `p1` - Application sends {METHOD} /api/oagw/v1/proxy/{alias}[/{path_suffix}][?{query}] - `inst-pr-1`
2. [ ] - `p1` - Extract SecurityContext (tenant_id, principal_id) from bearer token - `inst-pr-2`
3. [ ] - `p1` - Parse alias and path_suffix from URI - `inst-pr-3`
4. [ ] - `p1` - Resolve upstream by alias for tenant (calls Alias Resolution algorithm) - `inst-pr-4`
5. [ ] - `p1` - **IF** upstream not found - `inst-pr-5`
   1. [ ] - `p1` - **RETURN** 404 RouteNotFound with X-OAGW-Error-Source: gateway - `inst-pr-5a`
6. [ ] - `p1` - **IF** upstream disabled - `inst-pr-6`
   1. [ ] - `p1` - **RETURN** 503 upstream_disabled with X-OAGW-Error-Source: gateway - `inst-pr-6a`
7. [ ] - `p1` - Match route by (upstream_id, method, path_suffix) using Route Matching algorithm - `inst-pr-7`
8. [ ] - `p1` - **IF** no matching route - `inst-pr-8`
   1. [ ] - `p1` - **RETURN** 404 RouteNotFound with X-OAGW-Error-Source: gateway - `inst-pr-8a`
9. [ ] - `p1` - Merge configurations: upstream (base) < route < tenant - `inst-pr-9`
10. [ ] - `p1` - Execute auth plugin: retrieve credentials from cred_store, inject into outbound request - `inst-pr-10`
11. [ ] - `p1` - **IF** auth plugin fails - `inst-pr-11`
    1. [ ] - `p1` - **RETURN** 401 AuthenticationFailed with X-OAGW-Error-Source: gateway - `inst-pr-11a`
12. [ ] - `p1` - Execute guard plugins (validation, can reject) - `inst-pr-12`
13. [ ] - `p1` - **IF** guard rejects - `inst-pr-13`
    1. [ ] - `p1` - **RETURN** guard rejection status with X-OAGW-Error-Source: gateway - `inst-pr-13a`
14. [ ] - `p1` - Execute transform plugins (on_request phase): mutate outbound request - `inst-pr-14`
15. [ ] - `p1` - Build outbound HTTP request (URL, headers, body) using Request Build algorithm - `inst-pr-15`
16. [ ] - `p1` - Forward request to upstream service via HTTP client - `inst-pr-16`
17. [ ] - `p1` - **IF** upstream responds successfully - `inst-pr-17`
    1. [ ] - `p1` - Execute transform plugins (on_response phase) - `inst-pr-17a`
    2. [ ] - `p1` - **RETURN** upstream response with X-OAGW-Error-Source: upstream (if error status) - `inst-pr-17b`
18. [ ] - `p1` - **IF** upstream connection/timeout error - `inst-pr-18`
    1. [ ] - `p1` - Execute transform plugins (on_error phase) - `inst-pr-18a`
    2. [ ] - `p1` - **RETURN** 502/504 with X-OAGW-Error-Source: gateway - `inst-pr-18b`

## 3. Processes / Business Logic (CDSL)

### Route Matching Algorithm

- [ ] `p1` - **ID**: `cpt-cf-oagw-algo-route-matching`

**Input**: upstream_id, HTTP method, request path_suffix

**Output**: Matched route or 404

**Steps**:
1. [ ] - `p1` - DB: SELECT oagw_route WHERE upstream_id = :upstream_id AND enabled = TRUE AND match->'http'->'methods' ? :method - `inst-rm-1`
2. [ ] - `p1` - Filter routes where request path starts with route's match.http.path - `inst-rm-2`
3. [ ] - `p1` - **IF** path_suffix_mode = disabled and path_suffix is non-empty, exclude route - `inst-rm-3`
4. [ ] - `p1` - Order by priority DESC, then by longest path match DESC - `inst-rm-4`
5. [ ] - `p1` - **IF** no routes match - `inst-rm-5`
   1. [ ] - `p1` - **RETURN** 404 RouteNotFound - `inst-rm-5a`
6. [ ] - `p1` - **RETURN** first (highest priority, longest path) matching route - `inst-rm-6`

### Request Build Algorithm

- [ ] `p1` - **ID**: `cpt-cf-oagw-algo-request-build`

**Input**: Inbound request, resolved upstream, matched route, merged configuration

**Output**: Outbound HTTP request ready for forwarding

**Steps**:
1. [ ] - `p1` - Select target endpoint from upstream (single endpoint or X-OAGW-Target-Host directed, or round-robin) - `inst-rb-1`
2. [ ] - `p1` - Build outbound URL: {endpoint.scheme}://{endpoint.host}:{endpoint.port}{route.match.http.path} - `inst-rb-2`
3. [ ] - `p1` - **IF** path_suffix_mode = append and path_suffix present - `inst-rb-3`
   1. [ ] - `p1` - Append path_suffix to outbound URL path - `inst-rb-3a`
4. [ ] - `p1` - Filter query parameters against route's query_allowlist; pass only allowed params - `inst-rb-4`
5. [ ] - `p1` - Apply header transformation: strip hop-by-hop headers, replace Host with upstream host - `inst-rb-5`
6. [ ] - `p1` - Apply configured header transforms from upstream.headers (set/add/remove) - `inst-rb-6`
7. [ ] - `p1` - Strip X-OAGW-Target-Host header (consumed by routing, not forwarded) - `inst-rb-7`
8. [ ] - `p1` - Passthrough request body - `inst-rb-8`
9. [ ] - `p1` - **RETURN** constructed outbound request - `inst-rb-9`

### Auth Plugin Execution Algorithm

- [ ] `p1` - **ID**: `cpt-cf-oagw-algo-auth-execution`

**Input**: Upstream auth configuration, outbound request context

**Output**: Request with credentials injected, or authentication error

**Steps**:
1. [ ] - `p1` - Read auth plugin type from upstream configuration (GTS identifier) - `inst-ae-1`
2. [ ] - `p1` - **IF** auth type is noop (`...~x.core.oagw.noop.v1`) - `inst-ae-2`
   1. [ ] - `p1` - **RETURN** request unchanged - `inst-ae-2a`
3. [ ] - `p1` - Resolve plugin: named builtin → lookup in builtin registry; UUID → lookup in plugin table - `inst-ae-3`
4. [ ] - `p1` - **IF** plugin not found - `inst-ae-4`
   1. [ ] - `p1` - **RETURN** 503 PluginNotFound - `inst-ae-4a`
5. [ ] - `p1` - Retrieve secret material from cred_store by secret_ref UUID - `inst-ae-5`
6. [ ] - `p1` - **IF** secret not found or not accessible to tenant - `inst-ae-6`
   1. [ ] - `p1` - **RETURN** 500 SecretNotFound (no credential details in error) - `inst-ae-6a`
7. [ ] - `p1` - Execute auth plugin with secret material and request context - `inst-ae-7`
8. [ ] - `p1` - **IF** apikey plugin: inject secret into configured header with optional prefix - `inst-ae-8`
9. [ ] - `p1` - **IF** basic plugin: encode username:password as Base64, set Authorization: Basic header - `inst-ae-9`
10. [ ] - `p1` - **IF** bearer plugin: set Authorization: Bearer {token} header - `inst-ae-10`
11. [ ] - `p1` - **IF** oauth2_client_cred plugin: exchange client_id/secret for access token, cache token, set Authorization: Bearer header - `inst-ae-11`
12. [ ] - `p1` - **RETURN** request with credentials injected - `inst-ae-12`

### Input Validation Algorithm

- [ ] `p1` - **ID**: `cpt-cf-oagw-algo-input-validation`

**Input**: Inbound proxy request, matched route configuration

**Output**: Validation pass or 400/413 error

**Steps**:
1. [ ] - `p1` - Validate HTTP method is in route's match.http.methods allowlist - `inst-iv-1`
2. [ ] - `p1` - Validate query parameters against route's match.http.query_allowlist; reject unknown keys - `inst-iv-2`
3. [ ] - `p1` - **IF** path_suffix_mode = disabled and path_suffix present - `inst-iv-3`
   1. [ ] - `p1` - **RETURN** 400 ValidationError - `inst-iv-3a`
4. [ ] - `p1` - **IF** Content-Length header present, validate it is a valid integer - `inst-iv-4`
5. [ ] - `p1` - **IF** body size exceeds 100MB hard limit - `inst-iv-5`
   1. [ ] - `p1` - **RETURN** 413 PayloadTooLarge (reject before buffering) - `inst-iv-5a`
6. [ ] - `p1` - **IF** Transfer-Encoding present and not `chunked` - `inst-iv-6`
   1. [ ] - `p1` - **RETURN** 400 ValidationError - `inst-iv-6a`
7. [ ] - `p1` - Validate no ambiguous Content-Length / Transfer-Encoding combinations - `inst-iv-7`
8. [ ] - `p1` - **RETURN** validation pass - `inst-iv-8`

### Error Source Classification Algorithm

- [ ] `p1` - **ID**: `cpt-cf-oagw-algo-error-source`

**Input**: Error response context (gateway-generated or upstream passthrough)

**Output**: Response with X-OAGW-Error-Source header

**Steps**:
1. [ ] - `p1` - **IF** error originated within OAGW (validation, auth, rate limit, circuit breaker, timeout) - `inst-es-1`
   1. [ ] - `p1` - Set X-OAGW-Error-Source: gateway - `inst-es-1a`
   2. [ ] - `p1` - Format response as RFC 9457 Problem Details JSON - `inst-es-1b`
2. [ ] - `p1` - **IF** error is upstream response (status >= 400 from upstream) - `inst-es-2`
   1. [ ] - `p1` - Set X-OAGW-Error-Source: upstream - `inst-es-2a`
   2. [ ] - `p1` - Passthrough upstream response body and headers as-is - `inst-es-2b`
3. [ ] - `p1` - **RETURN** classified error response - `inst-es-3`

## 4. States (CDSL)

Not applicable. Request proxying is stateless per-request processing. Stateful concerns (circuit breaker, rate limiter) are covered in the Traffic Resilience feature.

## 5. Definitions of Done

### Implement Proxy Endpoint

- [ ] `p1` - **ID**: `cpt-cf-oagw-dod-proxy-endpoint`

The system **MUST** implement `{METHOD} /api/oagw/v1/proxy/{alias}[/{path_suffix}][?{query}]` that resolves upstream by alias, matches route, merges configuration, executes plugin chain, and forwards the request to the upstream service. No automatic retries; each inbound request results in at most one upstream attempt.

**Implements**:
- `cpt-cf-oagw-flow-proxy-request`
- `cpt-cf-oagw-algo-route-matching`
- `cpt-cf-oagw-algo-request-build`

**Touches**:
- API: `{METHOD} /api/oagw/v1/proxy/{alias}[/{path_suffix}]`
- DB: `oagw_upstream`, `oagw_route`
- Entities: `DataPlaneService`, `ControlPlaneService`

### Implement Auth Plugin Execution

- [ ] `p1` - **ID**: `cpt-cf-oagw-dod-auth-execution`

The system **MUST** execute the configured auth plugin for each proxy request, retrieving credentials from `cred_store` by UUID reference at request time. Built-in auth plugins **MUST** include: noop, apikey, basic, bearer, oauth2_client_cred, oauth2_client_cred_basic. Credentials **MUST** never appear in logs, error messages, or client responses.

**Implements**:
- `cpt-cf-oagw-algo-auth-execution`

**Touches**:
- Entities: `AuthPluginRegistry`, `ApiKeyAuthPlugin`, `BasicAuthPlugin`, `BearerTokenAuthPlugin`, `OAuth2ClientCredPlugin`

### Implement Input Validation

- [ ] `p1` - **ID**: `cpt-cf-oagw-dod-input-validation`

The system **MUST** validate method, path, query parameters, headers, and body size on all proxy requests before forwarding to upstream. Invalid requests **MUST** be rejected with 400 Bad Request or 413 Payload Too Large. Body size hard limit is 100MB.

**Implements**:
- `cpt-cf-oagw-algo-input-validation`

**Touches**:
- API: `{METHOD} /api/oagw/v1/proxy/{alias}/*`
- Entities: `DataPlaneService`

### Implement Error Source Distinction

- [ ] `p1` - **ID**: `cpt-cf-oagw-dod-error-source`

The system **MUST** distinguish gateway errors from upstream errors using the `X-OAGW-Error-Source` header (`gateway` or `upstream`). Gateway errors **MUST** use RFC 9457 Problem Details JSON format with stable GTS `type` identifiers. Upstream errors **MUST** be passed through without body modification.

**Implements**:
- `cpt-cf-oagw-algo-error-source`

**Touches**:
- API: all proxy error responses
- Entities: `DataPlaneService`

## 6. Acceptance Criteria

- [ ] Proxy request to a valid alias/route returns the upstream response to the caller
- [ ] Proxy request to non-existent alias returns 404 with X-OAGW-Error-Source: gateway
- [ ] Proxy request to disabled upstream returns 503 with X-OAGW-Error-Source: gateway
- [ ] Proxy request with no matching route returns 404 with X-OAGW-Error-Source: gateway
- [ ] Auth plugin injects credentials into outbound request (API key in header verified)
- [ ] Missing or inaccessible secret_ref returns error without exposing credential details
- [ ] Query parameters not in allowlist are rejected with 400
- [ ] Body exceeding 100MB is rejected with 413 before buffering
- [ ] Invalid Transfer-Encoding is rejected with 400
- [ ] Route matching selects highest priority, longest path prefix match
- [ ] path_suffix_mode=append appends suffix to outbound path
- [ ] path_suffix_mode=disabled rejects requests with path suffix
- [ ] Hop-by-hop headers are stripped from outbound request
- [ ] Host header is replaced with upstream host
- [ ] X-OAGW-Target-Host is consumed and stripped (not forwarded)
- [ ] Upstream error responses pass through with X-OAGW-Error-Source: upstream
- [ ] Gateway errors use RFC 9457 Problem Details format
- [ ] No automatic retries on upstream failure
- [ ] Credentials never appear in logs or error responses

## 7. Not Applicable Sections

**States (CDSL)**: Not applicable because request proxying is stateless per-request processing. Stateful concerns (circuit breaker, rate limiter) are covered in the Traffic Resilience feature.

**Compliance (COMPL)**: Not applicable because OAGW proxies requests without inspecting or storing request/response bodies. PII handling is the responsibility of calling modules and upstream services.

**User-Facing Architecture (UX)**: Not applicable because this is a backend proxy API with no user-facing frontend.
