# OAGW Scenario Guide

Practical companion to [DESIGN.md](../DESIGN.md). Organized in two sections:

1. **How To** — what you CAN do with OAGW, organized by integration journey then by feature
2. **Guardrails** — what is rejected, guarded, or explicitly unsupported

Rules for all scenarios:
- Use Management API to create upstreams/routes/plugins unless scenario explicitly tests invalid config.
- Use Proxy API to invoke.
- Assert both:
  - Response semantics (status, headers, body/stream behavior)
  - Side effects (metrics/audit logs, stored config, plugin lifecycle)

Legend (used in checks):
- `ESrc`: `X-OAGW-Error-Source` header.
- `PD`: RFC 9457 Problem Details (`application/problem+json`).

---

## 1. How To (Common Integration Scenarios)

> **Flow reference**: [Proxy Operations](flows/proxy-operations.md)

### Integration journey

To proxy requests to an external service through OAGW, follow these steps in order. Each step links to the feature section with full scenario coverage.

#### Step 1: Register the upstream

Create an upstream pointing to your external service.

- [Create minimal HTTP upstream](management-api/upstreams/positive-2.1-create-minimal-http-upstream.md) — `POST /upstreams` with host, scheme, port. Auto-generates alias from hostname.
- For non-standard ports: [Alias auto-generation](management-api/upstreams/positive-2.2-alias-auto-generation-non-standard-port.md)
- For multiple endpoints: [Multi-endpoint load balancing](management-api/upstreams/positive-2.10-multi-endpoint-load-balancing-distributes-requests.md)
- See → [Upstream management](#upstream-management)

#### Step 2: Define routes

Create routes that map inbound method + path to outbound upstream calls.

- [Create HTTP route with method + path](management-api/routes/positive-3.1-create-http-route-method-path.md) — `POST /routes` with methods, path, upstream_id
- Configure path suffix mode: [append](management-api/routes/positive-3.5-path-suffix-mode-append.md) for REST prefixes
- For gRPC: [Create gRPC route by service+method](management-api/routes/positive-3.8-create-grpc-route-service-method.md)
- See → [Route configuration](#route-configuration)

#### Step 3: Attach authentication

Configure how OAGW authenticates to the external service.

- [API key injection](proxy-api/authentication/positive-9.2-api-key-injection.md) — header-based key injection from `cred_store`
- [OAuth2 client credentials](proxy-api/authentication/positive-9.5-oauth2-client-credentials.md) — automatic token fetch + cache + refresh
- [Noop (public APIs)](proxy-api/authentication/positive-9.1-noop-auth-plugin-forwards-credential-injection.md) — no credential injection
- See → [Injecting authentication](#injecting-authentication)

#### Step 4: Invoke via proxy

Call your external service through OAGW's proxy endpoint: `{METHOD} /api/oagw/v1/proxy/{alias}/{path_suffix}`

- [Plain HTTP passthrough](protocols/http/positive-12.1-plain-http-request-response-passthrough.md)
- [SSE streaming](protocols/sse/positive-13.1-sse-stream-forwarded-buffering.md)
- [WebSocket upgrade](protocols/websocket/positive-14.1-websocket-upgrade-proxied.md)
- [gRPC unary](protocols/grpc/positive-15.1-grpc-unary-request-proxied.md)
- See → [E2E protocol examples](#e2e-protocol-examples)

#### Optional: Rate limits, plugins, transforms

- [Token bucket rate limiting](rate-limiting/positive-18.1-token-bucket-sustained-burst.md) — see [Rate limiting](#rate-limiting)
- [Request path rewrite](plugins/transforms/positive-11.1-request-path-rewrite-transform.md) — see [Transform plugins](#transform-plugins)
- [Custom Starlark guard](plugins/guards/positive-10.2-built-cors-handling.md) — see [Guard plugins](#guard-plugins)
- [Tags for discovery](management-api/upstreams/positive-2.9-tags-support-discovery-filtering.md) — see [Upstream management](#upstream-management)

---

### Upstream management

> **Flow references**: [Management Operations](flows/management-operations.md) | [Cache Invalidation](flows/cache-invalidation.md)

#### Create a minimal HTTP upstream (single endpoint)
- **Scenario**: [positive-2.1-create-minimal-http-upstream.md](management-api/upstreams/positive-2.1-create-minimal-http-upstream.md)
- **Mechanism**: `POST /upstreams` with server endpoints and protocol. Returns `201` with auto-generated alias and `enabled=true` default. Alias follows hostname rules for standard ports.

#### Alias auto-generation for non-standard port
- **Scenario**: [positive-2.2-alias-auto-generation-non-standard-port.md](management-api/upstreams/positive-2.2-alias-auto-generation-non-standard-port.md)
- **Mechanism**: Endpoint `host=api.example.com, port=8443` yields alias `api.example.com:8443`. Prevents collisions and ambiguous routing.

#### Update upstream configuration
- **Scenario**: [positive-2.5-update-upstream.md](management-api/upstreams/positive-2.5-update-upstream.md)
- **Mechanism**: `PUT /upstreams/{id}` updates mutable fields. Versioning/immutability rules respected. Triggers cache invalidation across CP/DP layers.

#### Delete upstream with route cascade
- **Scenario**: [positive-2.7-delete-upstream-cascades-routes.md](management-api/upstreams/positive-2.7-delete-upstream-cascades-routes.md)
- **Mechanism**: Deleting upstream removes dependent routes (or rejects with clear error if cascade not implemented). Guardrail: subsequent proxy requests return `404 UPSTREAM_NOT_FOUND` or `ROUTE_NOT_FOUND`.

#### Tags support discovery and filtering
- **Scenario**: [positive-2.9-tags-support-discovery-filtering.md](management-api/upstreams/positive-2.9-tags-support-discovery-filtering.md)
- **Mechanism**: Create upstream with `tags` array. Listing/filtering returns expected upstreams by tag.

#### Multi-endpoint load balancing distributes requests
- **Scenario**: [positive-2.10-multi-endpoint-load-balancing-distributes-requests.md](management-api/upstreams/positive-2.10-multi-endpoint-load-balancing-distributes-requests.md)
- **Mechanism**: Multiple endpoints in one upstream form a pool. Requests distributed round-robin. All endpoints must share same protocol/scheme/port.

#### Re-enable upstream restores proxy traffic
- **Scenario**: [positive-2.11-re-enable-upstream-restores-proxy-traffic.md](management-api/upstreams/positive-2.11-re-enable-upstream-restores-proxy-traffic.md)
- **Mechanism**: `PUT /upstreams/{id}` with `enabled=true` after maintenance. Subsequent proxy requests succeed.

#### List upstreams includes disabled resources
- **Scenario**: [positive-2.12-list-upstreams-includes-disabled-resources.md](management-api/upstreams/positive-2.12-list-upstreams-includes-disabled-resources.md)
- **Mechanism**: `GET /upstreams` returns both enabled and disabled upstreams with `enabled` field. Optional `$filter=enabled eq true` for active only.

---

### Route configuration

#### Create HTTP route with method + path
- **Scenario**: [positive-3.1-create-http-route-method-path.md](management-api/routes/positive-3.1-create-http-route-method-path.md)
- **Mechanism**: `POST /routes` with `match.http.methods` and `match.http.path`. Returns `201`. Route is associated to the target upstream.

#### Path suffix mode = append (prefix routes for REST resources)
- **Scenario**: [positive-3.5-path-suffix-mode-append.md](management-api/routes/positive-3.5-path-suffix-mode-append.md)
- **Mechanism**: Suffix appended to outbound path. Outbound path normalized (no `//` surprises).

#### Route priority resolves ambiguities
- **Scenario**: [positive-3.6-route-priority-resolves-ambiguities.md](management-api/routes/positive-3.6-route-priority-resolves-ambiguities.md)
- **Mechanism**: With two candidate routes, higher `priority` wins. Deterministic selection.

#### Create gRPC route by service+method
- **Scenario**: [positive-3.8-create-grpc-route-service-method.md](management-api/routes/positive-3.8-create-grpc-route-service-method.md)
- **Mechanism**: `match.grpc.service` + `match.grpc.method` routes to HTTP/2 `:path` `/Service/Method`.

#### Re-enable route restores proxy traffic
- **Scenario**: [positive-3.9-re-enable-route-restores-proxy-traffic.md](management-api/routes/positive-3.9-re-enable-route-restores-proxy-traffic.md)
- **Mechanism**: Route with `enabled=false` is skipped in matching. `PUT /routes/{id}` with `enabled=true` restores matching.

#### List routes includes disabled resources
- **Scenario**: [positive-3.10-list-routes-includes-disabled-resources.md](management-api/routes/positive-3.10-list-routes-includes-disabled-resources.md)
- **Mechanism**: `GET /routes` returns both enabled and disabled routes with `enabled` field.

---

### Plugin management

#### Create custom Starlark guard plugin
- **Scenario**: [positive-4.1-create-custom-starlark-guard-plugin.md](management-api/plugins/positive-4.1-create-custom-starlark-guard-plugin.md)
- **Mechanism**: `POST /plugins` returns `201`. Plugin addressable by anonymous GTS id. Source retrievable via `GET /plugins/{id}/source`.

#### Delete plugin succeeds only when unreferenced
- **Scenario**: [positive-4.3-delete-plugin-succeeds-only-unreferenced.md](management-api/plugins/positive-4.3-delete-plugin-succeeds-only-unreferenced.md)
- **Mechanism**: Referenced plugin deletion returns `409 plugin.in_use`. Unlinked plugin deletion returns `204`.

#### Plugin resolution supports builtin named ids and custom UUID ids
- **Scenario**: [positive-4.5-plugin-resolution-supports-builtin-named-ids-custom-uuid.md](management-api/plugins/positive-4.5-plugin-resolution-supports-builtin-named-ids-custom-uuid.md)
- **Mechanism**: Builtin plugin id works without DB row. UUID plugin id requires DB row; missing yields `503 PLUGIN_NOT_FOUND`.

#### Plugin sharing modes merge correctly across tenant hierarchy
- **Scenario**: [positive-4.6-plugin-sharing-modes-merge-correctly-across-tenant-hierarchy.md](management-api/plugins/positive-4.6-plugin-sharing-modes-merge-correctly-across-tenant-hierarchy.md)
- **Mechanism**: `sharing=inherit` → chain is parent + child. `sharing=enforce` → child cannot remove parent plugins. `sharing=private` → child does not see parent plugins.

#### Plugin usage tracking and GC eligibility
- **Scenario**: [positive-4.7-plugin-usage-tracking-gc-eligibility-behavior.md](management-api/plugins/positive-4.7-plugin-usage-tracking-gc-eligibility-behavior.md)
- **Mechanism**: `last_used_at` updated on proxy invoke. Unlinked plugins get `gc_eligible_at` set. GC job deletes when `gc_eligible_at < now`.

---

### Injecting authentication

#### Noop auth plugin (public upstreams)
- **Scenario**: [positive-9.1-noop-auth-plugin-forwards-credential-injection.md](proxy-api/authentication/positive-9.1-noop-auth-plugin-forwards-credential-injection.md)
- **Mechanism**: No auth headers added. Use for public APIs.

#### API key injection (header + optional prefix)
- **Scenario**: [positive-9.2-api-key-injection.md](proxy-api/authentication/positive-9.2-api-key-injection.md)
- **Mechanism**: Configure `auth.type` = apikey with header name, prefix, and `secret_ref`. Secret retrieved from `cred_store` at request time. Common for OpenAI, Anthropic, etc.

#### Basic auth injection
- **Scenario**: [positive-9.3-basic-auth-injection.md](proxy-api/authentication/positive-9.3-basic-auth-injection.md)
- **Mechanism**: Correct `Authorization: Basic ...` formatting from secret.

#### Bearer token passthrough/injection
- **Scenario**: [positive-9.4-bearer-token-passthrough-injection.md](proxy-api/authentication/positive-9.4-bearer-token-passthrough-injection.md)
- **Mechanism**: `Authorization: Bearer ...` set from static secret. For service tokens.

#### OAuth2 client credentials (body-based)
- **Scenario**: [positive-9.5-oauth2-client-credentials.md](proxy-api/authentication/positive-9.5-oauth2-client-credentials.md)
- **Mechanism**: Token fetched via OAuth2 flow, cached. On upstream `401`, plugin refreshes token. Plugin does not retry the original request.

#### OAuth2 client credentials (basic-auth variant)
- **Scenario**: [positive-9.6-oauth2-client-credentials.md](proxy-api/authentication/positive-9.6-oauth2-client-credentials.md)
- **Mechanism**: Some token endpoints require `client_id/client_secret` via basic auth. Token request uses correct client authentication.

#### Hierarchical auth sharing modes
- **Scenario**: [positive-9.8-hierarchical-auth-sharing-modes-behave-specified.md](proxy-api/authentication/positive-9.8-hierarchical-auth-sharing-modes-behave-specified.md)
- **Mechanism**: `auth.sharing=inherit` + child provides auth → child override. `inherit` + child omits → parent auth. `enforce` → child cannot override. `private` → child must provide own.

---

### Request transforms

#### Inbound → outbound path + query mapping for HTTP
- **Scenario**: [positive-7.1-inbound-outbound-path-query-mapping-http.md](proxy-api/request-transforms/positive-7.1-inbound-outbound-path-query-mapping-http.md)
- **Mechanism**: Outbound path = `route.match.http.path` (+ suffix if enabled). Only allowlisted query params forwarded.

#### Hop-by-hop headers stripped
- **Scenario**: [positive-7.2-hop-hop-headers-stripped.md](proxy-api/request-transforms/positive-7.2-hop-hop-headers-stripped.md)
- **Mechanism**: `Connection`, `Upgrade`, `Transfer-Encoding`, `TE`, etc. do not reach upstream unless explicitly allowed by protocol handler.

#### Host header replaced by upstream host
- **Scenario**: [positive-7.3-host-header-replaced-upstream-host.md](proxy-api/request-transforms/positive-7.3-host-header-replaced-upstream-host.md)
- **Mechanism**: Upstream sees correct `Host` header. Prevents host header injection.

#### Upstream headers config applies simple transformations
- **Scenario**: [positive-7.5-upstream-headers-config-applies-simple-transformations.md](proxy-api/request-transforms/positive-7.5-upstream-headers-config-applies-simple-transformations.md)
- **Mechanism**: `upstream.headers.request.set` adds/overwrites headers. Header removal rules apply. Invalid header names/values rejected with `400 PD`.

#### Request correlation headers propagate end-to-end
- **Scenario**: [positive-7.6-request-correlation-headers-propagate-end-end.md](proxy-api/request-transforms/positive-7.6-request-correlation-headers-propagate-end-end.md)
- **Mechanism**: Client `X-Request-ID` forwarded to upstream and included in response. If absent, gateway generates one.

---

### Alias resolution and shadowing

#### Alias resolves by walking tenant hierarchy (shadowing)
- **Scenario**: [positive-6.1-alias-resolves-walking-tenant-hierarchy.md](proxy-api/alias-resolution/positive-6.1-alias-resolves-walking-tenant-hierarchy.md)
- **Mechanism**: Child upstream with same alias overrides parent for routing. Closest tenant wins.

#### Upstream sharing via ancestor hierarchy
- **Scenario**: [positive-5.3-upstream-sharing-ancestor-hierarchy.md](proxy-api/authz/positive-5.3-upstream-sharing-ancestor-hierarchy.md)
- **Mechanism**: Ancestor upstream visible to descendant when sharing mode permits. Descendant cannot see ancestor `private` configuration.

---

### Custom header routing (X-OAGW-Target-Host)

#### Single-endpoint upstream without X-OAGW-Target-Host header
- **Scenario**: [positive-1.1-single-endpoint-no-header.md](proxy-api/custom-header-routing/positive-1.1-single-endpoint-no-header.md)
- **Mechanism**: Single-endpoint upstreams route successfully without the custom header. Backward compatibility preserved.

#### Single-endpoint upstream with valid X-OAGW-Target-Host header
- **Scenario**: [positive-1.2-single-endpoint-with-header.md](proxy-api/custom-header-routing/positive-1.2-single-endpoint-with-header.md)
- **Mechanism**: Header value must match the single endpoint. Header stripped before forwarding.

#### Multi-endpoint explicit alias without header uses round-robin
- **Scenario**: [positive-2.1-multi-endpoint-explicit-alias-no-header.md](proxy-api/custom-header-routing/positive-2.1-multi-endpoint-explicit-alias-no-header.md)
- **Mechanism**: Requests distribute across endpoints via round-robin. No header required for explicit alias.

#### Multi-endpoint explicit alias with header bypasses load balancing
- **Scenario**: [positive-2.2-multi-endpoint-explicit-alias-with-header.md](proxy-api/custom-header-routing/positive-2.2-multi-endpoint-explicit-alias-with-header.md)
- **Mechanism**: Header value selects specific endpoint. Round-robin bypassed.

#### Multi-endpoint common suffix alias with header succeeds
- **Scenario**: [positive-3.1-multi-endpoint-common-suffix-with-header.md](proxy-api/custom-header-routing/positive-3.1-multi-endpoint-common-suffix-with-header.md)
- **Mechanism**: Header disambiguates endpoints with common suffix. Core use case for custom header routing.

#### Case-insensitive X-OAGW-Target-Host matching
- **Scenario**: [positive-3.2-case-insensitive-matching.md](proxy-api/custom-header-routing/positive-3.2-case-insensitive-matching.md)
- **Mechanism**: Mixed case header values match endpoints. DNS convention followed.

#### X-OAGW-Target-Host bypasses round-robin load balancing
- **Scenario**: [positive-3.3-load-balancing-bypass.md](proxy-api/custom-header-routing/positive-3.3-load-balancing-bypass.md)
- **Mechanism**: Header consistently routes to same endpoint. Load balancing state preserved for requests without header.

---

### Rate limiting

#### Token bucket sustained + burst
- **Scenario**: [positive-18.1-token-bucket-sustained-burst.md](rate-limiting/positive-18.1-token-bucket-sustained-burst.md)
- **Mechanism**: Burst allows short spike. Sustained rate enforced. `429` includes `Retry-After` and `X-RateLimit-*` headers.

#### Rate limit response headers can be disabled
- **Scenario**: [positive-18.1.1-rate-limit-response-headers-can-be-disabled.md](rate-limiting/positive-18.1.1-rate-limit-response-headers-can-be-disabled.md)
- **Mechanism**: With `rate_limit.response_headers=false`, success responses omit `X-RateLimit-*`. Error responses still include `Retry-After`.

#### Rate limit scope variants
- **Scenario**: [positive-18.3-rate-limit-scope-variants.md](rate-limiting/positive-18.3-rate-limit-scope-variants.md)
- **Mechanism**: One scenario each for `scope=global`, `tenant`, `user`, `ip`, `route`.

#### Weighted cost per route
- **Scenario**: [positive-18.4-weighted-cost-per-route.md](rate-limiting/positive-18.4-weighted-cost-per-route.md)
- **Mechanism**: Route with `cost=10` consumes 10 tokens. Expensive endpoints consume more budget.

#### Hierarchical min() merge for descendant overrides
- **Scenario**: [positive-18.6-hierarchical-min-merge-descendant-overrides.md](rate-limiting/positive-18.6-hierarchical-min-merge-descendant-overrides.md)
- **Mechanism**: Parent `enforce 1000/min`, child `500/min` → effective `500/min`. Parent caps child.

#### Budget modes (allocated/shared/unlimited)
- **Scenario**: [positive-18.7-budget-modes-behave-specified.md](rate-limiting/positive-18.7-budget-modes-behave-specified.md)
- **Mechanism**: `allocated` rejects when sum exceeds `total * overcommit_ratio`. `shared` pools between tenants. `unlimited` skips validation.

---

### HTTP basics

#### Plain HTTP request/response passthrough
- **Scenario**: [positive-12.1-plain-http-request-response-passthrough.md](protocols/http/positive-12.1-plain-http-request-response-passthrough.md)
- **Mechanism**: Status, headers, body forwarded. Gateway adds rate-limit headers if enabled.

#### HTTP version negotiation (HTTP/2 attempt + fallback)
- **Scenario**: [positive-12.4-http-version-negotiation.md](protocols/http/positive-12.4-http-version-negotiation.md)
- **Mechanism**: First call attempts HTTP/2 via ALPN. On failure, falls back to HTTP/1.1. Subsequent calls use cached protocol decision.

---

### Streaming (SSE)

#### SSE stream forwarded without buffering
- **Scenario**: [positive-13.1-sse-stream-forwarded-buffering.md](protocols/sse/positive-13.1-sse-stream-forwarded-buffering.md)
- **Mechanism**: Response `Content-Type: text/event-stream`. Events arrive incrementally. Rate limit headers in initial headers.

#### Client disconnect aborts upstream stream
- **Scenario**: [positive-13.2-client-disconnect-aborts-upstream-stream.md](protocols/sse/positive-13.2-client-disconnect-aborts-upstream-stream.md)
- **Mechanism**: Disconnect client mid-stream. Upstream stream closed. No leaked in-flight metrics.

---

### WebSocket

#### WebSocket upgrade is proxied
- **Scenario**: [positive-14.1-websocket-upgrade-proxied.md](protocols/websocket/positive-14.1-websocket-upgrade-proxied.md)
- **Mechanism**: `101 Switching Protocols` handshake forwarded. Required WS headers forwarded/validated.

#### Auth injected during handshake (not per-message)
- **Scenario**: [positive-14.2-auth-injected-during-handshake.md](protocols/websocket/positive-14.2-auth-injected-during-handshake.md)
- **Mechanism**: Upstream sees auth header on upgrade request. Subsequent WS frames forwarded unchanged.

---

### gRPC

#### gRPC unary request proxied (native)
- **Scenario**: [positive-15.1-grpc-unary-request-proxied.md](protocols/grpc/positive-15.1-grpc-unary-request-proxied.md)
- **Mechanism**: `content-type: application/grpc*` detection routes to gRPC handler. Metadata headers preserved.

#### gRPC server streaming proxied
- **Scenario**: [positive-15.2-grpc-server-streaming-proxied.md](protocols/grpc/positive-15.2-grpc-server-streaming-proxied.md)
- **Mechanism**: Stream forwarded without buffering. Backpressure respects HTTP/2 flow control.

#### gRPC JSON transcoding for HTTP clients
- **Scenario**: [positive-15.3-grpc-json-transcoding-http-clients.md](protocols/grpc/positive-15.3-grpc-json-transcoding-http-clients.md)
- **Mechanism**: HTTP JSON request converted to gRPC protobuf. Server streaming returned as `application/x-ndjson`.

---

### WebTransport

#### WT session establishment + auth
- **Scenario**: [positive-17.1-wt-session-establishment-auth.md](protocols/webtransport/positive-17.1-wt-session-establishment-auth.md)
- **Mechanism**: Upstream scheme `wt` accepted. Future-facing — if not implemented yet, `502 ProtocolError` with `ESrc=gateway`.

---

### CORS

#### Built-in CORS handling (preflight)
- **Scenario**: [positive-10.2-built-cors-handling.md](plugins/guards/positive-10.2-built-cors-handling.md)
- **Mechanism**: OPTIONS preflight handled locally (`204` with correct `Access-Control-*`). Preflight bypasses upstream call.

#### Actual request adds CORS headers on response
- **Scenario**: *Section 21.1 — no separate file. Behavior verified as part of HTTP passthrough and guard plugin scenarios.*
- **Mechanism**: Response includes `Access-Control-Allow-Origin` (specific origin, not `*` when credentials). Includes `Vary: Origin`.

#### Hierarchical merge/union for CORS allowed origins
- **Scenario**: *Covered within hierarchical configuration scenarios.*
- **Mechanism**: `inherit` merges origins by union. `enforce` forbids child adding origins.

---

### Guard plugins

> **Flow reference**: [Plugin Execution](flows/plugin-execution.md)

*Note: CORS preflight guard is covered in [CORS](#cors) above. Timeout and Starlark guard rejection scenarios are in [Guardrails → Guard rejections](#guard-rejections).*

---

### Transform plugins

#### Request path rewrite transform
- **Scenario**: [positive-11.1-request-path-rewrite-transform.md](plugins/transforms/positive-11.1-request-path-rewrite-transform.md)
- **Mechanism**: Outbound `path` is rewritten. Transform emits audit-safe logs (no secrets).

#### Query mutation transform
- **Scenario**: [positive-11.2-query-mutation-transform.md](plugins/transforms/positive-11.2-query-mutation-transform.md)
- **Mechanism**: New query param added. Internal params removed. Query allowlist rules remain enforced before transform.

#### Header mutation transform
- **Scenario**: [positive-11.3-header-mutation-transform.md](plugins/transforms/positive-11.3-header-mutation-transform.md)
- **Mechanism**: `headers.set/add/remove` changes reflected upstream. Hop-by-hop headers remain stripped even if plugin tries to set them.

#### Response JSON redaction transform
- **Scenario**: [positive-11.4-response-json-redaction-transform.md](plugins/transforms/positive-11.4-response-json-redaction-transform.md)
- **Mechanism**: Target fields replaced with placeholder. Non-JSON response triggers defined behavior (reject or no-op).

#### on_error transform handles gateway errors
- **Scenario**: [positive-11.5-error-transform-handles-gateway-errors.md](plugins/transforms/positive-11.5-error-transform-handles-gateway-errors.md)
- **Mechanism**: For a gateway error (e.g., rate limit), `on_error` can rewrite `title/detail` and status if allowed.

#### Plugin ordering and layering (upstream before route)
- **Scenario**: [positive-11.6-plugin-ordering-layering.md](plugins/transforms/positive-11.6-plugin-ordering-layering.md)
- **Mechanism**: Upstream plugins run before route plugins. Auth before guards, before transforms.

#### Plugin control flow (next, reject, respond)
- **Scenario**: [positive-11.7-plugin-control-flow.md](plugins/transforms/positive-11.7-plugin-control-flow.md)
- **Mechanism**: `ctx.reject` stops chain and returns gateway error. `ctx.respond` returns custom success response without calling upstream.

---

### Streaming body handling

#### Streaming request bodies are not buffered
- **Scenario**: [positive-8.3-streaming-request-bodies-not-buffered.md](proxy-api/body-validation/positive-8.3-streaming-request-bodies-not-buffered.md)
- **Mechanism**: For endpoints supporting streaming body, memory does not grow with body size.

---

### Observability

#### Metrics endpoint is auth-protected
- **Scenario**: *Section 22.1 — no separate file. Verified as part of management auth scenarios.*
- **Mechanism**: Unauthenticated `GET /metrics` rejected. Non-admin rejected.

#### Request metrics increment on success
- **Scenario**: *Section 22.2 — no separate file.*
- **Mechanism**: `oagw_requests_total` increments with correct labels (host, path, method, status_class). Histogram `phase=total/upstream/plugins` recorded.

#### Error metrics increment on gateway errors
- **Scenario**: *Section 22.3 — no separate file.*
- **Mechanism**: Induce gateway error; `oagw_errors_total{error_type=...}` increments.

#### Cardinality rules (no tenant labels)
- **Scenario**: *Section 22.4 — no separate file.*
- **Mechanism**: Metrics output does not include `tenant_id` label. Path label uses configured route path, not dynamic suffix.

#### Status metrics use status class grouping
- **Scenario**: *Section 22.5 — no separate file.*
- **Mechanism**: Metrics export uses `status_class=2xx/3xx/4xx/5xx` labels.

#### Proxy requests produce structured audit log
- **Scenario**: *Section 23.1 — no separate file.*
- **Mechanism**: Log includes request_id, tenant_id, principal_id, host, path, status, duration.

#### Sensitive data not logged
- **Scenario**: *Section 23.2 — no separate file.*
- **Mechanism**: No request bodies, query params, auth headers, or secret values logged.

#### Config change operations are logged
- **Scenario**: *Section 23.3 — no separate file.*
- **Mechanism**: Upstream/route/plugin create/update/delete yields audit log entry.

---

### OData query support

#### $select projects fields for upstream list
- **Scenario**: *Section 25.1 — no separate file.*
- **Mechanism**: `GET /upstreams?$select=id,alias` returns only those fields.

#### $select validation
- **Scenario**: *Section 25.2 — no separate file.*
- **Mechanism**: Too long, too many fields, or duplicate fields returns `400 PD`.

#### $filter on alias
- **Scenario**: *Section 25.3 — no separate file.*
- **Mechanism**: `GET /upstreams?$filter=alias eq 'api.openai.com'` returns only matches.

#### Pagination ($top/$skip) stable ordering
- **Scenario**: *Section 25.4 — no separate file.*
- **Mechanism**: Paging produces consistent sets.

#### $select works for routes and plugins lists
- **Scenario**: *Section 25.5 — no separate file.*
- **Mechanism**: `GET /routes?$select=id,upstream_id,match` and `GET /plugins?$select=id,plugin_type,name` return only those fields.

#### $orderby is applied and validated
- **Scenario**: *Section 25.6 — no separate file.*
- **Mechanism**: `created_at desc` ordering changes item order. Invalid `$orderby` yields `400 PD`.

#### $top max and $skip bounds enforced
- **Scenario**: *Section 25.7 — no separate file.*
- **Mechanism**: `$top` above max clamps or rejects. Negative `$skip` rejected.

---

### E2E protocol examples

Full integration walkthroughs — each demonstrates the complete journey (upstream → route → auth → invoke → response).

#### HTTP request/response
- **Scenario**: [positive-example-01-http-request-response.md](protocols/http/examples/positive-example-01-http-request-response.md)
- **Mechanism**: End-to-end HTTP proxy call with upstream creation, route setup, auth injection, and response passthrough.

#### SSE streaming
- **Scenario**: [positive-example-02-sse-streaming.md](protocols/sse/examples/positive-example-02-sse-streaming.md)
- **Mechanism**: End-to-end SSE stream with `text/event-stream` forwarding without buffering.

#### WebSocket upgrade
- **Scenario**: [positive-example-03-websocket-upgrade.md](protocols/websocket/examples/positive-example-03-websocket-upgrade.md)
- **Mechanism**: End-to-end WebSocket with `101 Switching Protocols` handshake proxying.

#### gRPC unary proxy
- **Scenario**: [positive-example-04-grpc-unary-proxy.md](protocols/grpc/examples/positive-example-04-grpc-unary-proxy.md)
- **Mechanism**: End-to-end gRPC unary call with service+method routing and metadata preservation.

---

## 2. Guardrails, Edge Cases & Rejections

### Authentication & authorization

#### Missing Bearer token on management endpoints → 401
- **Scenario**: [negative-1.1-all-management-endpoints-require-bearer-auth.md](management-api/auth/negative-1.1-all-management-endpoints-require-bearer-auth.md)
- **What happens**: All management endpoints require `Authorization: Bearer` header. Missing or invalid token returns `401` `PD` with stable `type`.

#### Permission gates for upstream/route/plugin CRUD → 403
- **Scenario**: [negative-1.2-permission-gates-upstream-route-plugin-crud.md](management-api/auth/negative-1.2-permission-gates-upstream-route-plugin-crud.md)
- **What happens**: Missing permission for resource type returns `403`. With correct permission, same call succeeds.

#### Tenant scoping in management APIs → cross-tenant blocked
- **Scenario**: [negative-1.3-tenant-scoping-management-apis.md](management-api/auth/negative-1.3-tenant-scoping-management-apis.md)
- **What happens**: Tenant A cannot `GET /upstreams/{id}` created by tenant B. Listing returns only tenant-visible resources.

#### Proxy invoke permission required → 403
- **Scenario**: [negative-5.1-proxy-invoke-permission-required.md](proxy-api/authz/negative-5.1-proxy-invoke-permission-required.md)
- **What happens**: Missing `gts.x.core.oagw.proxy.v1~:invoke` returns `403`.

#### Proxy cannot access upstream not owned or shared → 403/404
- **Scenario**: [negative-5.2-proxy-cannot-access-upstream-not-owned-shared.md](proxy-api/authz/negative-5.2-proxy-cannot-access-upstream-not-owned-shared.md)
- **What happens**: Tenant A invoking Tenant B private upstream returns `403` or `404` (must not leak existence).

---

### Input validation

#### Method allowlist enforcement → 404
- **Scenario**: [negative-3.2-method-allowlist-enforcement.md](management-api/routes/negative-3.2-method-allowlist-enforcement.md)
- **What happens**: Route allows `POST`; invoking `GET` returns `404 ROUTE_NOT_FOUND` as a gateway error.

#### Query allowlist enforcement → 400
- **Scenario**: [negative-3.3-query-allowlist-enforcement.md](management-api/routes/negative-3.3-query-allowlist-enforcement.md)
- **What happens**: Unknown query param rejected with `400 PD`, `ESrc=gateway`.

#### Path suffix mode = disabled → rejected
- **Scenario**: [negative-3.4-path-suffix-mode-disabled.md](management-api/routes/negative-3.4-path-suffix-mode-disabled.md)
- **What happens**: Supplying any suffix returns a gateway validation error.

#### Well-known header validation errors → 400
- **Scenario**: [negative-7.4-well-known-header-validation-errors-400.md](proxy-api/request-transforms/negative-7.4-well-known-header-validation-errors-400.md)
- **What happens**: Invalid `Content-Length` or mismatch with actual body returns `400 PD`.

#### Maximum body size limit enforced (100MB) → 413
- **Scenario**: [negative-8.1-maximum-body-size-limit-enforced.md](proxy-api/body-validation/negative-8.1-maximum-body-size-limit-enforced.md)
- **What happens**: Body > 100MB rejected early with `413 PD`, `ESrc=gateway`.

#### Transfer-Encoding support limited to chunked → 400
- **Scenario**: [negative-8.2-transfer-encoding-support-limited-chunked.md](proxy-api/body-validation/negative-8.2-transfer-encoding-support-limited-chunked.md)
- **What happens**: Unsupported transfer-encoding rejected with `400 PD`.

---

### Routing rejections

#### Explicit alias required for IP-based endpoints → 400
- **Scenario**: [negative-2.3-explicit-alias-required-ip-based-endpoints.md](management-api/upstreams/negative-2.3-explicit-alias-required-ip-based-endpoints.md)
- **What happens**: Create upstream with `host=10.0.0.1` and no alias is rejected (`400 PD`).

#### Multi-endpoint upstream pool compatibility rules → 400
- **Scenario**: [negative-2.4-multi-endpoint-upstream-pool-compatibility-rules.md](management-api/upstreams/negative-2.4-multi-endpoint-upstream-pool-compatibility-rules.md)
- **What happens**: Mismatched `scheme`, `port`, or `protocol` in pool fails with `400`.

#### Disable upstream blocks proxy traffic → 503
- **Scenario**: [negative-2.6-disable-upstream-blocks-proxy-traffic.md](management-api/upstreams/negative-2.6-disable-upstream-blocks-proxy-traffic.md)
- **What happens**: With `enabled=false`, proxy returns `503 PD`, `ESrc=gateway`. Disabled in ancestor → disabled for descendants too.

#### Alias uniqueness enforced per tenant → conflict
- **Scenario**: [negative-2.8-alias-uniqueness-enforced-per-tenant.md](management-api/upstreams/negative-2.8-alias-uniqueness-enforced-per-tenant.md)
- **What happens**: Two upstreams with same alias in same tenant rejected (conflict). Same alias in different tenants succeeds.

#### Enforced ancestor limits still apply when alias shadowed
- **Scenario**: [negative-6.2-enforced-ancestor-limits-still-apply-alias-shadowed.md](proxy-api/alias-resolution/negative-6.2-enforced-ancestor-limits-still-apply-alias-shadowed.md)
- **What happens**: Parent `rate_limit.sharing=enforce` still constrains child effective limit even when alias is shadowed.

#### Alias not found → stable 404
- **Scenario**: [negative-6.4-alias-not-found-returns-stable-404.md](proxy-api/alias-resolution/negative-6.4-alias-not-found-returns-stable-404.md)
- **What happens**: Unknown alias returns `404 PD`, `ESrc=gateway`, `type` = `...upstream.not_found...`.

#### Disable route blocks proxy traffic → 404/503
- **Scenario**: [negative-3.7-disable-route-blocks-proxy-traffic.md](management-api/routes/negative-3.7-disable-route-blocks-proxy-traffic.md)
- **What happens**: With `route.enabled=false`, request returns `404 ROUTE_NOT_FOUND` or `503`, `ESrc=gateway`.

---

### Security protections (SSRF, headers, injection)

#### SSRF prevention by strict upstream host selection
- **Scenario**: *Section 26.1 — no separate file.*
- **What happens**: Client cannot override upstream destination via inbound `Host` header. Absolute-form URLs in request line rejected or normalized.

#### Header injection protections
- **Scenario**: *Section 26.2 — no separate file.*
- **What happens**: Newline characters in header values rejected. Prevents response/request splitting.

#### Protocol mismatch errors are explicit
- **Scenario**: *Section 26.3 — no separate file.*
- **What happens**: Using HTTP route against `protocol=grpc` upstream fails with `502 ProtocolError` (`PD`).

---

### Plugin enforcement

#### Plugin immutability (no update) → 404/405
- **Scenario**: [negative-4.2-plugin-immutability.md](management-api/plugins/negative-4.2-plugin-immutability.md)
- **What happens**: `PUT /plugins/{id}` is not available (404/405). Ensures reproducibility and auditability.

#### Plugin type enforcement → 400
- **Scenario**: [negative-4.4-plugin-type-enforcement.md](management-api/plugins/negative-4.4-plugin-type-enforcement.md)
- **What happens**: Attaching `plugin.guard` where auth is expected fails at config validation (`400`).

#### Secret access control via cred_store → 401/500
- **Scenario**: [negative-9.7-secret-access-control-cred-store.md](proxy-api/authentication/negative-9.7-secret-access-control-cred-store.md)
- **What happens**: If `cred_store` denies access, proxy returns `401 AuthenticationFailed`. If secret missing, returns `500 SecretNotFound`.

#### Descendant override permissions enforced → blocked
- **Scenario**: [negative-9.9-descendant-override-permissions-enforced.md](proxy-api/authentication/negative-9.9-descendant-override-permissions-enforced.md)
- **What happens**: Without `oagw:upstream:override_auth`, child cannot override inherited auth. Same for `override_rate` and `add_plugins`.

#### Starlark sandbox restrictions → blocked
- **Scenario**: [negative-11.8-starlark-sandbox-restrictions.md](plugins/transforms/negative-11.8-starlark-sandbox-restrictions.md)
- **What happens**: Network I/O fails. File I/O fails. Infinite loop times out. Large allocation blocked.

---

### Rate limiting rejections

#### Sliding window strictness → no boundary bursts
- **Scenario**: [negative-18.2-sliding-window-strictness.md](rate-limiting/negative-18.2-sliding-window-strictness.md)
- **What happens**: Requests across a window boundary do not allow 2x burst.

#### Strategy variants when limit exceeded → 429/503
- **Scenario**: [negative-18.5-strategy-variants-limit-exceeded.md](rate-limiting/negative-18.5-strategy-variants-limit-exceeded.md)
- **What happens**: `strategy=reject` returns `429`. `strategy=queue` delays then succeeds or times out with `503 queue.timeout`. `strategy=degrade` uses configured fallback.

---

### Custom header routing rejections

#### Missing X-OAGW-Target-Host for common suffix alias → 400
- **Scenarios**:
  - [negative-6.3-multi-endpoint-common-suffix-alias-requires-host-header.md](proxy-api/alias-resolution/negative-6.3-multi-endpoint-common-suffix-alias-requires-host-header.md)
  - [negative-1.1-missing-header-common-suffix.md](proxy-api/custom-header-routing/negative-1.1-missing-header-common-suffix.md)
- **What happens**: Missing header for common suffix alias returns `400 PD` with list of valid endpoint hosts.

#### Invalid X-OAGW-Target-Host format (port) → 400
- **Scenario**: [negative-1.2-invalid-format-with-port.md](proxy-api/custom-header-routing/negative-1.2-invalid-format-with-port.md)
- **What happens**: Header with port number rejected. Port is defined in upstream config, not header.

#### Invalid X-OAGW-Target-Host format (path) → 400
- **Scenario**: [negative-1.3-invalid-format-with-path.md](proxy-api/custom-header-routing/negative-1.3-invalid-format-with-path.md)
- **What happens**: Header with path component rejected.

#### Invalid X-OAGW-Target-Host format (special chars) → 400
- **Scenario**: [negative-1.4-invalid-format-special-chars.md](proxy-api/custom-header-routing/negative-1.4-invalid-format-special-chars.md)
- **What happens**: Header with query params or special characters rejected.

#### Unknown X-OAGW-Target-Host not in endpoint list → 400
- **Scenario**: [negative-2.1-unknown-host.md](proxy-api/custom-header-routing/negative-2.1-unknown-host.md)
- **What happens**: Header value not matching any endpoint rejected. Allowlist validation prevents SSRF.

#### IP address when hostname expected → 400
- **Scenario**: [negative-2.2-ip-address-when-hostname-expected.md](proxy-api/custom-header-routing/negative-2.2-ip-address-when-hostname-expected.md)
- **What happens**: IP address when endpoints use hostnames treated as unknown host.

---

### Guard rejections

#### Timeout guard plugin enforces request timeout → 504
- **Scenario**: [negative-10.1-timeout-guard-plugin-enforces-request-timeout.md](plugins/guards/negative-10.1-timeout-guard-plugin-enforces-request-timeout.md)
- **What happens**: Request exceeding timeout returns `504` gateway timeout (`PD`, `ESrc=gateway`).

#### CORS credentials + wildcard rejected by config validation → 400
- **Scenario**: [negative-10.3-cors-credentials-wildcard-rejected-config-validation.md](plugins/guards/negative-10.3-cors-credentials-wildcard-rejected-config-validation.md)
- **What happens**: `allow_credentials=true` + `allowed_origins=['*']` rejected (`400`).

#### Custom Starlark guard rejects based on headers/body
- **Scenario**: [negative-10.4-custom-starlark-guard-rejects-based-headers-body.md](plugins/guards/negative-10.4-custom-starlark-guard-rejects-based-headers-body.md)
- **What happens**: Missing required header rejected with plugin-defined status. Body-too-large rejected with `413`. Guard only runs `on_request`.

---

### Protocol errors

#### Upstream error passthrough with ESrc=upstream
- **Scenario**: [negative-12.2-upstream-error-passthrough-esrc-upstream.md](protocols/http/negative-12.2-upstream-error-passthrough-esrc-upstream.md)
- **What happens**: Upstream returns `500` with JSON body. OAGW keeps body intact, sets `ESrc=upstream`.

#### Gateway error uses PD + ESrc=gateway
- **Scenario**: [negative-12.3-gateway-error-uses-pd-esrc-gateway.md](protocols/http/negative-12.3-gateway-error-uses-pd-esrc-gateway.md)
- **What happens**: Route not found / validation error → `PD`, `Content-Type=application/problem+json`, `ESrc=gateway`.

#### No automatic retries for upstream failures
- **Scenario**: [negative-12.6-no-automatic-retries-upstream-failures.md](protocols/http/negative-12.6-no-automatic-retries-upstream-failures.md)
- **What happens**: Upstream transient `5xx` or connection close → OAGW returns error once. Upstream sees single request attempt.

#### Scheme/protocol mismatches fail explicitly
- **Scenario**: [negative-12.7-scheme-protocol-mismatches-fail-explicitly.md](protocols/http/negative-12.7-scheme-protocol-mismatches-fail-explicitly.md)
- **What happens**: `protocol=http` with `scheme=grpc` rejected at config validation. `protocol=grpc` with `scheme=https` yields `502 ProtocolError`.

#### gRPC status mapping and error source
- **Scenario**: [negative-15.4-grpc-status-mapping-error-source.md](protocols/grpc/negative-15.4-grpc-status-mapping-error-source.md)
- **What happens**: gRPC `RESOURCE_EXHAUSTED` maps to rate limit error. Upstream gRPC failures marked `ESrc=upstream`.

---

### Connection limits

#### WebSocket rate limit applies to connection establishment → 429
- **Scenario**: [negative-14.3-rate-limit-applies-connection-establishment.md](protocols/websocket/negative-14.3-rate-limit-applies-connection-establishment.md)
- **What happens**: Exceeding rate limit rejects upgrade with `429` (`PD`, `ESrc=gateway`).

#### WS connection idle timeout enforced
- **Scenario**: [negative-14.4-ws-connection-idle-timeout-enforced.md](protocols/websocket/negative-14.4-ws-connection-idle-timeout-enforced.md)
- **What happens**: Idle connection closed after configured timeout.

---

### Error handling invariants

#### Gateway errors always use RFC 9457 PD
- **Scenario**: *Section 24.1 — no separate file.*
- **What happens**: For each gateway error class (400/401/403/404/409/413/429/5xx), body is `PD`.

#### ESrc header set for both gateway and upstream failures
- **Scenario**: *Section 24.2 — no separate file.*
- **What happens**: Gateway error → `ESrc=gateway`. Upstream error → `ESrc=upstream`.

#### Retry-After present for retriable gateway errors
- **Scenario**: *Section 24.3 — no separate file.*
- **What happens**: `429`, `503 link unavailable`, `503 circuit breaker open`, `504` timeouts include `Retry-After`.

#### Stream aborted classified distinctly
- **Scenario**: *Section 24.4 — no separate file.*
- **What happens**: Abort SSE/WS mid-flight → classified as `StreamAborted`.

#### Every documented error type has at least one reproducer
- **Scenario**: *Section 24.5 — no separate file.*
- **What happens**: `ValidationError`, `RouteNotFound`, `AuthenticationFailed`, `PayloadTooLarge`, `RateLimitExceeded`, `SecretNotFound`, `ProtocolError`, `DownstreamError`, `StreamAborted`, `LinkUnavailable`, `CircuitBreakerOpen`, `ConnectionTimeout`, `RequestTimeout`, `IdleTimeout`, `PluginNotFound`, `PluginInUse` — each has at least one reproducer scenario.

---

### Concurrency & circuit breaker (future-facing)

#### Concurrency limit rejects when max in-flight reached → 503
- *No scenario file (future-facing).*
- **What happens**: When `max_concurrent` exceeded, return `503 concurrency_limit.exceeded` with `Retry-After`.

#### Queue strategy buffers requests up to max depth
- *No scenario file (future-facing).*
- **What happens**: FIFO order. `queue.full` when depth exceeded. `queue.timeout` when wait exceeds config. Memory limit enforced.

#### Streaming requests hold permits until completion
- *No scenario file (future-facing).*
- **What happens**: SSE and WS hold concurrency permit until closed.

#### Circuit opens after consecutive failures → 503
- *No scenario file (future-facing).*
- **What happens**: After threshold, requests fail fast with `503 circuit_breaker.open` + `Retry-After`.

#### Half-open probing closes circuit on recovery
- *No scenario file (future-facing).*
- **What happens**: After open timeout, limited probes allowed. Success threshold closes circuit.

#### Per-endpoint circuit scope
- *No scenario file (future-facing).*
- **What happens**: With `scope=per_endpoint`, failures isolate to single endpoint.

---

### ID & cross-surface consistency

#### IDs are anonymous GTS identifiers on API surface
- **Scenario**: *Section 27.1 — no separate file.*
- **What happens**: `GET /upstreams/{id}` accepts `gts.x.core.oagw.upstream.v1~{uuid}`. Same for routes/plugins.

---

## Appendix: Coverage Matrix

For E2E coverage, ensure at least one scenario exists for each cell:

### Protocol × scheme
- HTTP:
  - `https`
- Streaming:
  - SSE over `https`
  - WebSocket over `wss`
  - WebTransport over `wt` (feature or explicit-not-supported behavior)
- gRPC:
  - `grpc` (and HTTP/2 multiplexing on shared ingress port)

### Outbound auth plugin × protocol
- `noop`: HTTP
- `apikey`: HTTP, WebSocket, SSE, gRPC
- `basic`: HTTP
- `bearer`: HTTP
- `oauth2.client_cred`: HTTP
- `oauth2.client_cred_basic`: HTTP

### Plugin chain phases
- Guard-only (`on_request`)
- Transform (`on_request`)
- Transform (`on_response`)
- Transform (`on_error`)

### Error source
- Gateway-generated (validation/rate-limit/etc)
- Upstream passthrough (4xx/5xx)
