# PRD — Outbound API Gateway (OAGW)

## 1. Overview

### 1.1 Purpose

The Outbound API Gateway (OAGW) manages all outbound API requests from CyberFabric to external services. It acts as a centralized proxy layer that handles credential injection, rate limiting, header transformation, and security enforcement for every external call made by the platform.

OAGW provides a unified interface for application modules to reach external APIs without managing credentials, connection details, or security policies directly. Modules send requests to OAGW's proxy endpoint, and OAGW resolves the target upstream, injects authentication, applies policies, and forwards the request.

### 1.2 Background / Problem Statement

CyberFabric modules frequently need to call external services (AI providers, payment processors, third-party APIs). Without a centralized outbound gateway, each module must independently manage API keys, handle rate limits, implement SSRF protections, and maintain connection configurations. This leads to fragmented credential handling across modules, inconsistent error handling and retry behavior, no centralized rate limiting (risking abuse and cost overruns), and potential SSRF vulnerabilities where internal services could be tricked into calling unintended endpoints.

OAGW solves these problems by providing a single outbound proxy layer with pluggable authentication, configurable rate limiting, header transformation, and security policies. All external calls flow through OAGW, ensuring consistent credential isolation, audit trails, and policy enforcement across the platform.

### 1.3 Goals (Business Outcomes)

- Less than 10ms added latency at p95 for proxied requests
- Zero credential exposure in logs, error messages, or client responses
- 99.9% gateway availability
- Complete audit trail for all proxied requests

### 1.4 Glossary

| Term | Definition |
|------|------------|
| Upstream | External service target defined by server endpoints (scheme/host/port), protocol, authentication configuration, default headers, and rate limits |
| Route | API path on an upstream that matches requests by method, path, and query allowlist (HTTP) or service and method (gRPC). Routes map inbound proxy requests to specific upstream behaviors |
| Plugin | Modular processor attached to upstreams or routes. Three types: Auth (credential injection), Guard (validation/policy enforcement), Transform (request/response mutation) |
| Data Plane | Internal service that orchestrates proxy requests: resolves configuration, executes plugin chains, and forwards HTTP calls to external services |
| Control Plane | Internal service that manages configuration data (upstreams, routes, plugins) with repository access |
| Alias | Short identifier used in proxy URLs to reference an upstream. Derived from hostname by default, or explicitly set for IP-based or multi-endpoint upstreams |
| Sharing Mode | Configuration visibility setting for hierarchical tenancy: `private` (owner only), `inherit` (descendants can override), `enforce` (descendants cannot override) |
| GTS | Global Type System — the platform's schema and instance registration system used for plugin type identification |

## 2. Actors

> **Note**: Stakeholder needs are managed at the project/task level by the steering committee and are not duplicated in module specs. Focus on **actors** (users, systems) that directly interact with this module.

### 2.1 Human Actors

#### Platform Operator

**ID**: `cpt-cf-oagw-actor-platform-operator`

- **Role**: Manages global OAGW configuration: upstreams, routes, system-wide plugins, and security policies.
- **Needs**: Full CRUD access to all upstreams, routes, and plugins. Ability to enforce rate limits and security policies across all tenants. Visibility into all proxy traffic for audit and troubleshooting.

#### Tenant Administrator

**ID**: `cpt-cf-oagw-actor-tenant-admin`

- **Role**: Manages tenant-specific OAGW settings: credentials, rate limits, custom plugins, and upstream overrides within their tenant hierarchy.
- **Needs**: Ability to configure tenant-scoped upstreams and routes, manage credentials via credential store references, set tenant-level rate limits, and attach custom plugins. Ability to override inherited configurations where sharing mode permits.

#### Application Developer

**ID**: `cpt-cf-oagw-actor-app-developer`

- **Role**: Consumes external APIs via OAGW proxy endpoint. Does not manage credentials or upstream configurations directly.
- **Needs**: Simple proxy endpoint to reach external services. Consistent error responses. No credential management burden.

### 2.2 System Actors

#### Credential Store

**ID**: `cpt-cf-oagw-actor-credential-store`

- **Role**: Secure storage and retrieval of secrets by UUID reference. OAGW retrieves credentials at request time to inject into outbound requests. Supports hierarchical secret resolution across tenant hierarchy.

#### Types Registry

**ID**: `cpt-cf-oagw-actor-types-registry`

- **Role**: GTS schema and instance registration and validation. Provides plugin type identification and validation for Auth, Guard, and Transform plugin types.

#### Upstream Service

**ID**: `cpt-cf-oagw-actor-upstream-service`

- **Role**: External third-party service that OAGW proxies requests to (e.g., OpenAI, Stripe, cloud provider APIs). The ultimate recipient of outbound requests after OAGW applies authentication, transformation, and policy enforcement.

## 3. Operational Concept & Environment

> **Note**: Project-wide runtime, OS, architecture, lifecycle policy, and module integration patterns are defined in root PRD. Only document module-specific deviations or additional constraints here.

### 3.1 Module-Specific Environment Constraints

- OAGW is implemented as a single ModKit module (`oagw` crate) with internal service isolation via domain traits (Control Plane and Data Plane)
- Requires outbound network access to external services (firewall rules must permit egress to configured upstream hosts)
- Starlark sandbox environment for future scripting support requires constrained execution: no network I/O, no file I/O, no imports, with enforced timeout and memory limits

## 4. Scope

### 4.1 In Scope

- CRUD operations for upstream configurations (server endpoints, protocol, auth, headers, rate limits)
- CRUD operations for route configurations (method, path, query match rules mapped to upstreams)
- CRUD operations for plugin definitions (Auth, Guard, Transform types)
- HTTP request proxying via alias-based URL resolution
- Credential injection for API Key, Basic Auth, OAuth2 Client Credentials, and Bearer Token authentication
- Rate limiting at upstream and route levels with configurable scope and strategy
- Header transformation (set/add/remove) for requests and responses
- Plugin system with Auth, Guard, and Transform plugin types (built-in and external)
- Streaming support for HTTP request/response, SSE, WebSocket, and WebTransport
- Multi-tenant configuration layering (upstream < route < tenant)
- Hierarchical configuration override with sharing modes (private/inherit/enforce)
- Alias resolution with shadowing across tenant hierarchy
- Multi-endpoint pooling within upstreams
- Enable/disable semantics for upstreams and routes with hierarchical inheritance
- Circuit breaker as core gateway resilience policy
- SSRF protection (DNS validation, IP pinning, header stripping)
- Complete audit trail for all proxied requests

### 4.2 Out of Scope

- gRPC proxying (planned for phase 4)
- Automatic retries (each inbound request results in at most one upstream attempt; retry behavior is client-managed)
- Starlark user-defined scripts (sandbox infrastructure exists but scripting is a future capability)
- Service mesh or east-west traffic management (OAGW handles outbound/north-south traffic only)

## 5. Functional Requirements

> **Testing strategy**: All requirements verified via automated tests (unit, integration, e2e) targeting 90%+ code coverage unless otherwise specified. Document verification method only for non-test approaches.

### 5.1 P1 — Core Proxy Operations

#### Upstream Management

- [ ] `p1` - **ID**: `cpt-cf-oagw-fr-upstream-mgmt`

The system **MUST** provide CRUD operations for upstream configurations. Each upstream defines server endpoints (scheme, host, port), protocol, authentication configuration, default headers, and rate limits.

- **Rationale**: Core capability — all outbound proxying depends on upstream definitions.
- **Actors**: `cpt-cf-oagw-actor-platform-operator`, `cpt-cf-oagw-actor-tenant-admin`

#### Route Management

- [ ] `p1` - **ID**: `cpt-cf-oagw-fr-route-mgmt`

The system **MUST** provide CRUD operations for routes. Routes define matching rules (method, path, query allowlist) that map inbound proxy requests to upstreams.

- **Rationale**: Routes are the mechanism by which proxy requests are matched to specific upstream behaviors.
- **Actors**: `cpt-cf-oagw-actor-platform-operator`, `cpt-cf-oagw-actor-tenant-admin`

#### Enable/Disable Semantics

- [ ] `p1` - **ID**: `cpt-cf-oagw-fr-enable-disable`

The system **MUST** support an `enabled` boolean field (default: `true`) on upstreams and routes. Disabled upstreams **MUST** reject all proxy requests with 503 Service Unavailable while remaining visible in list/get operations. Disabled routes **MUST** be excluded from matching, causing requests to fall through to the next match or return 404. If an ancestor tenant disables an upstream, it **MUST** be disabled for all descendants. Descendants **MUST NOT** re-enable an ancestor-disabled resource.

- **Rationale**: Enables temporary maintenance without deleting configuration, emergency circuit breaking at the management layer, and gradual rollout (enable route for subset of tenants).
- **Actors**: `cpt-cf-oagw-actor-platform-operator`, `cpt-cf-oagw-actor-tenant-admin`

#### Request Proxying

- [ ] `p1` - **ID**: `cpt-cf-oagw-fr-request-proxy`

The system **MUST** proxy requests via `{METHOD} /api/oagw/v1/proxy/{alias}[/{path}][?{query}]`. The system **MUST** resolve the upstream by alias, match the route by method/path, merge configurations (upstream < route < tenant), retrieve credentials, execute the plugin chain, transform the request, and forward it to the external service. No automatic retries are performed; each inbound request results in at most one upstream attempt.

- **Rationale**: Core value proposition — unified outbound proxy with centralized policy enforcement.
- **Actors**: `cpt-cf-oagw-actor-app-developer`, `cpt-cf-oagw-actor-upstream-service`

#### Authentication Injection

- [ ] `p1` - **ID**: `cpt-cf-oagw-fr-auth-injection`

The system **MUST** inject credentials into outbound requests at request time. Supported authentication methods: API Key (header or query parameter), HTTP Basic Auth, OAuth2 Client Credentials flow, and Bearer Token. Credentials **MUST** be retrieved from the credential store by UUID reference at the time of each request.

- **Rationale**: Centralizes credential management so application modules never handle API keys or tokens directly.
- **Actors**: `cpt-cf-oagw-actor-app-developer`, `cpt-cf-oagw-actor-credential-store`

#### Rate Limiting

- [ ] `p1` - **ID**: `cpt-cf-oagw-fr-rate-limiting`

The system **MUST** enforce rate limits at upstream and route levels. Rate limit configuration **MUST** support: rate (requests per window), window duration, capacity (burst), cost (per-request weight), scope (global, tenant, user, or IP), and strategy (reject with 429 and Retry-After header, queue, or degrade).

- **Rationale**: Prevents abuse and cost overruns when calling external paid APIs.
- **Actors**: `cpt-cf-oagw-actor-platform-operator`, `cpt-cf-oagw-actor-tenant-admin`

### 5.2 P1 — Multi-Tenant Configuration

#### Configuration Layering

- [ ] `p1` - **ID**: `cpt-cf-oagw-fr-config-layering`

The system **MUST** merge configurations with the following priority order: upstream (base) < route < tenant (highest priority). When the same setting is defined at multiple levels, the higher-priority level wins.

- **Rationale**: Enables fine-grained configuration without duplicating settings at every level.
- **Actors**: `cpt-cf-oagw-actor-platform-operator`, `cpt-cf-oagw-actor-tenant-admin`

#### Hierarchical Configuration Override

- [ ] `p1` - **ID**: `cpt-cf-oagw-fr-config-hierarchy`

The system **MUST** support hierarchical configuration override with three sharing modes:

- `private`: not visible to descendants (default)
- `inherit`: visible to descendants; descendant can override if specified
- `enforce`: visible to descendants; descendant cannot override

Override rules:

- **Auth**: with `sharing: inherit`, a descendant with permission can use its own credentials
- **Rate limits**: descendant can only be stricter: `effective = min(ancestor.enforced, descendant)`
- **Plugins**: descendant's plugins append to the chain; enforced plugins cannot be removed
- **Tags (discovery metadata)**: merged top-to-bottom with add-only semantics (`effective_tags = union(ancestor_tags..., descendant_tags)`); descendants can add tags but cannot remove inherited tags

If upstream creation resolves to an existing upstream definition (binding-style flow), request tags are treated as tenant-local additions for effective discovery; they do not mutate ancestor tags.

- **Rationale**: Enables partner tenants to share configurations with customers while maintaining control over enforced policies.
- **Actors**: `cpt-cf-oagw-actor-platform-operator`, `cpt-cf-oagw-actor-tenant-admin`

#### Alias Resolution and Shadowing

- [ ] `p1` - **ID**: `cpt-cf-oagw-fr-alias-resolution`

The system **MUST** resolve upstreams by alias in proxy URLs (`{METHOD} /api/oagw/v1/proxy/{alias}/{path}`).

**Alias defaults**:

- Single endpoint: alias defaults to `server.endpoints[0].host` (without port)
- Multiple endpoints: system extracts common domain suffix
- IP-based or heterogeneous hosts: explicit alias is mandatory

**Shadowing behavior**: when resolving an alias, the system **MUST** search the tenant hierarchy from descendant to root; the closest match wins (descendant shadows ancestor). When a descendant shadows an ancestor's alias, enforced limits from the ancestor still apply.

**Multi-endpoint pooling**: multiple endpoints within the same upstream form a load-balance pool. Requests are distributed across endpoints. Endpoints in a pool **MUST** have identical protocol, scheme, and port.

- **Rationale**: Alias-based routing provides a stable, human-readable proxy URL that decouples callers from upstream connection details. Shadowing enables tenant-specific overrides.
- **Actors**: `cpt-cf-oagw-actor-app-developer`, `cpt-cf-oagw-actor-tenant-admin`

### 5.3 P1 — Extensibility

#### Plugin System

- [ ] `p1` - **ID**: `cpt-cf-oagw-fr-plugin-system`

The system **MUST** provide a plugin system with three plugin types:

- **Auth** (`gts.x.core.oagw.auth_plugin.v1~*`): credential injection (API key, OAuth2, Bearer token, Basic auth)
- **Guard** (`gts.x.core.oagw.guard_plugin.v1~*`): validation and policy enforcement; can reject requests (timeout, CORS, rate limiting)
- **Transform** (`gts.x.core.oagw.transform_plugin.v1~*`): request/response mutation (logging, metrics, request ID)

**Execution order**: Auth → Guards → Transform(request) → Upstream call → Transform(response/error).

Plugin chain composition: upstream plugins execute before route plugins.

The system **MUST** support two plugin categories:

- **Built-in plugins**: included in the `oagw` crate, implemented in Rust
- **External plugins**: separate ModKit modules implementing plugin traits from `oagw-sdk`

**Built-in Auth plugins**:

- `gts.x.core.oagw.auth_plugin.v1~x.core.oagw.noop.v1` — no authentication
- `gts.x.core.oagw.auth_plugin.v1~x.core.oagw.apikey.v1` — API key injection (header/query)
- `gts.x.core.oagw.auth_plugin.v1~x.core.oagw.basic.v1` — HTTP Basic authentication
- `gts.x.core.oagw.auth_plugin.v1~x.core.oagw.oauth2_client_cred.v1` — OAuth2 client credentials flow
- `gts.x.core.oagw.auth_plugin.v1~x.core.oagw.oauth2_client_cred_basic.v1` — OAuth2 with Basic auth
- `gts.x.core.oagw.auth_plugin.v1~x.core.oagw.bearer.v1` — Bearer token injection

**Built-in Guard plugins**:

- `gts.x.core.oagw.guard_plugin.v1~x.core.oagw.timeout.v1` — request timeout enforcement
- `gts.x.core.oagw.guard_plugin.v1~x.core.oagw.cors.v1` — CORS preflight validation

**Built-in Transform plugins**:

- `gts.x.core.oagw.transform_plugin.v1~x.core.oagw.logging.v1` — request/response logging
- `gts.x.core.oagw.transform_plugin.v1~x.core.oagw.metrics.v1` — Prometheus metrics collection
- `gts.x.core.oagw.transform_plugin.v1~x.core.oagw.request_id.v1` — X-Request-ID propagation

- **Rationale**: Plugin architecture enables extensibility without modifying core gateway logic. Built-in plugins cover common cross-cutting concerns. External plugins allow tenant-specific or domain-specific customization.
- **Actors**: `cpt-cf-oagw-actor-platform-operator`, `cpt-cf-oagw-actor-tenant-admin`, `cpt-cf-oagw-actor-types-registry`

#### Plugin Immutability

- [ ] `p1` - **ID**: `cpt-cf-oagw-fr-plugin-immutability`

Plugin definitions **MUST** be immutable after creation. Updates are performed by creating a new plugin version and re-binding references.

- **Rationale**: Immutability guarantees deterministic behavior for attached routes and upstreams, improves auditability, and avoids in-place source mutation risks.
- **Actors**: `cpt-cf-oagw-actor-platform-operator`, `cpt-cf-oagw-actor-tenant-admin`

#### Plugin CRUD

- [ ] `p2` - **ID**: `cpt-cf-oagw-fr-plugin-crud`

The system **MUST** provide create, get, list, and delete operations for plugin definitions. The system **MUST** provide a read-only endpoint to retrieve the plugin source.

- **Rationale**: Management operations for plugin lifecycle.
- **Actors**: `cpt-cf-oagw-actor-platform-operator`, `cpt-cf-oagw-actor-tenant-admin`

### 5.4 P2 — Traffic Handling

#### Header Transformation

- [ ] `p2` - **ID**: `cpt-cf-oagw-fr-header-transform`

The system **MUST** support request and response header transformation: set (overwrite), add (append), and remove operations. The system **MUST** support passthrough control (allow or block specific headers) and automatic stripping of hop-by-hop headers.

- **Rationale**: Enables adapting requests/responses to upstream API requirements and enforcing security policies on headers.
- **Actors**: `cpt-cf-oagw-actor-platform-operator`, `cpt-cf-oagw-actor-tenant-admin`

#### Streaming Support

- [ ] `p2` - **ID**: `cpt-cf-oagw-fr-streaming`

The system **MUST** support streaming for HTTP request/response, Server-Sent Events (SSE), WebSocket, and WebTransport session flows. SSE streaming **MUST** forward events as received and handle connection lifecycle (open, close, error).

- **Rationale**: Many external APIs (especially AI providers) use streaming responses. OAGW must transparently proxy these without buffering.
- **Actors**: `cpt-cf-oagw-actor-app-developer`, `cpt-cf-oagw-actor-upstream-service`

#### Circuit Breaker

- [ ] `p2` - **ID**: `cpt-cf-oagw-fr-circuit-breaker`

The system **MUST** provide circuit breaker as a core gateway resilience capability (configured as core policy, not a plugin). When the circuit is open, the system **MUST** return 503 Service Unavailable.

- **Rationale**: Prevents cascade failures when upstream services are degraded or unavailable.
- **Actors**: `cpt-cf-oagw-actor-platform-operator`

## 6. Non-Functional Requirements

> **Global baselines**: Project-wide NFRs are defined in root PRD and guidelines. Document only module-specific NFRs here.

### 6.1 Module-Specific NFRs

#### Low Latency

- [ ] `p1` - **ID**: `cpt-cf-oagw-nfr-low-latency`

The system **MUST** add less than 10ms overhead at p95 to proxied requests (excluding upstream response time). Plugin execution timeouts **MUST** be enforced to prevent latency degradation.

- **Threshold**: <10ms added latency at p95
- **Rationale**: OAGW is in the critical path of every outbound API call; excessive overhead directly impacts end-user experience
- **Architecture Allocation**: See DESIGN.md § NFR Allocation

#### High Availability

- [ ] `p1` - **ID**: `cpt-cf-oagw-nfr-high-availability`

The system **MUST** maintain 99.9% availability. Circuit breakers **MUST** prevent cascade failures from degraded upstreams.

- **Threshold**: 99.9% uptime (approximately 8.7 hours downtime per year)
- **Rationale**: OAGW is a shared platform service; its unavailability blocks all outbound API traffic
- **Architecture Allocation**: See DESIGN.md § NFR Allocation

#### Credential Isolation

- [ ] `p1` - **ID**: `cpt-cf-oagw-nfr-credential-isolation`

Credentials **MUST** never appear in logs, error messages, or client-facing responses. All credential references **MUST** use UUID references only. Credentials **MUST** be tenant-isolated.

- **Threshold**: Zero credential exposure in any observable output
- **Rationale**: API keys and tokens are high-value secrets; any exposure creates immediate security risk
- **Architecture Allocation**: See DESIGN.md § NFR Allocation

#### SSRF Protection

- [ ] `p1` - **ID**: `cpt-cf-oagw-nfr-ssrf-protection`

The system **MUST** implement SSRF protection including DNS validation, IP pinning, and header stripping to prevent requests to unintended internal services.

- **Threshold**: Zero successful SSRF attacks in penetration testing
- **Rationale**: OAGW accepts user-influenced URLs; without SSRF protection, attackers could reach internal services
- **Architecture Allocation**: See DESIGN.md § NFR Allocation

#### Input Validation

- [ ] `p1` - **ID**: `cpt-cf-oagw-nfr-input-validation`

The system **MUST** validate path, query parameters, headers, and body size on all incoming requests. Invalid requests **MUST** be rejected with 400 Bad Request.

- **Threshold**: All invalid inputs rejected before reaching upstream
- **Rationale**: Defense in depth — validates requests before forwarding to external services
- **Architecture Allocation**: See DESIGN.md § NFR Allocation

#### Multi-Tenancy

- [ ] `p1` - **ID**: `cpt-cf-oagw-nfr-multi-tenancy`

All resources (upstreams, routes, plugins, rate limit counters) **MUST** be tenant-scoped. Isolation **MUST** be enforced at the data layer.

- **Threshold**: Zero cross-tenant data access in security audit
- **Rationale**: CyberFabric is a multi-tenant platform; tenant isolation is a foundational security requirement
- **Architecture Allocation**: See DESIGN.md § NFR Allocation

#### Observability

- [ ] `p2` - **ID**: `cpt-cf-oagw-nfr-observability`

The system **MUST** provide request logs with correlation IDs for distributed tracing and Prometheus metrics for monitoring proxy throughput, latency, error rates, and rate limit utilization.

- **Threshold**: All proxied requests logged with correlation ID; metrics available within 10s of request completion
- **Rationale**: Operators need visibility into proxy traffic patterns for troubleshooting and capacity planning
- **Architecture Allocation**: See DESIGN.md § NFR Allocation

#### Starlark Sandbox

- [ ] `p2` - **ID**: `cpt-cf-oagw-nfr-starlark-sandbox`

The Starlark execution environment **MUST** prohibit network I/O, file I/O, and imports. Execution **MUST** be constrained by timeout and memory limits.

- **Threshold**: Zero sandbox escapes in security testing
- **Rationale**: User-provided scripts must not compromise gateway security or stability
- **Architecture Allocation**: See DESIGN.md § NFR Allocation

### 6.2 NFR Exclusions

No project-default NFR exclusions. All project-wide NFRs apply to this module.

## 7. Public Library Interfaces

### 7.1 Public API Surface

#### Management API

- [ ] `p1` - **ID**: `cpt-cf-oagw-interface-management-api`

- **Type**: REST API
- **Stability**: stable
- **Description**: CRUD endpoints for upstream, route, and plugin management. Endpoints: `POST/GET/PUT/DELETE /api/oagw/v1/upstreams[/{id}]`, `POST/GET/PUT/DELETE /api/oagw/v1/routes[/{id}]`, `POST/GET/DELETE /api/oagw/v1/plugins[/{id}]`, `GET /api/oagw/v1/plugins/{id}/source`.
- **Breaking Change Policy**: Major version bump required for endpoint removal or request/response schema incompatible changes

#### Proxy API

- [ ] `p1` - **ID**: `cpt-cf-oagw-interface-proxy-api`

- **Type**: REST API
- **Stability**: stable
- **Description**: Single proxy endpoint that accepts any HTTP method: `{METHOD} /api/oagw/v1/proxy/{alias}[/{path}][?{query}]`. Supports HTTP request/response, SSE, WebSocket, and WebTransport.
- **Breaking Change Policy**: URL structure changes require major version bump

#### OAGW SDK Traits

- [ ] `p1` - **ID**: `cpt-cf-oagw-interface-sdk`

- **Type**: Rust module (oagw-sdk crate)
- **Stability**: unstable
- **Description**: Public traits for external plugin implementation. Defines Auth, Guard, and Transform plugin interfaces that external ModKit modules implement.
- **Breaking Change Policy**: SDK is unstable during initial development; breaking changes expected until v1.0

### 7.2 External Integration Contracts

#### Plugin Type Contracts

- [ ] `p1` - **ID**: `cpt-cf-oagw-contract-plugin-types`

- **Direction**: provided by library
- **Protocol/Format**: GTS type identifiers: `gts.x.core.oagw.auth_plugin.v1~*`, `gts.x.core.oagw.guard_plugin.v1~*`, `gts.x.core.oagw.transform_plugin.v1~*`
- **Compatibility**: Plugin type schemas versioned via GTS; backward-compatible additions are minor changes

#### Error Response Contract

- [ ] `p1` - **ID**: `cpt-cf-oagw-contract-error-response`

- **Direction**: provided by library
- **Protocol/Format**: RFC 9457 Problem Details JSON with standard HTTP error codes:

| HTTP | Error | Retriable |
|------|-------|-----------|
| 400 | ValidationError | No |
| 401 | AuthenticationFailed | No |
| 404 | RouteNotFound | No |
| 413 | PayloadTooLarge | No |
| 429 | RateLimitExceeded | Yes |
| 500 | SecretNotFound | No |
| 502 | DownstreamError | Depends |
| 503 | CircuitBreakerOpen | Yes |
| 504 | Timeout | Yes |

- **Compatibility**: Error codes are stable; new error codes are additive changes

#### Credential Store Integration

- [ ] `p1` - **ID**: `cpt-cf-oagw-contract-credential-store`

- **Direction**: required from client (credential store service)
- **Protocol/Format**: In-process ClientHub API for secret retrieval by UUID reference. Supports hierarchical resolution across tenant hierarchy.
- **Compatibility**: Depends on `cred_store` SDK stability

#### Types Registry Integration

- [ ] `p2` - **ID**: `cpt-cf-oagw-contract-types-registry`

- **Direction**: required from client (types registry service)
- **Protocol/Format**: In-process ClientHub API for GTS schema and instance registration and validation
- **Compatibility**: Depends on `types_registry` SDK stability

## 8. Use Cases

#### Proxy HTTP Request

- [ ] `p1` - **ID**: `cpt-cf-oagw-usecase-proxy-request`

**Actor**: `cpt-cf-oagw-actor-app-developer`

**Preconditions**:
- Upstream is configured and enabled
- Route matching the request exists and is enabled
- Authentication credentials are stored in credential store

**Main Flow**:
1. Application sends request to `/api/oagw/v1/proxy/{alias}/{path}`
2. System resolves upstream by alias (searching tenant hierarchy from descendant to root)
3. System matches route by method and path
4. System merges configurations (upstream < route < tenant)
5. System retrieves credentials from credential store
6. System executes plugin chain: Auth → Guards → Transform(request)
7. System forwards transformed request to upstream service
8. System executes Transform(response) plugins on the response
9. System returns response to the calling application

**Postconditions**:
- Response from upstream service delivered to caller
- Request logged with correlation ID for audit trail

**Alternative Flows**:
- **Upstream not found**: System returns 404 RouteNotFound
- **Upstream disabled**: System returns 503 Service Unavailable
- **Route not matched**: System returns 404 RouteNotFound
- **Auth plugin fails**: System returns 401 AuthenticationFailed
- **Guard rejects**: System returns the guard's rejection status code
- **Rate limit exceeded**: System returns 429 with Retry-After header (reject strategy), queues the request (queue strategy), or degrades the response (degrade strategy)
- **Upstream timeout**: System returns 504 Timeout
- **Upstream error**: System returns 502 DownstreamError
- **Circuit breaker open**: System returns 503 CircuitBreakerOpen

#### Configure Upstream

- [ ] `p1` - **ID**: `cpt-cf-oagw-usecase-configure-upstream`

**Actor**: `cpt-cf-oagw-actor-platform-operator`

**Preconditions**:
- Operator is authenticated with appropriate permissions

**Main Flow**:
1. Operator sends POST to `/api/oagw/v1/upstreams` with server endpoints, protocol, auth config, headers, and rate limits
2. System validates the upstream configuration (endpoints compatibility, alias uniqueness within tenant)
3. System persists the upstream configuration
4. System returns the created upstream with generated ID

**Postconditions**:
- Upstream is available for route binding and proxy resolution

**Alternative Flows**:
- **Validation fails**: System returns 400 ValidationError with details

#### Configure Route

- [ ] `p1` - **ID**: `cpt-cf-oagw-usecase-configure-route`

**Actor**: `cpt-cf-oagw-actor-platform-operator`

**Preconditions**:
- Referenced upstream exists
- Operator is authenticated with appropriate permissions

**Main Flow**:
1. Operator sends POST to `/api/oagw/v1/routes` with upstream_id and match rules (method, path, query allowlist)
2. System validates the upstream reference and match rules
3. System persists the route configuration
4. System returns the created route with generated ID

**Postconditions**:
- Route is active and participates in request matching

**Alternative Flows**:
- **Upstream not found**: System returns 400 ValidationError
- **Validation fails**: System returns 400 ValidationError with details

#### Rate Limit Exceeded

- [ ] `p2` - **ID**: `cpt-cf-oagw-usecase-rate-limit`

**Actor**: `cpt-cf-oagw-actor-app-developer`

**Preconditions**:
- Rate limit is configured on the upstream or route
- Request count has reached or exceeded the configured limit within the current window

**Main Flow**:
1. Application sends request to proxy endpoint
2. System resolves upstream and route
3. System evaluates rate limit for the applicable scope (global, tenant, user, or IP)
4. System determines limit is exceeded
5. Based on configured strategy:
   - **Reject**: System returns 429 with Retry-After header
   - **Queue**: System queues the request for later execution
   - **Degrade**: System forwards with degraded quality of service

**Postconditions**:
- Request handled according to rate limit strategy
- Rate limit counter updated

**Alternative Flows**:
- **Hierarchical enforcement**: Ancestor's enforced rate limit applies even if descendant's own limit is not exceeded; effective limit is `min(ancestor.enforced, descendant)`

#### SSE Streaming

- [ ] `p2` - **ID**: `cpt-cf-oagw-usecase-sse-streaming`

**Actor**: `cpt-cf-oagw-actor-app-developer`

**Preconditions**:
- Upstream supports SSE responses
- Route is configured for the upstream

**Main Flow**:
1. Application sends request to proxy endpoint
2. System resolves upstream, matches route, applies auth and plugins
3. System establishes connection to upstream
4. System forwards SSE events to client as received
5. System maintains connection lifecycle (open, data, error, close events)

**Postconditions**:
- All upstream events delivered to client
- Connection cleanly closed

**Alternative Flows**:
- **Upstream connection drops**: System propagates close event to client
- **Client disconnects**: System closes upstream connection

## 9. Acceptance Criteria

- [ ] Proxy requests are forwarded to correct upstream with credentials injected and response returned to caller
- [ ] Upstream, route, and plugin CRUD operations succeed with valid input and return appropriate errors for invalid input
- [ ] Rate limiting enforces configured limits and returns 429 with Retry-After header when exceeded (reject strategy)
- [ ] Disabled upstreams return 503; disabled routes are excluded from matching
- [ ] Hierarchical configuration override applies sharing modes correctly (private/inherit/enforce)
- [ ] Alias resolution searches tenant hierarchy from descendant to root; closest match wins
- [ ] Plugin chain executes in correct order: Auth → Guards → Transform(request) → Upstream → Transform(response/error)
- [ ] Credentials never appear in logs, error messages, or client responses
- [ ] SSRF protection blocks requests to internal/private IP ranges
- [ ] SSE, WebSocket, and WebTransport streams are proxied without buffering
- [ ] Circuit breaker opens after configured failure threshold and returns 503
- [ ] Added proxy latency is less than 10ms at p95

## 10. Dependencies

| Dependency | Description | Criticality |
|------------|-------------|-------------|
| `cred_store` | Secure secret retrieval by UUID reference for credential injection | p1 |
| `types_registry` | GTS schema and instance registration for plugin type validation | p1 |
| `api_ingress` | REST API hosting for management and proxy endpoints | p1 |
| `modkit-db` | Database persistence for upstream, route, and plugin configurations | p1 |
| `modkit-auth` | Authorization enforcement for management API operations | p1 |

## 11. Assumptions

- Credential store is available and operational before OAGW processes proxy requests requiring authentication
- Tenant hierarchy is resolvable via tenant resolver for hierarchical configuration and alias resolution
- External upstream services are reachable from the OAGW network environment (egress firewall rules are configured)
- Plugin execution completes within configured timeout limits; plugins that exceed limits are terminated

## 12. Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| High proxy latency under load | Degraded end-user experience for all outbound API calls | Enforce <10ms p95 budget; benchmark under load; circuit breaker to shed traffic from degraded upstreams |
| Credential exposure via logging or error messages | Security breach; compromised API keys/tokens | UUID-only references; audit logging pipeline; automated credential leak detection |
| SSRF exploitation via crafted alias or upstream config | Unauthorized access to internal services | DNS validation, IP pinning, private IP range blocking, header stripping |
| Plugin timeout or crash degrading gateway throughput | Increased latency or gateway unavailability | Per-plugin timeout enforcement; plugin isolation; circuit breaker on plugin failures |
| Rate limit bypass via scope manipulation | Cost overruns from uncapped external API usage | Server-side scope enforcement; hierarchical rate limit inheritance with `enforce` mode |
| Starlark sandbox escape | Arbitrary code execution in gateway context | No network/file I/O, no imports, enforced timeout and memory limits, security testing |

## 13. Open Questions

- What is the maximum number of endpoints per upstream pool before load balancing performance degrades?
- What is the default circuit breaker threshold (failure count/percentage and recovery window)?
- How are plugin execution metrics exposed for monitoring (per-plugin latency, error rates)?
- What is the maximum body size for proxied requests before rejection?
- How are WebTransport sessions authenticated and authorized differently from HTTP requests?

## 14. Traceability

- **Design**: [DESIGN.md](./DESIGN.md)
- **ADRs**: [ADR/](./ADR/)
- **Features**: [features/](./features/)
