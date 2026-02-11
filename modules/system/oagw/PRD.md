# PRD: Outbound API Gateway (OAGW)

<!-- TOC START -->
## Table of Contents

- [Overview](#overview)
  - [Key Concepts](#key-concepts)
  - [Target Users](#target-users)
  - [Problems Solved](#problems-solved)
  - [Success Criteria](#success-criteria)
  - [Deployment Flexibility](#deployment-flexibility)
- [Actors](#actors)
  - [Human](#human)
  - [System](#system)
- [Functional Requirements](#functional-requirements)
  - [Upstream Management](#upstream-management)
  - [Route Management](#route-management)
  - [Enable/Disable Semantics](#enabledisable-semantics)
  - [Request Proxying](#request-proxying)
  - [Authentication Injection](#authentication-injection)
  - [Rate Limiting](#rate-limiting)
  - [Header Transformation](#header-transformation)
  - [Plugin System](#plugin-system)
  - [Streaming Support](#streaming-support)
  - [Configuration Layering](#configuration-layering)
  - [Hierarchical Configuration Override](#hierarchical-configuration-override)
  - [Alias Resolution and Shadowing](#alias-resolution-and-shadowing)
- [Use Cases](#use-cases)
  - [Proxy HTTP Request](#proxy-http-request)
  - [Configure Upstream](#configure-upstream)
  - [Configure Route](#configure-route)
  - [Rate Limit Exceeded](#rate-limit-exceeded)
  - [SSE Streaming](#sse-streaming)
- [Non-Functional Requirements](#non-functional-requirements)
- [Built-in Plugins](#built-in-plugins)
  - [Auth Plugins (`gts.x.core.oagw.plugin.auth.v1~*`)](#auth-plugins-gtsxcoreoagwpluginauthv1)
  - [Guard Plugins (`gts.x.core.oagw.plugin.guard.v1~*`)](#guard-plugins-gtsxcoreoagwpluginguardv1)
  - [Transform Plugins (`gts.x.core.oagw.plugin.transform.v1~*`)](#transform-plugins-gtsxcoreoagwplugintransformv1)
- [Error Codes](#error-codes)
- [API Endpoints](#api-endpoints)
- [Dependencies](#dependencies)

<!-- TOC END -->

## Overview

OAGW manages all outbound API requests from CyberFabric to external services.

**Component Architecture**: OAGW is composed of three distinct components that work together:
- **API Handler**: Entry point for all requests, handles incoming auth/rate limiting, routes to CP or DP
- **Data Plane (CP)**: Orchestrates proxy requests, executes calls to external services
- **Control Plane (DP)**: Manages configuration data (upstreams/routes/plugins) with multi-layer caching

**Deployment Flexibility**: Modkit framework supports both single-executable (all components in-process) and microservice (components deployed separately) deployment modes.

See [DESIGN.md](./DESIGN.md#component-architecture) for detailed architecture.

**Terminology Note**: Due to historical naming, crate names don't match component terminology:
- `oagw-cp` crate implements the **Data Plane** (proxy/request processing)
- `oagw-dp` crate implements the **Control Plane** (config management)

Throughout this document we use industry-standard terminology. See DESIGN.md for full mapping details.

### Key Concepts

| Concept  | Definition                                                                                |
|----------|-------------------------------------------------------------------------------------------|
| Upstream | External service target (scheme/host/port, protocol, auth, headers, rate limits)          |
| Route    | API path on an upstream. Matches by method/path/query (HTTP), service/method (gRPC), etc. |
| Plugin   | Modular processor: Auth (credential injection), Guard (validation), Transform (mutation)  |
| API Handler | Entry point component that routes requests based on path                               |
| Data Plane | Orchestrates proxy requests and executes calls to external services                  |
| Control Plane | Manages configuration data with database access and multi-layer caching                 |

### Target Users

- **Platform Operators** - Configure upstreams, routes, global policies
- **Tenant Administrators** - Manage tenant-specific configs, credentials, rate limits
- **Application Developers** - Consume external APIs via proxy endpoint

### Problems Solved

- Centralizes credential management (apps don't handle API keys/tokens)
- Unified interface for external services with consistent error handling
- Rate limiting to prevent abuse and cost overruns
- SSRF protection, header validation, security policies

### Success Criteria

- <10ms added latency (p95)
- Zero credential exposure in logs/errors
- 99.9% availability
- Complete audit trail

### Deployment Flexibility

OAGW supports two deployment modes via the modkit framework:

**Single-Executable Mode**:
- All three components (API Handler, CP, DP) run in a single process
- Communication via direct in-process function calls (zero serialization overhead)
- Simpler deployment, ideal for development and small-scale deployments
- Shared in-memory resources (L1 cache, connection pools)

**Microservice Mode**:
- Components deployed as separate services (each horizontally scalable)
- Communication via RPC (modkit provides transparent transport)
- Independent scaling: scale CP for proxy load, scale DP for config operations
- Example topology: 1 API Handler, 10 CP instances, 2 DP instances
- Shared L2 cache (Redis) for config across DP instances

**Deployment Abstraction**: OAGW code is deployment-agnostic. Modkit handles service discovery, load balancing, and transparent RPC based on deployment configuration.

For module structure and deployment architecture, see [DESIGN.md](./DESIGN.md#module-structure).

## Actors

### Human

| Actor                 | Role                                                                    |
|-----------------------|-------------------------------------------------------------------------|
| Platform Operator     | Manages global config: upstreams, routes, system-wide plugins, security |
| Tenant Administrator  | Tenant-specific settings: credentials, rate limits, custom plugins      |
| Application Developer | Consumes APIs via proxy endpoint, no credential management              |

### System

| Actor            | Role                                                  |
|------------------|-------------------------------------------------------|
| Credential Store | Secure storage/retrieval of secrets by UUID reference |
| Types Registry   | GTS schema/instance registration, validation          |
| Upstream Service | External third-party service (OpenAI, Stripe, etc.)   |

## Functional Requirements

### Upstream Management

CRUD for upstream configurations. Each upstream defines: server endpoints, protocol, auth, headers, rate limits.

**Component**: Control Plane handles all upstream CRUD operations via `/api/oagw/v1/upstreams/*` endpoints.

### Route Management

CRUD for routes. Routes define matching rules (method, path, query allowlist) mapping requests to upstreams.

**Component**: Control Plane handles all route CRUD operations via `/api/oagw/v1/routes/*` endpoints.

### Enable/Disable Semantics

Upstreams and routes support an `enabled` boolean field (default: `true`).

**Behavior**:
- **Disabled upstream**: All proxy requests rejected with `503 Service Unavailable` (gateway error). Upstream remains visible in list/get operations.
- **Disabled route**: Route excluded from matching; request falls through to next match or returns 404

**Hierarchical Inheritance**:
- If ancestor tenant disables an upstream, it is disabled for all descendants
- Descendant cannot re-enable an ancestor-disabled resource

**Use Cases**:
- Temporary maintenance without deleting configuration
- Emergency circuit break at management layer
- Gradual rollout (enable route for subset of tenants)

### Request Proxying

Proxy requests via `{METHOD} /api/oagw/v1/proxy/{alias}[/{path}][?{query}]`. Resolves upstream by alias, matches route, transforms and forwards.

**Component Flow**:
1. API Handler receives request at `/api/oagw/v1/proxy/*`
2. Routes to Data Plane
3. Data Plane calls Control Plane for config resolution (upstream, route)
4. Data Plane executes plugins and proxies to external service
5. Returns response to client

No automatic retries are performed by OAGW. Each inbound request results in at most one upstream attempt; retry behavior is client-managed.

### Authentication Injection

Inject credentials into outbound requests. Supported: API Key, Basic Auth, OAuth2 Client Credentials, Bearer Token. Credentials retrieved from credential store at request time.

**Component**: Data Plane executes auth plugins during proxy request processing.

### Rate Limiting

Enforce rate limits at upstream/route levels. Configurable: rate, window, capacity, cost, scope (global/tenant/user/IP), strategy (reject/queue/degrade).

### Header Transformation

Transform request/response headers: set/add/remove, passthrough control, automatic hop-by-hop stripping.

### Plugin System

OAGW provides extensibility through a plugin system with three plugin types:

- **Auth** (`gts.x.core.oagw.plugin.auth.v1~*`): Credential injection (API key, OAuth2, Bearer token, Basic auth)
- **Guard** (`gts.x.core.oagw.plugin.guard.v1~*`): Validation/policy enforcement, can reject requests (timeout, CORS, rate limiting)
- **Transform** (`gts.x.core.oagw.plugin.transform.v1~*`): Request/response mutation (logging, metrics, request ID)

**Component**: Data Plane executes all plugins during proxy request processing. Plugins are defined in Control Plane (CRUD via `/api/oagw/v1/plugins/*`) and loaded by Data Plane.

**Plugin Types**:
- **Built-in plugins**: Included in Data Plane crate (`oagw-dp`), implemented in Rust
- **External plugins**: Separate modkit modules implementing plugin traits from `oagw-sdk`

**Execution order**: Auth → Guards → Transform(request) → Upstream → Transform(response/error)

Plugin chain composition: upstream plugins execute before route plugins.

Plugin API contract: plugin definitions are immutable after creation.

Justification: immutability guarantees deterministic behavior for attached routes/upstreams, improves auditability, and avoids in-place source mutation risks. Updates are performed by creating a new plugin version and re-binding references.

Circuit breaker is a core gateway resilience capability (configured as core policy), not a plugin.

For plugin trait definitions and architecture, see [DESIGN.md](./DESIGN.md#plugin-system-overview).

### Streaming Support

Main protocol focus is HTTP family traffic: HTTP request/response, SSE, WebSocket, and WebTransport session flows.
gRPC support is planned for a later phase (p4).

### Configuration Layering

Merge configs: Upstream (base) < Route < Tenant (highest priority).

### Hierarchical Configuration Override

Configurations defined by ancestor tenants can be overridden by descendants based on visibility and permissions.

**Sharing Modes**:

| Mode      | Behavior                                      |
|-----------|-----------------------------------------------|
| `private` | Not visible to descendants (default)          |
| `inherit` | Visible; descendant can override if specified |
| `enforce` | Visible; descendant cannot override           |

**Override Rules**:

- **Auth**: With `sharing: inherit`, descendant with permission can use own credentials
- **Rate limits**: Descendant can only be stricter: `effective = min(ancestor.enforced, descendant)`
- **Plugins**: Descendant's plugins append; enforced plugins cannot be removed
- **Tags (discovery metadata)**: Merged top-to-bottom with add-only semantics:
  `effective_tags = union(ancestor_tags..., descendant_tags)`. Descendants can add tags but cannot remove inherited tags.

If upstream creation resolves to an existing upstream definition (binding-style flow), request tags are treated as tenant-local additions for effective discovery; they do not mutate ancestor tags.

**Example**:

```
Partner Tenant:
  upstream: api.openai.com
  auth: { secret_ref: "cred://partner-openai-key", sharing: "inherit" }
  rate_limit: { rate: 10000/min, sharing: "enforce" }

Leaf Tenant (with permission):
  auth: { secret_ref: "cred://my-own-openai-key" }  ← overrides partner's key
  rate_limit: { rate: 100/min }  ← effective: min(10000, 100) = 100

Leaf Tenant (without permission):
  auth: inherited from partner  ← uses partner's key
```

### Alias Resolution and Shadowing

Upstreams are identified by alias in proxy URLs: `{METHOD} /api/oagw/v1/proxy/{alias}/{path}`.

**Alias Resolution Rules**:

| Scenario                          | Enforced Alias            | Example                                                |
|-----------------------------------|---------------------------|--------------------------------------------------------|
| Single host                       | `hostname` (without port) | `api.openai.com:443` → alias: `api.openai.com`         |
| Multiple hosts with common suffix | Common domain suffix      | `us.vendor.com`, `eu.vendor.com` → alias: `vendor.com` |
| No common suffix or IP addresses  | Explicit alias required   | `10.0.1.1`, `10.0.1.2` → alias: `my-service`           |

**Alias Defaults**:

- Single endpoint: alias defaults to `server.endpoints[0].host` (without port)
- Multiple endpoints: system extracts common domain suffix
- IP-based or heterogeneous hosts: explicit alias is mandatory

**Shadowing Behavior**:

When resolving an alias, OAGW searches the tenant hierarchy from descendant to root. The closest match wins (descendant shadows ancestor).

```
Request from: subsub-tenant
Alias: "api.openai.com"

Resolution order:
1. subsub-tenant's upstreams  ← wins if found
2. sub-tenant's upstreams
3. root-tenant's upstreams
```

**Example - Port Differentiation**:

```json
// Production upstream (same host, different port)
{
  "server": { "endpoints": [ { "host": "api.openai.com", "port": 443 } ] },
  "alias": "openai-prod"  // explicit alias needed to differentiate
}

// Staging upstream
{
  "server": { "endpoints": [ { "host": "api.openai.com", "port": 8443 } ] },
  "alias": "openai-staging"
}
```

**Example - Multi-Region with Common Suffix**:

```json
// Multi-region upstream with auto-generated alias
{
  "server": {
    "endpoints": [
      { "host": "us.vendor.com", "port": 443 },
      { "host": "eu.vendor.com", "port": 443 }
    ]
  }
  // alias automatically set to "vendor.com" (common suffix)
}
```

**Example - IP-Based Endpoints**:

```json
// IP-based upstream requires explicit alias
{
  "server": {
    "endpoints": [
      { "host": "10.0.1.1", "port": 443 },
      { "host": "10.0.1.2", "port": 443 }
    ]
  },
  "alias": "my-internal-service"  // mandatory for IP addresses
}
```

**Multi-Endpoint Pooling**:

Multiple endpoints within same upstream form a load-balance pool. Requests are distributed across endpoints.

**Compatibility Requirements**:

Endpoints in a pool must have identical:

- `protocol` (can't mix HTTP and gRPC)
- `scheme` (can't mix https and wss)
- `port` (all endpoints must use same port)

**Enforced Limits Across Shadowing**:

When descendant shadows ancestor's alias, enforced limits from ancestor still apply:

```
Root: alias "api.openai.com", rate_limit: { sharing: "enforce", rate: 10000 }
Sub:  alias "api.openai.com" (shadows root)

Effective for sub: min(root.enforced:10000, sub:500) = 500
```

For detailed alias resolution implementation, see [ADR: Resource Identification and Discovery](./docs/adr-resource-identification.md).

## Use Cases

### Proxy HTTP Request

1. App sends request to `/api/oagw/v1/proxy/{alias}/{path}`
2. Resolve upstream by alias
3. Match route by method/path
4. Merge configs (upstream < route < tenant)
5. Retrieve credentials, transform request
6. Execute plugin chain
7. Forward to upstream, return response

### Configure Upstream

POST to `/api/oagw/v1/upstreams` with server endpoints, protocol, auth config. System validates and persists.

### Configure Route

POST to `/api/oagw/v1/routes` with upstream_id and match rules. System validates upstream reference and persists.

### Rate Limit Exceeded

When limit hit: reject (429 with Retry-After), queue, or degrade based on strategy.

### SSE Streaming

Forward events as received, handle connection lifecycle (open/close/error).

## Non-Functional Requirements

| Requirement          | Description                                                |
|----------------------|------------------------------------------------------------|
| Low Latency          | <10ms overhead (p95), plugin timeout enforced              |
| High Availability    | 99.9%, circuit breakers prevent cascade failures           |
| SSRF Protection      | DNS validation, IP pinning, header stripping               |
| Credential Isolation | Never in logs/errors, UUID reference only, tenant-isolated |
| Input Validation     | Path, query, headers, body size validated; reject with 400 |
| Observability        | Request logs with correlation ID, Prometheus metrics       |
| Starlark Sandbox     | No network/file I/O, no imports, timeout/memory limits     |
| Multi-tenancy        | All resources tenant-scoped, isolation at data layer       |

## Built-in Plugins

### Auth Plugins (`gts.x.core.oagw.plugin.auth.v1~*`)

- `gts.x.core.oagw.plugin.auth.v1~x.core.oagw.noop.v1` - No authentication
- `gts.x.core.oagw.plugin.auth.v1~x.core.oagw.apikey.v1` - API key injection (header/query)
- `gts.x.core.oagw.plugin.auth.v1~x.core.oagw.basic.v1` - HTTP Basic authentication
- `gts.x.core.oagw.plugin.auth.v1~x.core.oagw.oauth2.client_cred.v1` - OAuth2 client credentials flow
- `gts.x.core.oagw.plugin.auth.v1~x.core.oagw.oauth2.client_cred_basic.v1` - OAuth2 with Basic auth
- `gts.x.core.oagw.plugin.auth.v1~x.core.oagw.bearer.v1` - Bearer token injection

### Guard Plugins (`gts.x.core.oagw.plugin.guard.v1~*`)

- `gts.x.core.oagw.plugin.guard.v1~x.core.oagw.timeout.v1` - Request timeout enforcement
- `gts.x.core.oagw.plugin.guard.v1~x.core.oagw.cors.v1` - CORS preflight validation

### Transform Plugins (`gts.x.core.oagw.plugin.transform.v1~*`)

- `gts.x.core.oagw.plugin.transform.v1~x.core.oagw.logging.v1` - Request/response logging
- `gts.x.core.oagw.plugin.transform.v1~x.core.oagw.metrics.v1` - Prometheus metrics collection
- `gts.x.core.oagw.plugin.transform.v1~x.core.oagw.request_id.v1` - X-Request-ID propagation

## Error Codes

| HTTP | Error                | Retriable |
|------|----------------------|-----------|
| 400  | ValidationError      | No        |
| 401  | AuthenticationFailed | No        |
| 404  | RouteNotFound        | No        |
| 413  | PayloadTooLarge      | No        |
| 429  | RateLimitExceeded    | Yes       |
| 500  | SecretNotFound       | No        |
| 502  | DownstreamError      | Depends   |
| 503  | CircuitBreakerOpen   | Yes       |
| 504  | Timeout              | Yes       |

## API Endpoints

All requests enter through API Handler, which routes based on path:

**Management API** (routed to Control Plane):
```
POST/GET/PUT/DELETE /api/oagw/v1/upstreams[/{id}]
POST/GET/PUT/DELETE /api/oagw/v1/routes[/{id}]
POST/GET/DELETE /api/oagw/v1/plugins[/{id}]
GET /api/oagw/v1/plugins/{id}/source
```

**Proxy API** (routed to Data Plane):
```
{METHOD} /api/oagw/v1/proxy/{alias}[/{path}][?{query}]
```

**Request Flow**:
- Management operations: `API Handler → Control Plane`
- Proxy operations: `API Handler → Data Plane → Control Plane (config) → Data Plane (execute)`

See [DESIGN.md](./DESIGN.md#request-flow) for detailed flow diagrams.

## Dependencies

- `types_registry` - GTS schema/instance registration
- `cred_store` - Secret retrieval
- `api_ingress` - REST API hosting
- `modkit-db` - Database persistence
- `modkit-auth` - Authorization
