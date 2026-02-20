# Feature: Core Configuration Management

- [ ] `p1` - **ID**: `cpt-cf-oagw-featstatus-core-config-mgmt`

- [ ] `p1` - `cpt-cf-oagw-feature-core-config-mgmt`

## 1. Feature Context

### 1.1 Overview

CRUD operations for upstream and route configurations, including enable/disable semantics. This feature covers the Control Plane management surface that all other OAGW capabilities depend on.

### 1.2 Purpose

Provides the foundational configuration management layer for OAGW. Without upstreams and routes, no proxy requests can be resolved or forwarded. Enable/disable semantics allow operators to temporarily suspend traffic without deleting configuration.

Addresses PRD requirements: `cpt-cf-oagw-fr-upstream-mgmt`, `cpt-cf-oagw-fr-route-mgmt`, `cpt-cf-oagw-fr-enable-disable`.

### 1.3 Actors

| Actor | Role in Feature |
|-------|-----------------|
| `cpt-cf-oagw-actor-platform-operator` | Creates, updates, deletes, enables/disables upstreams and routes globally |
| `cpt-cf-oagw-actor-tenant-admin` | Manages tenant-scoped upstreams and routes within their hierarchy |

### 1.4 References

- **PRD**: [PRD.md](../PRD.md)
- **Design**: [DESIGN.md](../DESIGN.md)
- **Requirements**: `cpt-cf-oagw-fr-upstream-mgmt`, `cpt-cf-oagw-fr-route-mgmt`, `cpt-cf-oagw-fr-enable-disable`
- **Design elements**: `cpt-cf-oagw-design-oagw`, `cpt-cf-oagw-adr-component-architecture`, `cpt-cf-oagw-adr-resource-identification`
- **Dependencies**: None (foundational feature)

## 2. Actor Flows (CDSL)

### Create Upstream Flow

- [ ] `p1` - **ID**: `cpt-cf-oagw-flow-create-upstream`

**Actor**: `cpt-cf-oagw-actor-platform-operator`

**Success Scenarios**:
- Upstream is created with server endpoints, protocol, auth config, headers, and rate limits
- Generated alias is derived from endpoint hostname (or explicitly provided)
- Upstream is immediately available for route binding and proxy resolution

**Error Scenarios**:
- Validation fails (missing required fields, invalid endpoint format, invalid protocol)
- Alias already exists within the same tenant (409 Conflict)
- Authentication/authorization failure (401/403)

**Steps**:
1. [ ] - `p1` - Operator sends POST /api/oagw/v1/upstreams with upstream configuration - `inst-cu-1`
2. [ ] - `p1` - Extract SecurityContext (tenant_id, principal_id) from bearer token - `inst-cu-2`
3. [ ] - `p1` - Parse and validate request DTO against upstream schema - `inst-cu-3`
4. [ ] - `p1` - **IF** alias not provided - `inst-cu-4`
   1. [ ] - `p1` - Derive alias from endpoint hostname using alias generation rules - `inst-cu-4a`
5. [ ] - `p1` - **IF** alias is IP address or heterogeneous hosts and no explicit alias - `inst-cu-5`
   1. [ ] - `p1` - **RETURN** 400 ValidationError: explicit alias required - `inst-cu-5a`
6. [ ] - `p1` - Validate endpoint compatibility (same scheme, port, protocol for multi-endpoint) - `inst-cu-6`
7. [ ] - `p1` - DB: INSERT oagw_upstream (tenant_id, alias, server, protocol, auth, headers, plugins, rate_limit, enabled) - `inst-cu-7`
8. [ ] - `p1` - **IF** alias uniqueness constraint violated - `inst-cu-8`
   1. [ ] - `p1` - **RETURN** 409 Conflict: alias already exists for this tenant - `inst-cu-8a`
9. [ ] - `p1` - **RETURN** 201 Created with upstream resource including generated ID (anonymous GTS identifier) - `inst-cu-9`

### Update Upstream Flow

- [ ] `p1` - **ID**: `cpt-cf-oagw-flow-update-upstream`

**Actor**: `cpt-cf-oagw-actor-platform-operator`

**Success Scenarios**:
- Upstream configuration is updated with new values
- Existing routes remain bound to the updated upstream

**Error Scenarios**:
- Upstream not found (404)
- Alias change conflicts with existing alias in same tenant (409)
- Validation failure (400)

**Steps**:
1. [ ] - `p1` - Operator sends PUT /api/oagw/v1/upstreams/{id} with updated configuration - `inst-uu-1`
2. [ ] - `p1` - Extract SecurityContext and parse anonymous GTS identifier to extract UUID - `inst-uu-2`
3. [ ] - `p1` - DB: SELECT oagw_upstream WHERE id = :uuid AND tenant_id = :tenant_id - `inst-uu-3`
4. [ ] - `p1` - **IF** upstream not found - `inst-uu-4`
   1. [ ] - `p1` - **RETURN** 404 Not Found - `inst-uu-4a`
5. [ ] - `p1` - Validate updated fields against upstream schema - `inst-uu-5`
6. [ ] - `p1` - DB: UPDATE oagw_upstream SET (server, protocol, auth, headers, plugins, rate_limit, alias, updated_at, updated_by) WHERE id = :uuid - `inst-uu-6`
7. [ ] - `p1` - **IF** alias uniqueness constraint violated - `inst-uu-7`
   1. [ ] - `p1` - **RETURN** 409 Conflict - `inst-uu-7a`
8. [ ] - `p1` - **RETURN** 200 OK with updated upstream resource - `inst-uu-8`

### Delete Upstream Flow

- [ ] `p1` - **ID**: `cpt-cf-oagw-flow-delete-upstream`

**Actor**: `cpt-cf-oagw-actor-platform-operator`

**Success Scenarios**:
- Upstream and all associated routes are deleted (CASCADE)

**Error Scenarios**:
- Upstream not found (404)

**Steps**:
1. [ ] - `p1` - Operator sends DELETE /api/oagw/v1/upstreams/{id} - `inst-du-1`
2. [ ] - `p1` - Extract SecurityContext and parse GTS identifier to UUID - `inst-du-2`
3. [ ] - `p1` - DB: DELETE oagw_upstream WHERE id = :uuid AND tenant_id = :tenant_id (CASCADE deletes routes) - `inst-du-3`
4. [ ] - `p1` - **IF** no rows affected - `inst-du-4`
   1. [ ] - `p1` - **RETURN** 404 Not Found - `inst-du-4a`
5. [ ] - `p1` - **RETURN** 204 No Content - `inst-du-5`

### List Upstreams Flow

- [ ] `p1` - **ID**: `cpt-cf-oagw-flow-list-upstreams`

**Actor**: `cpt-cf-oagw-actor-platform-operator`

**Success Scenarios**:
- Returns paginated list of upstreams visible to the tenant

**Error Scenarios**:
- Invalid pagination parameters (400)

**Steps**:
1. [ ] - `p1` - Operator sends GET /api/oagw/v1/upstreams with optional $top/$skip parameters - `inst-lu-1`
2. [ ] - `p1` - Extract SecurityContext (tenant_id) - `inst-lu-2`
3. [ ] - `p1` - DB: SELECT oagw_upstream WHERE tenant_id = :tenant_id ORDER BY created_at LIMIT $top OFFSET $skip - `inst-lu-3`
4. [ ] - `p1` - **RETURN** 200 OK with upstream array - `inst-lu-4`

### Create Route Flow

- [ ] `p1` - **ID**: `cpt-cf-oagw-flow-create-route`

**Actor**: `cpt-cf-oagw-actor-platform-operator`

**Success Scenarios**:
- Route is created with match rules linked to an existing upstream
- Route is immediately active for request matching

**Error Scenarios**:
- Referenced upstream not found or not owned by tenant (400)
- Validation failure: empty methods, path not starting with `/` (400)

**Steps**:
1. [ ] - `p1` - Operator sends POST /api/oagw/v1/routes with route configuration - `inst-cr-1`
2. [ ] - `p1` - Extract SecurityContext - `inst-cr-2`
3. [ ] - `p1` - Parse and validate request DTO against route schema - `inst-cr-3`
4. [ ] - `p1` - DB: SELECT oagw_upstream WHERE id = :upstream_id AND tenant_id = :tenant_id - `inst-cr-4`
5. [ ] - `p1` - **IF** upstream not found - `inst-cr-5`
   1. [ ] - `p1` - **RETURN** 400 ValidationError: upstream not found - `inst-cr-5a`
6. [ ] - `p1` - Validate route invariants (non-empty methods, path starts with `/`, valid path_suffix_mode, integer priority) - `inst-cr-6`
7. [ ] - `p1` - DB: INSERT oagw_route (tenant_id, upstream_id, match, plugins, rate_limit, enabled, priority, tags) - `inst-cr-7`
8. [ ] - `p1` - **RETURN** 201 Created with route resource including generated ID - `inst-cr-8`

### Update Route Flow

- [ ] `p1` - **ID**: `cpt-cf-oagw-flow-update-route`

**Actor**: `cpt-cf-oagw-actor-platform-operator`

**Success Scenarios**:
- Route match rules, plugins, rate limits, or priority are updated

**Error Scenarios**:
- Route not found (404)
- Upstream reference change to non-existent upstream (400)

**Steps**:
1. [ ] - `p1` - Operator sends PUT /api/oagw/v1/routes/{id} with updated configuration - `inst-ur-1`
2. [ ] - `p1` - Extract SecurityContext and parse GTS identifier to UUID - `inst-ur-2`
3. [ ] - `p1` - DB: SELECT oagw_route WHERE id = :uuid AND tenant_id = :tenant_id - `inst-ur-3`
4. [ ] - `p1` - **IF** route not found - `inst-ur-4`
   1. [ ] - `p1` - **RETURN** 404 Not Found - `inst-ur-4a`
5. [ ] - `p1` - **IF** upstream_id changed, validate new upstream exists and is owned by tenant - `inst-ur-5`
6. [ ] - `p1` - Validate updated route invariants - `inst-ur-6`
7. [ ] - `p1` - DB: UPDATE oagw_route SET (match, plugins, rate_limit, priority, tags, updated_at, updated_by) WHERE id = :uuid - `inst-ur-7`
8. [ ] - `p1` - **RETURN** 200 OK with updated route resource - `inst-ur-8`

### Delete Route Flow

- [ ] `p1` - **ID**: `cpt-cf-oagw-flow-delete-route`

**Actor**: `cpt-cf-oagw-actor-platform-operator`

**Success Scenarios**:
- Route is permanently removed

**Error Scenarios**:
- Route not found (404)

**Steps**:
1. [ ] - `p1` - Operator sends DELETE /api/oagw/v1/routes/{id} - `inst-dr-1`
2. [ ] - `p1` - Extract SecurityContext and parse GTS identifier to UUID - `inst-dr-2`
3. [ ] - `p1` - DB: DELETE oagw_route WHERE id = :uuid AND tenant_id = :tenant_id - `inst-dr-3`
4. [ ] - `p1` - **IF** no rows affected - `inst-dr-4`
   1. [ ] - `p1` - **RETURN** 404 Not Found - `inst-dr-4a`
5. [ ] - `p1` - **RETURN** 204 No Content - `inst-dr-5`

### Toggle Enable/Disable Flow

- [ ] `p1` - **ID**: `cpt-cf-oagw-flow-toggle-enabled`

**Actor**: `cpt-cf-oagw-actor-platform-operator`

**Success Scenarios**:
- Upstream or route enabled field is toggled
- Disabled upstream causes all proxy requests to return 503
- Disabled route is excluded from route matching

**Error Scenarios**:
- Resource not found (404)

**Steps**:
1. [ ] - `p1` - Operator sends PUT /api/oagw/v1/upstreams/{id} or /routes/{id} with updated `enabled` field - `inst-te-1`
2. [ ] - `p1` - Extract SecurityContext and parse GTS identifier - `inst-te-2`
3. [ ] - `p1` - DB: UPDATE oagw_upstream/oagw_route SET enabled = :value, updated_at = NOW(), updated_by = :principal WHERE id = :uuid AND tenant_id = :tenant_id - `inst-te-3`
4. [ ] - `p1` - **IF** no rows affected - `inst-te-4`
   1. [ ] - `p1` - **RETURN** 404 Not Found - `inst-te-4a`
5. [ ] - `p1` - **RETURN** 200 OK with updated resource - `inst-te-5`

## 3. Processes / Business Logic (CDSL)

### Alias Generation Algorithm

- [ ] `p1` - **ID**: `cpt-cf-oagw-algo-alias-generation`

**Input**: Upstream server endpoints configuration, optional explicit alias

**Output**: Resolved alias string or validation error

**Steps**:
1. [ ] - `p1` - **IF** explicit alias provided, use it directly - `inst-ag-1`
2. [ ] - `p1` - **IF** single endpoint with standard port (80/443/ws:80/wss:443/wt:443/grpc:443) - `inst-ag-2`
   1. [ ] - `p1` - Alias = hostname (port omitted) - `inst-ag-2a`
3. [ ] - `p1` - **IF** single endpoint with non-standard port - `inst-ag-3`
   1. [ ] - `p1` - Alias = hostname:port - `inst-ag-3a`
4. [ ] - `p1` - **IF** multiple endpoints with common domain suffix - `inst-ag-4`
   1. [ ] - `p1` - Alias = common domain suffix - `inst-ag-4a`
5. [ ] - `p1` - **IF** IP addresses or heterogeneous hosts without explicit alias - `inst-ag-5`
   1. [ ] - `p1` - **RETURN** validation error: explicit alias required - `inst-ag-5a`
6. [ ] - `p1` - **RETURN** resolved alias - `inst-ag-6`

### Upstream Validation Algorithm

- [ ] `p1` - **ID**: `cpt-cf-oagw-algo-validate-upstream`

**Input**: Upstream creation/update payload

**Output**: Validation result with errors

**Steps**:
1. [ ] - `p1` - Validate server.endpoints is non-empty - `inst-vu-1`
2. [ ] - `p1` - **FOR EACH** endpoint in server.endpoints - `inst-vu-2`
   1. [ ] - `p1` - Validate scheme is allowed (https for MVP) - `inst-vu-2a`
   2. [ ] - `p1` - Validate host is valid hostname or IP - `inst-vu-2b`
   3. [ ] - `p1` - Validate port is in valid range (1-65535) - `inst-vu-2c`
3. [ ] - `p1` - **IF** multiple endpoints, validate all have same scheme, port, and protocol - `inst-vu-3`
4. [ ] - `p1` - Validate protocol is a recognized GTS identifier - `inst-vu-4`
5. [ ] - `p1` - **IF** auth provided, validate auth type is a recognized GTS plugin identifier - `inst-vu-5`
6. [ ] - `p1` - **IF** rate_limit provided, validate rate > 0, window is valid, scope is recognized - `inst-vu-6`
7. [ ] - `p1` - **RETURN** validation result - `inst-vu-7`

### Route Validation Algorithm

- [ ] `p1` - **ID**: `cpt-cf-oagw-algo-validate-route`

**Input**: Route creation/update payload

**Output**: Validation result with errors

**Steps**:
1. [ ] - `p1` - Validate upstream_id references an existing upstream owned by the tenant - `inst-vr-1`
2. [ ] - `p1` - Validate match.http.methods is non-empty and contains valid HTTP methods - `inst-vr-2`
3. [ ] - `p1` - Validate match.http.path starts with `/` - `inst-vr-3`
4. [ ] - `p1` - Validate match.http.path_suffix_mode is one of: append, disabled - `inst-vr-4`
5. [ ] - `p1` - **IF** match.http.query_allowlist provided, validate entries are valid query parameter names - `inst-vr-5`
6. [ ] - `p1` - Validate priority is a non-negative integer - `inst-vr-6`
7. [ ] - `p1` - **RETURN** validation result - `inst-vr-7`

### GTS Identifier Parsing Algorithm

- [ ] `p1` - **ID**: `cpt-cf-oagw-algo-parse-gts-id`

**Input**: Anonymous GTS identifier string (e.g., `gts.x.core.oagw.upstream.v1~{uuid}`)

**Output**: Extracted UUID or validation error

**Steps**:
1. [ ] - `p1` - Split identifier on `~` separator - `inst-pg-1`
2. [ ] - `p1` - **IF** no `~` found or parts count != 2 - `inst-pg-2`
   1. [ ] - `p1` - **RETURN** validation error: invalid GTS identifier format - `inst-pg-2a`
3. [ ] - `p1` - Validate schema prefix matches expected resource type - `inst-pg-3`
4. [ ] - `p1` - Parse instance part as UUID - `inst-pg-4`
5. [ ] - `p1` - **IF** UUID parse fails - `inst-pg-5`
   1. [ ] - `p1` - **RETURN** validation error: invalid UUID in GTS identifier - `inst-pg-5a`
6. [ ] - `p1` - **RETURN** extracted UUID - `inst-pg-6`

## 4. States (CDSL)

### Upstream Enabled State Machine

- [ ] `p1` - **ID**: `cpt-cf-oagw-state-upstream-enabled`

**States**: enabled, disabled

**Initial State**: enabled

**Transitions**:
1. [ ] - `p1` - **FROM** enabled **TO** disabled **WHEN** operator sets enabled=false via PUT - `inst-ue-1`
2. [ ] - `p1` - **FROM** disabled **TO** enabled **WHEN** operator sets enabled=true via PUT - `inst-ue-2`

**Effects**:
- Disabled upstream: proxy requests return 503 Service Unavailable with error type `gts.x.core.errors.err.v1~x.oagw.routing.upstream_disabled.v1`
- Disabled upstream: still visible in list/get operations for management
- Ancestor disable propagates: if ancestor disables an upstream alias, descendants cannot enable it

### Route Enabled State Machine

- [ ] `p1` - **ID**: `cpt-cf-oagw-state-route-enabled`

**States**: enabled, disabled

**Initial State**: enabled

**Transitions**:
1. [ ] - `p1` - **FROM** enabled **TO** disabled **WHEN** operator sets enabled=false via PUT - `inst-re-1`
2. [ ] - `p1` - **FROM** disabled **TO** enabled **WHEN** operator sets enabled=true via PUT - `inst-re-2`

**Effects**:
- Disabled route: excluded from route matching during proxy resolution
- Disabled route: still visible in list/get operations for management

## 5. Definitions of Done

### Implement Upstream CRUD Operations

- [ ] `p1` - **ID**: `cpt-cf-oagw-dod-upstream-crud`

The system **MUST** provide Create, Read (get + list), Update, Delete operations for upstream configurations via `/api/oagw/v1/upstreams`. All operations **MUST** be tenant-scoped via SecureConn. Alias uniqueness **MUST** be enforced per tenant. Anonymous GTS identifiers **MUST** be used in the API layer with UUID extraction for DB operations.

**Implements**:
- `cpt-cf-oagw-flow-create-upstream`
- `cpt-cf-oagw-flow-update-upstream`
- `cpt-cf-oagw-flow-delete-upstream`
- `cpt-cf-oagw-flow-list-upstreams`
- `cpt-cf-oagw-algo-validate-upstream`
- `cpt-cf-oagw-algo-alias-generation`
- `cpt-cf-oagw-algo-parse-gts-id`

**Touches**:
- API: `POST/GET/PUT/DELETE /api/oagw/v1/upstreams[/{id}]`
- DB: `oagw_upstream`
- Entities: `Upstream`

### Implement Route CRUD Operations

- [ ] `p1` - **ID**: `cpt-cf-oagw-dod-route-crud`

The system **MUST** provide Create, Read (get + list), Update, Delete operations for route configurations via `/api/oagw/v1/routes`. Routes **MUST** reference an existing upstream owned by the same tenant. Match rules **MUST** support HTTP methods, path prefix, query allowlist, and path_suffix_mode.

**Implements**:
- `cpt-cf-oagw-flow-create-route`
- `cpt-cf-oagw-flow-update-route`
- `cpt-cf-oagw-flow-delete-route`
- `cpt-cf-oagw-algo-validate-route`

**Touches**:
- API: `POST/GET/PUT/DELETE /api/oagw/v1/routes[/{id}]`
- DB: `oagw_route`
- Entities: `Route`

### Implement Enable/Disable Semantics

- [ ] `p1` - **ID**: `cpt-cf-oagw-dod-enable-disable`

The system **MUST** support boolean `enabled` field on upstreams and routes. Disabled upstreams **MUST** return 503 on proxy requests. Disabled routes **MUST** be excluded from route matching. Ancestor disable **MUST** propagate to descendants.

**Implements**:
- `cpt-cf-oagw-flow-toggle-enabled`
- `cpt-cf-oagw-state-upstream-enabled`
- `cpt-cf-oagw-state-route-enabled`

**Touches**:
- API: `PUT /api/oagw/v1/upstreams/{id}`, `PUT /api/oagw/v1/routes/{id}`
- DB: `oagw_upstream.enabled`, `oagw_route.enabled`

### Implement DB Schema and Repositories

- [ ] `p1` - **ID**: `cpt-cf-oagw-dod-db-schema`

The system **MUST** provide migrations for `oagw_upstream` and `oagw_route` tables with tenant-scoped UUID PKs, JSONB config columns, `enabled` field, and audit timestamps. Repositories **MUST** use SeaORM with `SecureConn` scoping. Indexes **MUST** cover alias lookup, tenant filtering, route matching by upstream+priority, and enabled filtering.

**Implements**:
- `cpt-cf-oagw-flow-create-upstream`
- `cpt-cf-oagw-flow-create-route`

**Touches**:
- DB: `oagw_upstream`, `oagw_route`
- Entities: `Upstream`, `Route`

## 6. Acceptance Criteria

- [ ] Upstream can be created with server endpoints, protocol, auth, headers, rate limits, and tags
- [ ] Upstream alias is auto-derived from hostname or explicitly provided; IP/heterogeneous hosts require explicit alias
- [ ] Alias uniqueness is enforced per tenant; duplicate alias returns 409
- [ ] Upstream can be read by ID (anonymous GTS identifier), listed with $top/$skip pagination
- [ ] Upstream can be updated; alias change respects uniqueness constraint
- [ ] Upstream deletion cascades to associated routes
- [ ] Route can be created with HTTP match rules (methods, path, query_allowlist, path_suffix_mode, priority) linked to an existing upstream
- [ ] Route creation fails with 400 if referenced upstream does not exist or is not owned by tenant
- [ ] Route can be read, listed, updated, and deleted
- [ ] Disabling an upstream causes proxy requests to return 503 Service Unavailable
- [ ] Disabling a route excludes it from route matching
- [ ] All operations are tenant-scoped via SecureConn; cross-tenant access is impossible
- [ ] All API responses use anonymous GTS identifiers for resource IDs
- [ ] Invalid input returns 400 ValidationError with clear error details

## 7. Not Applicable Sections

**Compliance (COMPL)**: Not applicable because OAGW configuration management does not handle regulated data (PII, financial records). Audit logging provides the compliance trail.

**Privacy (COMPL)**: Not applicable because upstream/route configurations do not contain PII. Credential references use UUID-only `secret_ref`.

**User-Facing Architecture (UX)**: Not applicable because this is a backend REST API with no user-facing frontend.
