# Technical Design — Outbound API Gateway (OAGW)

**ID**: `cpt-cf-oagw-design-oagw`

## 1. Architecture Overview

### 1.1 Architectural Vision

OAGW is implemented as a single ModKit module (`oagw` crate) with internal service isolation via domain traits. The architecture separates concerns into a Control Plane for configuration management and a Data Plane for proxy request orchestration, both wired in-process through trait-based dependency injection within a single-executable deployment.

The module follows DDD-Light layering: a transport layer (`api/rest/`) handles HTTP concerns, a domain layer (`domain/`) owns business logic and repository trait definitions, and an infrastructure layer (`infra/`) provides concrete implementations for persistence, HTTP proxying, and plugin execution. A companion `oagw-sdk` crate exposes the public API surface (traits, models, errors) consumed by external modules and plugin authors.

This architecture satisfies the PRD requirements by centralizing all outbound API traffic through a single proxy layer with pluggable authentication, configurable rate limiting, header transformation, and hierarchical multi-tenant configuration. The plugin system enables extensibility without modifying core gateway logic, while the Control Plane / Data Plane separation allows independent evolution of configuration management and request processing.

### 1.2 Architecture Drivers

Requirements that significantly influence architecture decisions.

#### Functional Drivers

| Requirement | Design Response |
|-------------|-----------------|
| `cpt-cf-oagw-fr-upstream-mgmt` | ControlPlaneService provides CRUD operations for upstream configurations via tenant-scoped repositories with alias uniqueness constraints |
| `cpt-cf-oagw-fr-route-mgmt` | ControlPlaneService provides CRUD operations for routes with method/path/query match rules, linked to upstreams via foreign key |
| `cpt-cf-oagw-fr-enable-disable` | Boolean `enabled` field on upstreams and routes with hierarchical inheritance; ancestor disable propagates to all descendants |
| `cpt-cf-oagw-fr-request-proxy` | DataPlaneService orchestrates alias resolution, route matching, config merge, credential retrieval, plugin chain execution, and HTTP forwarding |
| `cpt-cf-oagw-fr-auth-injection` | Auth plugin subsystem with built-in plugins (API Key, Basic, Bearer, OAuth2) retrieves credentials from `cred_store` by UUID reference at request time |
| `cpt-cf-oagw-fr-rate-limiting` | Token bucket rate limiter at upstream and route levels with configurable scope (global/tenant/user/IP), strategy (reject/queue/degrade), and hierarchical min-merge |
| `cpt-cf-oagw-fr-config-layering` | Configuration merge with priority order: upstream (base) < route < tenant; higher-priority level wins for same setting |
| `cpt-cf-oagw-fr-config-hierarchy` | Three sharing modes (private/inherit/enforce) with field-specific merge strategies: auth override, rate limit min-merge, plugin chain append, tags additive union |
| `cpt-cf-oagw-fr-alias-resolution` | Alias resolution walks tenant hierarchy from descendant to root; closest match wins (shadowing). Multi-endpoint pooling with round-robin distribution |
| `cpt-cf-oagw-fr-plugin-system` | Three plugin types (Auth, Guard, Transform) with defined execution order, built-in and external (Starlark) categories, GTS type identification |
| `cpt-cf-oagw-fr-plugin-immutability` | Custom plugins are immutable after creation; updates via new version + re-binding; GC of unlinked plugins after TTL |
| `cpt-cf-oagw-fr-plugin-crud` | Management API for plugin create, get, list, delete with source retrieval endpoint; delete blocked when plugin is referenced |
| `cpt-cf-oagw-fr-header-transform` | Header transformation (set/add/remove) on requests and responses; hop-by-hop stripping; passthrough control |
| `cpt-cf-oagw-fr-streaming` | Streaming support for HTTP request/response, SSE, WebSocket, and WebTransport with connection lifecycle management |
| `cpt-cf-oagw-fr-circuit-breaker` | Circuit breaker as core gateway resilience policy (not a plugin); returns 503 when open |

#### NFR Allocation

| NFR ID | NFR Summary | Allocated To | Design Response | Verification Approach |
|--------|-------------|--------------|-----------------|----------------------|
| `cpt-cf-oagw-nfr-low-latency` | <10ms added latency at p95 | DataPlaneService, HTTP client, plugin chain | Connection pooling, in-memory config caching, bounded plugin execution timeouts, streaming without buffering | Load testing with p95 latency assertions |
| `cpt-cf-oagw-nfr-high-availability` | 99.9% gateway availability | Circuit breaker, graceful shutdown, health checks | Circuit breaker prevents cascade failures; graceful shutdown drains in-flight requests; health/readiness endpoints | Uptime monitoring, chaos testing under SIGTERM |
| `cpt-cf-oagw-nfr-credential-isolation` | Zero credential exposure | Auth plugin subsystem, audit logging | UUID-only credential references; secrets retrieved at request time from `cred_store`; redaction in all logs and error responses | Automated credential leak detection in logs and responses |
| `cpt-cf-oagw-nfr-ssrf-protection` | Zero successful SSRF attacks | Request validation, DNS validation, header stripping | Scheme allowlist enforcement; private IP range blocking; `Host` header replacement; `X-OAGW-Target-Host` validation against endpoint allowlist | Penetration testing, automated SSRF test suite |
| `cpt-cf-oagw-nfr-input-validation` | All invalid inputs rejected before upstream | Guard rules, body validation | Method/path/query validation against route config; Content-Length validation; 100MB hard body limit; Transfer-Encoding validation | Unit tests for all validation paths |
| `cpt-cf-oagw-nfr-multi-tenancy` | Zero cross-tenant data access | Secure ORM, SecurityContext propagation | All DB access via `SecureConn` with tenant scoping; deny-by-default on empty scopes; alias uniqueness per tenant | Repository tests proving cross-tenant isolation |
| `cpt-cf-oagw-nfr-observability` | Correlation IDs and metrics within 10s | Metrics plugin, audit logging, Prometheus endpoint | Structured JSON audit logs with request_id/tenant_id; Prometheus counters/histograms/gauges for requests, errors, circuit breaker, rate limits | Metric presence smoke tests, log format validation |
| `cpt-cf-oagw-nfr-starlark-sandbox` | Zero sandbox escapes | Starlark runtime | No network/file I/O, no imports; enforced timeout and memory limits; `ctx` API with safe mutators only | Security testing for sandbox escape attempts |

#### Key ADRs

| ADR ID | Decision Summary |
|--------|-----------------|
| `cpt-cf-oagw-adr-component-architecture` | Single module with internal Control Plane / Data Plane service isolation via domain traits (superseded) |
| `cpt-cf-oagw-adr-request-routing` | Alias-based routing with tenant hierarchy walk, longest-path-prefix matching, priority ordering |
| `cpt-cf-oagw-adr-plugin-system` | Three plugin types (Auth/Guard/Transform) with defined execution order and built-in + external categories |
| `cpt-cf-oagw-adr-rate-limiting` | Token bucket algorithm with sustained rate, burst capacity, multiple scopes, and hierarchical min-merge |
| `cpt-cf-oagw-adr-circuit-breaker` | Circuit breaker as core policy with states (closed/half-open/open), configurable thresholds, and fallback behavior |
| `cpt-cf-oagw-adr-error-source-distinction` | `X-OAGW-Error-Source` header distinguishes gateway errors from upstream passthrough errors |
| `cpt-cf-oagw-adr-data-plane-caching` | In-memory config caching with bounded size, TTL, and invalidation on management writes |
| `cpt-cf-oagw-adr-state-management` | State management strategy for rate limiters, circuit breakers, and connection pools |
| `cpt-cf-oagw-adr-resource-identification` | Anonymous GTS identifiers in API, UUIDs in database; alias generation and uniqueness rules |
| `cpt-cf-oagw-adr-cors` | CORS preflight handled locally (no upstream round-trip); configurable per upstream/route |
| `cpt-cf-oagw-adr-concurrency-control` | Per-scope in-flight request limits with local limiter and optional distributed coordinator |
| `cpt-cf-oagw-adr-backpressure-queueing` | Bounded queue with fixed worker pool, cooperative cancellation, and selectable degradation strategies |
| `cpt-cf-oagw-adr-grpc-support` | gRPC proxying via HTTP/2 multiplexing with content-type detection (future phase, requires prototype) |
| `cpt-cf-oagw-adr-rust-abi-client-library` | HTTP client abstractions, streaming support, and plugin development APIs for the `oagw-sdk` crate |

### 1.3 Architecture Layers

```
┌─────────────────────────────────────────────────────────┐
│                    oagw-sdk (Public API)                 │
│  ServiceGatewayClientV1 trait, models, errors            │
├─────────────────────────────────────────────────────────┤
│                    Transport Layer                        │
│  api/rest/ — Axum handlers, DTOs, OperationBuilder       │
├─────────────────────────────────────────────────────────┤
│                    Domain Layer                           │
│  ControlPlaneService, DataPlaneService traits & impls     │
│  AuthPlugin/GuardPlugin/TransformPlugin trait definitions │
│  Repository traits, DomainError, ProxyContext             │
├─────────────────────────────────────────────────────────┤
│                  Infrastructure Layer                     │
│  SeaORM repositories, reqwest HTTP client,                │
│  AuthPluginRegistry, Starlark sandbox, GTS provisioning   │
└─────────────────────────────────────────────────────────┘
```

| Layer | Responsibility | Technology |
|-------|---------------|------------|
| SDK | Public API surface: traits, models, errors for external consumers and plugin authors | Rust crate (`oagw-sdk`) |
| Transport | HTTP request/response handling, DTO serialization, route registration, error mapping | Axum, serde, utoipa, OperationBuilder |
| Domain | Business logic, service traits, repository trait definitions, plugin trait definitions, domain errors | Rust traits, DDD-Light |
| Infrastructure | Concrete implementations: persistence, HTTP proxying, plugin execution, GTS registration | SeaORM, reqwest, Starlark, DashMap |

**ID**: `cpt-cf-oagw-tech-layers`

## 2. Principles & Constraints

### 2.1 Design Principles

#### No Automatic Retries

- [ ] `p1` - **ID**: `cpt-cf-oagw-principle-no-retries`

Each inbound request results in at most one upstream attempt. OAGW does not retry failed requests. Retry logic is the client's responsibility. Auth plugins may handle token refresh on 401 but do not replay the original request.

**ADRs**: `cpt-cf-oagw-adr-request-routing`

#### Credential Isolation

- [ ] `p1` - **ID**: `cpt-cf-oagw-principle-credential-isolation`

Credentials are never stored in OAGW configuration. All credential references use UUID references to `cred_store`. Secrets are retrieved at request time and never appear in logs, error messages, or client-facing responses.

**ADRs**: `cpt-cf-oagw-adr-component-architecture`

#### Plugin Immutability

- [ ] `p1` - **ID**: `cpt-cf-oagw-principle-plugin-immutability`

Custom plugin definitions are immutable after creation. Updates are performed by creating a new plugin version and re-binding upstream/route references. This guarantees deterministic behavior for attached configurations and improves auditability.

**ADRs**: `cpt-cf-oagw-adr-plugin-system`

#### Configuration Layering

- [ ] `p1` - **ID**: `cpt-cf-oagw-principle-config-layering`

Configuration merges follow a defined priority order: upstream (base) < route < tenant (highest priority). When the same setting is defined at multiple levels, the higher-priority level wins. Hierarchical sharing modes (private/inherit/enforce) control visibility and override behavior across the tenant hierarchy.

**ADRs**: `cpt-cf-oagw-adr-resource-identification`

#### Alias-Per-Tenant Uniqueness

- [ ] `p1` - **ID**: `cpt-cf-oagw-principle-alias-per-tenant`

Upstream aliases are unique per tenant, not globally. This enables tenant isolation, hierarchical override via shadowing, and eliminates cross-tenant coordination for upstream creation.

**ADRs**: `cpt-cf-oagw-adr-resource-identification`

#### Plugin Chain Composition

- [ ] `p1` - **ID**: `cpt-cf-oagw-principle-plugin-chain-composition`

Plugin chains are composed by concatenating upstream plugins before route plugins. Execution follows a defined order: Auth → Guards → Transform(request) → Upstream call → Transform(response/error). Enforced ancestor plugins cannot be removed by descendants.

**ADRs**: `cpt-cf-oagw-adr-plugin-system`

#### Error Source Distinction

- [ ] `p1` - **ID**: `cpt-cf-oagw-principle-error-source-distinction`

All error responses include an `X-OAGW-Error-Source` header (`gateway` or `upstream`) to distinguish OAGW-generated errors from upstream passthrough errors. Gateway errors use RFC 9457 Problem Details format; upstream errors are passed through unmodified.

**ADRs**: `cpt-cf-oagw-adr-error-source-distinction`

### 2.2 Constraints

#### ModKit Module Framework

- [ ] `p1` - **ID**: `cpt-cf-oagw-constraint-modkit`

OAGW is implemented as a ModKit module, requiring adherence to the ModKit module lifecycle, REST registration via OperationBuilder, SecurityContext propagation, and Secure ORM for all database access. Single-executable deployment via the ModKit framework.

#### Outbound Traffic Only

- [ ] `p1` - **ID**: `cpt-cf-oagw-constraint-outbound-only`

OAGW handles outbound (north-south) traffic only. Service mesh and east-west traffic management are out of scope. All proxied requests flow from CyberFabric modules to external services.

#### Starlark Sandbox Restrictions

- [ ] `p2` - **ID**: `cpt-cf-oagw-constraint-starlark-sandbox`

The Starlark execution environment for custom plugins prohibits network I/O, file I/O, and imports. Execution is constrained by timeout and memory limits. Only JSON manipulation, string/math operations, logging, and time utilities are permitted.

**ADRs**: `cpt-cf-oagw-adr-plugin-system`

#### Secure ORM Mandatory

- [ ] `p1` - **ID**: `cpt-cf-oagw-constraint-secure-orm`

All database access uses SeaORM with `SecureConn` tenant scoping. No raw SQL in production code. SQL in design documentation is illustrative only. Empty security scopes result in deny-all behavior.

#### GTS Type Identification

- [ ] `p1` - **ID**: `cpt-cf-oagw-constraint-gts-types`

All resources use anonymous GTS identifiers in the REST API (e.g., `gts.x.core.oagw.upstream.v1~{uuid}`) but store UUIDs in the database. Built-in plugins use named GTS identifiers. Plugin type schemas are versioned via GTS.

## 3. Technical Architecture

### 3.1 Domain Model

**Technology**: GTS type identifiers, Rust structs, JSON Schema

**Location**: [docs/schemas/](./schemas/)

**Core Entities**:

| Entity | Description | Schema |
|--------|-------------|--------|
| Upstream | External service target with server endpoints, protocol, auth, headers, plugins, rate limits, and sharing configuration | [upstream.v1.schema.json](./schemas/upstream.v1.schema.json) |
| Route | Request matching rule (method, path, query allowlist) mapped to an upstream with plugins and rate limits | [route.v1.schema.json](./schemas/route.v1.schema.json) |
| Auth Plugin | Credential injection plugin (API key, Basic, Bearer, OAuth2). One per upstream. | [auth_plugin.v1.schema.json](./schemas/auth_plugin.v1.schema.json) |
| Guard Plugin | Validation and policy enforcement plugin. Can reject requests. Multiple per upstream/route. | [guard_plugin.v1.schema.json](./schemas/guard_plugin.v1.schema.json) |
| Transform Plugin | Request/response mutation plugin. Multiple per upstream/route. Phases: on_request, on_response, on_error. | [transform_plugin.v1.schema.json](./schemas/transform_plugin.v1.schema.json) |

**GTS Type Identifiers**:

| Resource | Base Type |
|----------|-----------|
| Upstream | `gts.x.core.oagw.upstream.v1~` |
| Route | `gts.x.core.oagw.route.v1~` |
| Auth Plugin | `gts.x.core.oagw.auth_plugin.v1~` |
| Guard Plugin | `gts.x.core.oagw.guard_plugin.v1~` |
| Transform Plugin | `gts.x.core.oagw.transform_plugin.v1~` |
| Protocol (HTTP) | `gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1` |

**Relationships**:

- Upstream → Route: one-to-many (routes reference upstream via `upstream_id`)
- Upstream → Auth Plugin: one-to-one (single auth plugin per upstream)
- Upstream/Route → Guard/Transform Plugins: many-to-many (plugin references stored as GTS identifier arrays in JSONB)
- Plugin references are GTS identifiers; built-in plugins resolve to hardcoded Rust implementations, custom plugins resolve to `oagw_plugin` table rows by UUID extraction

**Resource Identification Pattern**:

| Resource | API Identifier | Database |
|----------|---------------|----------|
| Upstream | `gts.x.core.oagw.upstream.v1~{uuid}` | UUID |
| Route | `gts.x.core.oagw.route.v1~{uuid}` | UUID |
| Plugin | `gts.x.core.oagw.{type}_plugin.v1~{uuid}` | UUID |
| Built-in Plugin | `gts.x.core.oagw.{type}_plugin.v1~x.core.oagw.{name}.v1` | Not stored |

### 3.2 Component Model

```
┌──────────────────────────────────────────────────────────────────┐
│                        Transport Layer                            │
│  ┌──────────────────┐  ┌──────────────────┐                      │
│  │ Management       │  │ Proxy            │                      │
│  │ Handlers         │  │ Handler          │                      │
│  └────────┬─────────┘  └────────┬─────────┘                      │
├───────────┼──────────────────────┼────────────────────────────────┤
│           │     Domain Layer     │                                │
│           ▼                      ▼                                │
│  ┌──────────────────┐  ┌──────────────────┐                      │
│  │ ControlPlane     │  │ DataPlane        │                      │
│  │ Service          │◄─│ Service          │                      │
│  └────────┬─────────┘  └────────┬─────────┘                      │
│           │                      │                                │
│           │              ┌───────┼───────┐                        │
│           │              │       │       │                        │
├───────────┼──────────────┼───────┼───────┼────────────────────────┤
│           │  Infra Layer │       │       │                        │
│           ▼              ▼       ▼       ▼                        │
│  ┌──────────────┐ ┌──────────┐ ┌─────┐ ┌──────────┐             │
│  │ Repositories │ │ Plugin   │ │HTTP │ │ Starlark │             │
│  │ (SeaORM)     │ │ Registry │ │Client│ │ Sandbox  │             │
│  └──────────────┘ └──────────┘ └─────┘ └──────────┘             │
└──────────────────────────────────────────────────────────────────┘
```

#### Control Plane Service

- [ ] `p1` - **ID**: `cpt-cf-oagw-component-control-plane`

##### Why this component exists

Manages all configuration data (upstreams, routes, plugins) and provides resolution APIs consumed by the Data Plane during proxy operations.

##### Responsibility scope

CRUD operations for upstreams, routes, and plugins. Alias resolution with tenant hierarchy walk. Route matching by method, path, and priority. Effective configuration resolution with hierarchical merge. Plugin reference validation.

##### Responsibility boundaries

Does not execute proxy requests, perform HTTP calls to external services, or run plugin logic. Does not manage credentials directly (delegates to `cred_store`).

##### Related components (by ID)

- `cpt-cf-oagw-component-data-plane` — called by (config resolution during proxy)
- `cpt-cf-oagw-component-repositories` — depends on (persistence)

#### Data Plane Service

- [ ] `p1` - **ID**: `cpt-cf-oagw-component-data-plane`

##### Why this component exists

Orchestrates the end-to-end proxy request lifecycle: resolves configuration, executes the plugin chain, builds outbound requests, and forwards them to external services.

##### Responsibility scope

Proxy request orchestration. Plugin chain execution (Auth → Guards → Transform). Outbound HTTP request construction and forwarding. Response streaming. Error mapping with `X-OAGW-Error-Source` distinction. Rate limit evaluation. Circuit breaker enforcement.

##### Responsibility boundaries

Does not manage configuration persistence (delegates to Control Plane). Does not implement individual plugin logic (delegates to Plugin Registry and Starlark Sandbox).

##### Related components (by ID)

- `cpt-cf-oagw-component-control-plane` — depends on (config resolution)
- `cpt-cf-oagw-component-plugin-registry` — depends on (plugin execution)
- `cpt-cf-oagw-component-http-client` — depends on (upstream calls)
- `cpt-cf-oagw-component-starlark-sandbox` — depends on (custom plugin execution)

#### Plugin Registry

- [ ] `p1` - **ID**: `cpt-cf-oagw-component-plugin-registry`

##### Why this component exists

Manages the registry of built-in plugins and resolves plugin references from GTS identifiers to executable plugin instances.

##### Responsibility scope

Built-in plugin registration (Auth: noop, apikey, basic, bearer, oauth2_client_cred, oauth2_client_cred_basic; Guard: timeout, cors; Transform: logging, metrics, request_id). Plugin identifier resolution: named GTS identifiers for built-ins, UUID extraction for custom plugins. Plugin chain composition (upstream plugins + route plugins).

##### Responsibility boundaries

Does not execute Starlark code (delegates to Starlark Sandbox). Does not persist plugin definitions (delegates to Repositories).

##### Related components (by ID)

- `cpt-cf-oagw-component-data-plane` — called by (plugin chain execution)
- `cpt-cf-oagw-component-starlark-sandbox` — depends on (custom plugin execution)
- `cpt-cf-oagw-component-repositories` — depends on (custom plugin lookup)

#### Repositories

- [ ] `p1` - **ID**: `cpt-cf-oagw-component-repositories`

##### Why this component exists

Provides tenant-scoped persistence for all OAGW configuration entities using Secure ORM.

##### Responsibility scope

SeaORM entity definitions with `#[derive(Scopable)]`. `SecureConn`-scoped query builders for upstream, route, and plugin tables. Tenant-scoped CRUD operations. Alias lookup with tenant hierarchy. Route matching queries. Plugin usage tracking and GC eligibility management.

##### Responsibility boundaries

Does not contain business logic. Does not use raw SQL. All queries go through SeaORM query builders with `SecureConn` tenant scoping.

##### Related components (by ID)

- `cpt-cf-oagw-component-control-plane` — called by (CRUD and resolution)
- `cpt-cf-oagw-component-plugin-registry` — called by (custom plugin lookup)

#### HTTP Client

- [ ] `p1` - **ID**: `cpt-cf-oagw-component-http-client`

##### Why this component exists

Manages outbound HTTP connections to upstream services with connection pooling, keepalive, and timeout enforcement.

##### Responsibility scope

Shared HTTP client with connection pooling. Timeout policy enforcement (connection, request, idle timeouts mapped to specific 504 error variants). Adaptive HTTP/2 negotiation with per-host capability caching (TTL 1h). Safe cancellation propagation on client disconnect. Streaming request/response forwarding without buffering.

##### Responsibility boundaries

Does not perform authentication (handled by Auth plugins before HTTP call). Does not manage configuration (receives resolved config from Data Plane).

##### Related components (by ID)

- `cpt-cf-oagw-component-data-plane` — called by (upstream HTTP calls)

#### Starlark Sandbox

- [ ] `p2` - **ID**: `cpt-cf-oagw-component-starlark-sandbox`

##### Why this component exists

Provides a secure execution environment for tenant-defined custom plugins written in Starlark.

##### Responsibility scope

Starlark script execution with `ctx` API (request/response/error/config/route/log/time). Sandbox enforcement: no network I/O, no file I/O, no imports. Timeout and memory limit enforcement. Log redaction and message size limits for `ctx.log.*`.

##### Responsibility boundaries

Does not manage plugin persistence or lifecycle. Does not resolve plugin references. Executes only the Starlark source provided by the Plugin Registry.

##### Related components (by ID)

- `cpt-cf-oagw-component-plugin-registry` — called by (custom plugin execution)
- `cpt-cf-oagw-component-data-plane` — called by (via Plugin Registry)

### 3.3 API Contracts

**Technology**: REST/OpenAPI
**Location**: Management and Proxy endpoints registered via ModKit OperationBuilder

**Endpoints Overview**:

| Method | Path | Description | Stability |
|--------|------|-------------|-----------|
| `POST` | `/api/oagw/v1/upstreams` | Create upstream | stable |
| `GET` | `/api/oagw/v1/upstreams` | List upstreams | stable |
| `GET` | `/api/oagw/v1/upstreams/{id}` | Get upstream by ID | stable |
| `PUT` | `/api/oagw/v1/upstreams/{id}` | Update upstream | stable |
| `DELETE` | `/api/oagw/v1/upstreams/{id}` | Delete upstream | stable |
| `POST` | `/api/oagw/v1/routes` | Create route | stable |
| `GET` | `/api/oagw/v1/routes` | List routes | stable |
| `GET` | `/api/oagw/v1/routes/{id}` | Get route by ID | stable |
| `PUT` | `/api/oagw/v1/routes/{id}` | Update route | stable |
| `DELETE` | `/api/oagw/v1/routes/{id}` | Delete route | stable |
| `POST` | `/api/oagw/v1/plugins` | Create plugin | stable |
| `GET` | `/api/oagw/v1/plugins` | List plugins | stable |
| `GET` | `/api/oagw/v1/plugins/{id}` | Get plugin by ID | stable |
| `DELETE` | `/api/oagw/v1/plugins/{id}` | Delete plugin | stable |
| `GET` | `/api/oagw/v1/plugins/{id}/source` | Get plugin Starlark source | stable |
| `{METHOD}` | `/api/oagw/v1/proxy/{alias}[/{path}][?{query}]` | Proxy request to upstream | stable |

#### Management API

- [ ] `p1` - **ID**: `cpt-cf-oagw-interface-management-api`

**Technology**: REST with JSON request/response bodies
**Data Format**: JSON; resource identifiers use anonymous GTS format (`gts.x.core.oagw.{type}.v1~{uuid}`)

List endpoints support OData query parameters: `$filter`, `$select`, `$orderby`, `$top` (default 50, max 100), `$skip`.

Plugins are immutable — no PUT/UPDATE endpoint. DELETE returns `409 Conflict` with `referenced_by` details when plugin is in use.

**Inbound Authentication**: Bearer token required on all endpoints.

**Management Permissions**:

| Permission | Description |
|------------|------------|
| `gts.x.core.oagw.upstream.v1~:{create;override;read;delete}` | Upstream CRUD |
| `gts.x.core.oagw.route.v1~:{create;override;read;delete}` | Route CRUD |
| `gts.x.core.oagw.auth_plugin.v1~:{create;read;delete}` | Auth plugin management |
| `gts.x.core.oagw.guard_plugin.v1~:{create;read;delete}` | Guard plugin management |
| `gts.x.core.oagw.transform_plugin.v1~:{create;read;delete}` | Transform plugin management |

#### Proxy API

- [ ] `p1` - **ID**: `cpt-cf-oagw-interface-proxy-api`

**Technology**: REST; supports HTTP request/response, SSE, WebSocket, WebTransport
**Data Format**: Passthrough (request/response bodies forwarded as-is)

Proxy endpoint: `{METHOD} /api/oagw/v1/proxy/{alias}[/{path_suffix}][?{query_parameters}]`

**Proxy Permission**: `gts.x.core.oagw.proxy.v1~:invoke`

**X-OAGW-Target-Host Behavior**:

| Scenario | Endpoints | Alias Type | Header Present | Behavior |
|----------|-----------|------------|----------------|----------|
| Single endpoint | 1 | Any | No | Route to endpoint |
| Single endpoint | 1 | Any | Yes | Validate and route (optional but validated) |
| Multi-endpoint | 2+ | Explicit | No | Round-robin load balancing |
| Multi-endpoint | 2+ | Explicit | Yes | Route to specific endpoint |
| Multi-endpoint | 2+ | Common suffix | No | 400 Bad Request (missing required header) |
| Multi-endpoint | 2+ | Common suffix | Yes | Route to specific endpoint |

#### Error Response Contract

- [ ] `p1` - **ID**: `cpt-cf-oagw-interface-error-contract`

**Technology**: RFC 9457 Problem Details (`application/problem+json`)

All gateway errors use GTS type identifiers and include `X-OAGW-Error-Source: gateway`. Upstream errors are passed through with `X-OAGW-Error-Source: upstream`.

**Error Types**:

| Error | HTTP | GTS Instance ID | Retriable |
|-------|------|-----------------|-----------|
| ValidationError | 400 | `gts.x.core.errors.err.v1~x.oagw.validation.error.v1` | No |
| MissingTargetHost | 400 | `gts.x.core.errors.err.v1~x.oagw.routing.missing_target_host.v1` | No |
| InvalidTargetHost | 400 | `gts.x.core.errors.err.v1~x.oagw.routing.invalid_target_host.v1` | No |
| UnknownTargetHost | 400 | `gts.x.core.errors.err.v1~x.oagw.routing.unknown_target_host.v1` | No |
| AuthenticationFailed | 401 | `gts.x.core.errors.err.v1~x.oagw.auth.failed.v1` | No |
| RouteNotFound | 404 | `gts.x.core.errors.err.v1~x.oagw.route.not_found.v1` | No |
| PluginInUse | 409 | `gts.x.core.errors.err.v1~x.oagw.plugin.in_use.v1` | No |
| PayloadTooLarge | 413 | `gts.x.core.errors.err.v1~x.oagw.payload.too_large.v1` | No |
| RateLimitExceeded | 429 | `gts.x.core.errors.err.v1~x.oagw.rate_limit.exceeded.v1` | Yes |
| SecretNotFound | 500 | `gts.x.core.errors.err.v1~x.oagw.secret.not_found.v1` | No |
| ProtocolError | 502 | `gts.x.core.errors.err.v1~x.oagw.protocol.error.v1` | No |
| DownstreamError | 502 | `gts.x.core.errors.err.v1~x.oagw.downstream.error.v1` | Depends |
| StreamAborted | 502 | `gts.x.core.errors.err.v1~x.oagw.stream.aborted.v1` | No |
| LinkUnavailable | 503 | `gts.x.core.errors.err.v1~x.oagw.link.unavailable.v1` | Yes |
| CircuitBreakerOpen | 503 | `gts.x.core.errors.err.v1~x.oagw.circuit_breaker.open.v1` | Yes |
| PluginNotFound | 503 | `gts.x.core.errors.err.v1~x.oagw.plugin.not_found.v1` | No |
| ConnectionTimeout | 504 | `gts.x.core.errors.err.v1~x.oagw.timeout.connection.v1` | Yes |
| RequestTimeout | 504 | `gts.x.core.errors.err.v1~x.oagw.timeout.request.v1` | Yes |
| IdleTimeout | 504 | `gts.x.core.errors.err.v1~x.oagw.timeout.idle.v1` | Yes |

#### OAGW SDK

- [ ] `p1` - **ID**: `cpt-cf-oagw-interface-sdk`

**Technology**: Rust crate (`oagw-sdk`)
**Data Format**: Rust traits and types

Public traits for external plugin implementation (Auth, Guard, Transform interfaces). SDK models for upstream, route, and plugin request/response types. Error types with Problem `type` identifiers. Stability: unstable during initial development; breaking changes expected until v1.0.

### 3.4 Internal Dependencies

| Dependency Module | Interface Used | Purpose |
|-------------------|----------------|---------|
| `cred_store` | ClientHub SDK client | Secure secret retrieval by UUID reference for credential injection |
| `types_registry` | ClientHub SDK client | GTS schema and instance registration for plugin type validation |
| `api_ingress` | ModKit REST hosting | REST API hosting for management and proxy endpoints |
| `modkit-db` | SeaORM + SecureConn | Database persistence for upstream, route, and plugin configurations |
| `modkit-auth` | SecurityContext extractors | Authorization enforcement for management API operations and proxy access |

**Dependency Rules** (per project conventions):

- No circular dependencies
- Always use SDK modules for inter-module communication
- No cross-category sideways deps except through contracts
- Only integration/adapter modules talk to external systems
- `SecurityContext` must be propagated across all in-process calls

### 3.5 External Dependencies

#### Upstream Services

| Dependency | Interface Used | Purpose |
|------------|---------------|---------|
| External APIs | HTTP/HTTPS, WebSocket, WebTransport | Target services for proxied outbound requests |

OAGW proxies requests to external third-party services (e.g., OpenAI, Stripe, cloud provider APIs). Communication uses the protocol configured on the upstream (HTTP/1.1, HTTP/2 with adaptive negotiation). OAGW does not retry failed requests.

#### Credential Store

- **Contract**: `cpt-cf-oagw-contract-credential-store`

| Dependency | Interface Used | Purpose |
|------------|---------------|---------|
| `cred_store` | In-process ClientHub API | Secret retrieval by UUID reference with hierarchical tenant resolution |

OAGW retrieves credentials at request time. `cred_store` handles access control, secret sharing across tenant hierarchy, rotation, revocation, and expiration policies.

#### Types Registry

- **Contract**: `cpt-cf-oagw-contract-types-registry`

| Dependency | Interface Used | Purpose |
|------------|---------------|---------|
| `types_registry` | In-process ClientHub API | GTS schema and instance registration and validation |

OAGW registers schemas for upstream, route, protocol, and plugin types during module initialization. Plugin type validation uses GTS to verify schema/type correctness.

### 3.6 Interactions & Sequences

#### Proxy Request Flow

**ID**: `cpt-cf-oagw-seq-proxy-request`

**Use cases**: `cpt-cf-oagw-usecase-proxy-request`

**Actors**: `cpt-cf-oagw-actor-app-developer`, `cpt-cf-oagw-actor-upstream-service`, `cpt-cf-oagw-actor-credential-store`

```
Client Request: GET /api/oagw/v1/proxy/openai/v1/chat/completions
│
└─→ Proxy Handler
    ├─ Extract SecurityContext
    ├─ Parse alias ("openai") and path suffix
    └─→ DataPlaneService
        ├─ ControlPlaneService.resolve_upstream("openai", tenant_id)
        │   └─ Walk tenant hierarchy, closest alias match wins
        ├─ ControlPlaneService.resolve_route(upstream_id, method, path)
        │   └─ Longest path prefix match, priority ordering
        ├─ ControlPlaneService.resolve_effective_config(tenant_id, upstream_id)
        │   └─ Merge: upstream < route < tenant (sharing modes)
        ├─ Auth Plugin: inject credentials (cred_store lookup)
        ├─ Guard Plugins: validate request (can reject)
        ├─ Transform Plugins: mutate outbound request
        ├─ HTTP Client: forward to upstream
        ├─ Transform Plugins: mutate response
        └─ Return response to client
```

**Description**: End-to-end proxy request lifecycle from alias resolution through plugin chain execution to upstream forwarding and response return.

#### Management Operations Flow

**ID**: `cpt-cf-oagw-seq-management-ops`

**Use cases**: `cpt-cf-oagw-usecase-configure-upstream`, `cpt-cf-oagw-usecase-configure-route`

**Actors**: `cpt-cf-oagw-actor-platform-operator`, `cpt-cf-oagw-actor-tenant-admin`

```
Client Request: POST /api/oagw/v1/upstreams
│
└─→ Management Handler
    ├─ Extract SecurityContext (authentication via modkit-auth)
    ├─ Parse and validate request DTO
    └─→ ControlPlaneService
        ├─ Extract tenant_id from SecurityContext
        ├─ Validate alias format + uniqueness within tenant
        ├─ Convert DTO → domain model
        ├─ Write to repository (SecureConn-scoped)
        └─ Return domain model → DTO → HTTP 201 response
```

**Description**: Configuration management operations flow through REST handlers to ControlPlaneService with tenant-scoped persistence.

#### Alias Resolution with Shadowing

**ID**: `cpt-cf-oagw-seq-alias-resolution`

**Use cases**: `cpt-cf-oagw-usecase-proxy-request`

**Actors**: `cpt-cf-oagw-actor-app-developer`

```
Alias: "vendor.com", Tenant: subsub-tenant

Walk hierarchy:
1. subsub-tenant upstreams → match found (shadowing winner)
2. sub-tenant upstreams    → (skipped, closer match exists)
3. root-tenant upstreams   → enforced constraints still apply

Result:
├─ Routing target: subsub-tenant's upstream
├─ Enforced ancestors: collect sharing="enforce" constraints
└─ Effective rate = min(selected_rate, all_ancestor_enforced_rates)
```

**Description**: Alias resolution walks tenant hierarchy from descendant to root. Closest match wins for routing, but ancestor `sharing: enforce` constraints remain active.

#### Hierarchical Configuration Resolution

**ID**: `cpt-cf-oagw-seq-config-resolution`

**Use cases**: `cpt-cf-oagw-usecase-proxy-request`

**Actors**: `cpt-cf-oagw-actor-tenant-admin`

```
Walk hierarchy root → child, merge per field:

Auth:     enforce → use ancestor's | inherit + descendant → descendant overrides
Rate:     min(ancestor.enforced, descendant) — always stricter
Plugins:  ancestor.items + descendant.items — append only
Tags:     union(ancestor_tags, descendant_tags) — add only
```

**Description**: Configuration fields merge according to their sharing mode. Auth supports override under `inherit`; rate limits always take the stricter value; plugins append; tags are additive union.

#### Plugin Chain Execution

**ID**: `cpt-cf-oagw-seq-plugin-chain`

**Use cases**: `cpt-cf-oagw-usecase-proxy-request`

**Actors**: `cpt-cf-oagw-actor-app-developer`, `cpt-cf-oagw-actor-upstream-service`

```
Plugin Chain: [U1, U2] (upstream) + [R1, R2] (route) = [U1, U2, R1, R2]

Execution:
1. Auth Plugin         → credential injection (one per upstream)
2. Guard Plugins       → validation/policy (can reject with error)
3. Transform (request) → mutate outbound request
4. Upstream Call        → HTTP forward to external service
5. Transform (response)→ mutate response on success
6. Transform (error)   → mutate error response on failure
```

**Description**: Plugin chain is composed at config-resolution time by concatenating upstream plugins before route plugins. Execution follows the defined phase order.

### 3.7 Database schemas & tables

OAGW uses three main tables for configuration storage, all tenant-scoped via `tenant_id`. SQL below is illustrative; production code uses SeaORM entities with `#[derive(Scopable)]` and `SecureConn`-scoped repositories.

#### Table: oagw_upstream

**ID**: `cpt-cf-oagw-dbtable-upstream`

**Schema**:

| Column | Type | Description |
|--------|------|-------------|
| id | UUID (PK) | Auto-generated primary key |
| tenant_id | UUID (FK → tenant) | Tenant scope |
| alias | VARCHAR(255) | Human-readable upstream identifier |
| tags | TEXT[] | Tenant-local discovery tags |
| server | JSONB | Server endpoints (scheme, host, port) |
| protocol | VARCHAR(100) | GTS protocol identifier |
| auth | JSONB | Auth plugin config with sharing mode |
| headers | JSONB | Header transformation rules |
| plugins | JSONB | Plugin references with sharing mode |
| rate_limit | JSONB | Rate limit config with sharing mode |
| enabled | BOOLEAN | Enable/disable flag (default: true) |
| created_at | TIMESTAMPTZ | Creation timestamp |
| updated_at | TIMESTAMPTZ | Last update timestamp |
| created_by | UUID (FK → principal) | Creator principal |
| updated_by | UUID (FK → principal) | Last updater principal |

**PK**: `id`

**Constraints**: `UNIQUE (tenant_id, alias)`

**Additional info**: Indexes on `tenant_id`, `alias`, `tags` (GIN), `(tenant_id, enabled)` partial index.

#### Table: oagw_route

**ID**: `cpt-cf-oagw-dbtable-route`

**Schema**:

| Column | Type | Description |
|--------|------|-------------|
| id | UUID (PK) | Auto-generated primary key |
| tenant_id | UUID (FK → tenant) | Tenant scope |
| upstream_id | UUID (FK → oagw_upstream, CASCADE) | Parent upstream |
| tags | TEXT[] | Categorization tags |
| match | JSONB | Match rules (HTTP: methods, path, query_allowlist, path_suffix_mode; gRPC: service, method) |
| plugins | JSONB | Plugin references |
| rate_limit | JSONB | Route-level rate limit config |
| enabled | BOOLEAN | Enable/disable flag (default: true) |
| priority | INTEGER | Higher priority routes match first (default: 0) |
| created_at | TIMESTAMPTZ | Creation timestamp |
| updated_at | TIMESTAMPTZ | Last update timestamp |
| created_by | UUID (FK → principal) | Creator principal |
| updated_by | UUID (FK → principal) | Last updater principal |

**PK**: `id`

**Constraints**: FK to `oagw_upstream` with CASCADE delete

**Additional info**: Indexes on `tenant_id`, `upstream_id`, `(tenant_id, enabled)` partial, `(upstream_id, priority DESC)`, partial index on HTTP path for route matching.

#### Table: oagw_plugin

**ID**: `cpt-cf-oagw-dbtable-plugin`

**Schema**:

| Column | Type | Description |
|--------|------|-------------|
| id | UUID (PK) | Auto-generated primary key |
| tenant_id | UUID (FK → tenant) | Tenant scope |
| name | VARCHAR(255) | Human-readable name |
| description | TEXT | Plugin description |
| plugin_type | VARCHAR(20) | Type: 'auth', 'guard', 'transform' |
| phases | TEXT[] | Supported phases (on_request, on_response, on_error) |
| config_schema | JSONB | JSON Schema for plugin configuration |
| source_code | TEXT | Starlark source code |
| enabled | BOOLEAN | Enable/disable flag (default: true) |
| last_used_at | TIMESTAMPTZ | Last time plugin was referenced |
| gc_eligible_at | TIMESTAMPTZ | GC eligibility timestamp (set when unlinked) |
| created_at | TIMESTAMPTZ | Creation timestamp |
| updated_at | TIMESTAMPTZ | Last update timestamp |
| created_by | UUID (FK → principal) | Creator principal |
| updated_by | UUID (FK → principal) | Last updater principal |

**PK**: `id`

**Constraints**: `UNIQUE (tenant_id, name)`, CHECK `plugin_type IN ('auth', 'guard', 'transform')`

**Additional info**: Indexes on `tenant_id`, `plugin_type`, `(tenant_id, enabled)` partial, `gc_eligible_at` partial (for GC job). Plugins are immutable — no update operations. Background GC job deletes plugins where `gc_eligible_at < NOW()`.

## 4. Additional context

### Phased Delivery

OAGW is delivered incrementally across five phases:

| Phase | Goal | Key Deliverables |
|-------|------|-----------------|
| p0 | MVP — OpenAI Integration Ready | Module scaffold, DB schema, upstream/route CRUD, basic proxy routing, API key auth, token bucket rate limiting, HTTP + SSE streaming, minimal error surface |
| p1 | Production-Ready Minimal | RFC 9457 errors everywhere, AuthN/Z + tenant scoping, Secure ORM hardening, HTTP client reliability, structured audit logging, Prometheus metrics, SSRF guardrails, E2E test suite |
| p2 | Scalability & Operational Maturity | Circuit breaker, concurrency control, backpressure queueing, multi-endpoint load balancing, HTTP/2 negotiation, config caching, graceful shutdown |
| p3 | Advanced Product Features / Enterprise | Tenant hierarchy awareness, alias shadowing, hierarchical sharing modes, plugin framework (Starlark), plugin CRUD + GC, built-in plugin suite, CORS, WebSocket/WebTransport, full OData |
| p4 | Nice-to-Have / Long Tail | TLS pinning, mTLS, distributed tracing (OpenTelemetry), gRPC proxying, WebTransport refinements, Starlark stdlib extensions, advanced metrics |

### Observability Architecture

OAGW exposes Prometheus metrics at an auth-protected `/metrics` endpoint covering request throughput, latency histograms, error counters, circuit breaker state, rate limit utilization, routing decisions, and upstream health. Cardinality is managed by excluding per-tenant labels, using upstream hostname instead of UUID, normalizing paths from route config, and grouping status codes by class (2xx/3xx/4xx/5xx).

Structured JSON audit logs are emitted to stdout for ingestion by centralized logging systems. Logs include request_id, tenant_id, principal_id, host, path, method, status, and duration. Request/response bodies, query parameters, and credentials are never logged.

### Security Architecture

**SSRF Protection**: Scheme allowlist enforcement (HTTPS for production), private IP range blocking, DNS validation, `Host` header replacement with upstream host, `X-OAGW-Target-Host` validation against endpoint allowlist, hop-by-hop header stripping.

**CORS**: Built-in support configured per upstream/route. Preflight OPTIONS requests handled locally without upstream round-trip. Secure defaults prohibit wildcard-with-credentials. See `cpt-cf-oagw-adr-cors`.

**HTTP Version Negotiation**: Adaptive per-host HTTP/2 detection via ALPN during TLS handshake with capability caching (TTL 1h). Fallback to HTTP/1.1 on failure. Cache keyed by endpoint + resolved IP. HTTP/3 (QUIC) is future work.

**Header Security**: Reject invalid header names/values, CR/LF injection, obs-fold. Reject multiple Content-Length, multiple Host, ambiguous CL/TE combinations. Strip internal steering headers before forwarding.

**Hierarchical Override Permissions**: Descendant override of inherited configurations is gated by explicit permissions (`oagw:upstream:bind`, `override_auth`, `override_rate`, `add_plugins`). Without permissions, descendants use ancestor configuration as-is.

### Not Applicable Sections

**Compliance Architecture (COMPL)**: Not applicable because OAGW does not directly handle regulated data (PII, financial records). Compliance requirements for data handled by upstream services are the responsibility of those services and the calling modules. Audit logging provides the compliance trail for OAGW operations.

**Privacy Architecture (COMPL)**: Not applicable because OAGW proxies requests without inspecting or storing request/response bodies. PII handling is the responsibility of calling modules and upstream services. OAGW enforces credential isolation and never logs bodies.

**User-Facing Architecture (UX)**: Not applicable because OAGW is a backend infrastructure service with no user-facing frontend. All interaction is via REST API consumed by other platform modules and operators.

**Capacity and Cost Budgets (ARCH-DESIGN-010)**: Not applicable at the module level. OAGW runs as part of the CyberFabric platform; capacity planning and cost estimation are handled at the platform deployment level. Rate limiting and circuit breakers provide per-upstream cost controls.

**Infrastructure as Code (OPS-DESIGN-003)**: Not applicable because OAGW is deployed as part of the CyberFabric single-executable via ModKit. Infrastructure provisioning is handled at the platform level.

## 5. Traceability

- **PRD**: [PRD.md](./PRD.md)
- **ADRs**: [ADR/](./ADR/)
- **Features**: [features/](./features/)

