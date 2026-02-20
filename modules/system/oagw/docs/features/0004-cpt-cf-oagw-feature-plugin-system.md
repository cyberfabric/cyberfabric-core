# Feature: Plugin System

- [ ] `p1` - **ID**: `cpt-cf-oagw-featstatus-plugin-system`

- [ ] `p1` - `cpt-cf-oagw-feature-plugin-system`

## 1. Feature Context

### 1.1 Overview

Three plugin types (Auth, Guard, Transform) with defined execution order, built-in and external (Starlark) categories, GTS type identification, immutable custom plugin lifecycle, CRUD management API, plugin chain composition, and garbage collection of unlinked plugins.

### 1.2 Purpose

Plugin architecture enables extensibility without modifying core gateway logic. Built-in plugins cover common cross-cutting concerns (API key auth, timeout, logging). External Starlark plugins allow tenant-specific or domain-specific customization. Immutability guarantees deterministic behavior for attached routes and upstreams.

Addresses PRD requirements: `cpt-cf-oagw-fr-plugin-system`, `cpt-cf-oagw-fr-plugin-immutability`, `cpt-cf-oagw-fr-plugin-crud`.

### 1.3 Actors

| Actor | Role in Feature |
|-------|-----------------|
| `cpt-cf-oagw-actor-platform-operator` | Manages system-wide plugin definitions and built-in plugin configuration |
| `cpt-cf-oagw-actor-tenant-admin` | Creates custom Starlark plugins, attaches plugins to upstreams/routes |
| `cpt-cf-oagw-actor-types-registry` | Provides GTS schema and instance registration for plugin type identification |

### 1.4 References

- **PRD**: [PRD.md](../PRD.md)
- **Design**: [DESIGN.md](../DESIGN.md)
- **Requirements**: `cpt-cf-oagw-fr-plugin-system`, `cpt-cf-oagw-fr-plugin-immutability`, `cpt-cf-oagw-fr-plugin-crud`
- **Design elements**: `cpt-cf-oagw-design-oagw`, `cpt-cf-oagw-adr-plugin-system`
- **Dependencies**: `cpt-cf-oagw-feature-core-config-mgmt` (plugins are referenced by upstreams/routes), `cpt-cf-oagw-feature-proxy-auth` (plugin chain executes during proxy flow), `cpt-cf-oagw-feature-multi-tenant-config` (plugin chains merge across hierarchy)

## 2. Actor Flows (CDSL)

### Create Custom Plugin Flow

- [ ] `p2` - **ID**: `cpt-cf-oagw-flow-create-plugin`

**Actor**: `cpt-cf-oagw-actor-tenant-admin`

**Success Scenarios**:
- Custom Starlark plugin is created with source code, config schema, and type classification
- Plugin is available for attachment to upstreams/routes

**Error Scenarios**:
- Validation failure: invalid plugin_type, missing source_code, invalid config_schema (400)
- Plugin name already exists for tenant (409 Conflict)
- Authentication/authorization failure (401/403)

**Steps**:
1. [ ] - `p2` - Tenant admin sends POST /api/oagw/v1/plugins with plugin definition - `inst-cp-1`
2. [ ] - `p2` - Extract SecurityContext (tenant_id, principal_id) - `inst-cp-2`
3. [ ] - `p2` - Validate plugin_type is one of: auth, guard, transform - `inst-cp-3`
4. [ ] - `p2` - Validate source_code is non-empty Starlark - `inst-cp-4`
5. [ ] - `p2` - Validate config_schema is valid JSON Schema - `inst-cp-5`
6. [ ] - `p2` - **IF** plugin_type = transform, validate phases array contains valid values (on_request, on_response, on_error) - `inst-cp-6`
7. [ ] - `p2` - DB: INSERT oagw_plugin (tenant_id, name, description, plugin_type, phases, config_schema, source_code, enabled) - `inst-cp-7`
8. [ ] - `p2` - **IF** name uniqueness constraint violated - `inst-cp-8`
   1. [ ] - `p2` - **RETURN** 409 Conflict: plugin name already exists for this tenant - `inst-cp-8a`
9. [ ] - `p2` - **RETURN** 201 Created with plugin resource including anonymous GTS identifier - `inst-cp-9`

### Delete Plugin Flow

- [ ] `p2` - **ID**: `cpt-cf-oagw-flow-delete-plugin`

**Actor**: `cpt-cf-oagw-actor-tenant-admin`

**Success Scenarios**:
- Unlinked plugin is permanently deleted

**Error Scenarios**:
- Plugin not found (404)
- Plugin is referenced by upstreams/routes (409 PluginInUse with referenced_by list)

**Steps**:
1. [ ] - `p2` - Tenant admin sends DELETE /api/oagw/v1/plugins/{id} - `inst-dp-1`
2. [ ] - `p2` - Extract SecurityContext and parse GTS identifier to UUID - `inst-dp-2`
3. [ ] - `p2` - DB: Check if plugin is referenced by any upstream or route plugins.items - `inst-dp-3`
4. [ ] - `p2` - **IF** plugin is referenced - `inst-dp-4`
   1. [ ] - `p2` - **RETURN** 409 PluginInUse with referenced_by lists (upstream IDs, route IDs) - `inst-dp-4a`
5. [ ] - `p2` - DB: DELETE oagw_plugin WHERE id = :uuid AND tenant_id = :tenant_id - `inst-dp-5`
6. [ ] - `p2` - **IF** no rows affected - `inst-dp-6`
   1. [ ] - `p2` - **RETURN** 404 Not Found - `inst-dp-6a`
7. [ ] - `p2` - **RETURN** 204 No Content - `inst-dp-7`

### Get Plugin Source Flow

- [ ] `p2` - **ID**: `cpt-cf-oagw-flow-get-plugin-source`

**Actor**: `cpt-cf-oagw-actor-tenant-admin`

**Success Scenarios**:
- Starlark source code is returned as text/plain

**Error Scenarios**:
- Plugin not found (404)
- Builtin plugin (no source code available)

**Steps**:
1. [ ] - `p2` - Tenant admin sends GET /api/oagw/v1/plugins/{id}/source - `inst-gs-1`
2. [ ] - `p2` - Extract SecurityContext and parse GTS identifier - `inst-gs-2`
3. [ ] - `p2` - **IF** identifier is a named builtin (not UUID) - `inst-gs-3`
   1. [ ] - `p2` - **RETURN** 404 Not Found (builtin plugins have no retrievable source) - `inst-gs-3a`
4. [ ] - `p2` - DB: SELECT source_code FROM oagw_plugin WHERE id = :uuid AND tenant_id = :tenant_id - `inst-gs-4`
5. [ ] - `p2` - **IF** not found - `inst-gs-5`
   1. [ ] - `p2` - **RETURN** 404 Not Found - `inst-gs-5a`
6. [ ] - `p2` - **RETURN** 200 OK with Content-Type: text/plain and source_code body - `inst-gs-6`

### Plugin Chain Execution Flow

- [ ] `p1` - **ID**: `cpt-cf-oagw-flow-plugin-chain-execution`

**Actor**: `cpt-cf-oagw-actor-app-developer` (indirect — triggered by proxy request)

**Success Scenarios**:
- Plugin chain executes in correct order: Auth → Guards → Transform(request) → Upstream → Transform(response/error)
- All plugins complete within timeout limits

**Error Scenarios**:
- Guard plugin rejects request (returns guard-specific error)
- Plugin execution timeout exceeded
- Plugin not found during resolution (503 PluginNotFound)

**Steps**:
1. [ ] - `p1` - Build plugin chain from merged configuration: upstream plugins + route plugins - `inst-pce-1`
2. [ ] - `p1` - Execute auth plugin (one per upstream, credential injection) - `inst-pce-2`
3. [ ] - `p1` - **IF** auth plugin fails - `inst-pce-3`
   1. [ ] - `p1` - **RETURN** auth error, skip remaining chain - `inst-pce-3a`
4. [ ] - `p1` - **FOR EACH** guard plugin in chain order - `inst-pce-4`
   1. [ ] - `p1` - Execute guard plugin - `inst-pce-4a`
   2. [ ] - `p1` - **IF** guard calls ctx.reject() - `inst-pce-4b`
      1. [ ] - `p1` - **RETURN** rejection response, skip remaining chain - `inst-pce-4b1`
5. [ ] - `p1` - **FOR EACH** transform plugin in chain order - `inst-pce-5`
   1. [ ] - `p1` - Execute on_request phase - `inst-pce-5a`
   2. [ ] - `p1` - **IF** transform calls ctx.reject() or ctx.respond() - `inst-pce-5b`
      1. [ ] - `p1` - **RETURN** custom response, skip upstream call - `inst-pce-5b1`
6. [ ] - `p1` - Forward request to upstream service - `inst-pce-6`
7. [ ] - `p1` - **IF** upstream responds successfully - `inst-pce-7`
   1. [ ] - `p1` - **FOR EACH** transform plugin in chain order, execute on_response phase - `inst-pce-7a`
8. [ ] - `p1` - **IF** upstream error - `inst-pce-8`
   1. [ ] - `p1` - **FOR EACH** transform plugin in chain order, execute on_error phase - `inst-pce-8a`
9. [ ] - `p1` - **RETURN** final response - `inst-pce-9`

## 3. Processes / Business Logic (CDSL)

### Plugin Identifier Resolution Algorithm

- [ ] `p1` - **ID**: `cpt-cf-oagw-algo-plugin-resolution`

**Input**: GTS plugin identifier string from configuration

**Output**: Resolved plugin (builtin implementation or custom Starlark definition)

**Steps**:
1. [ ] - `p1` - Parse GTS identifier: split on `~` to get schema prefix and instance part - `inst-pir-1`
2. [ ] - `p1` - Validate schema prefix matches a known plugin type (auth_plugin, guard_plugin, transform_plugin) - `inst-pir-2`
3. [ ] - `p1` - **IF** instance part is a named identifier (e.g., x.core.oagw.logging.v1) - `inst-pir-3`
   1. [ ] - `p1` - Lookup in builtin plugin registry - `inst-pir-3a`
   2. [ ] - `p1` - **IF** not found in registry - `inst-pir-3b`
      1. [ ] - `p1` - **RETURN** 503 PluginNotFound - `inst-pir-3b1`
   3. [ ] - `p1` - **RETURN** builtin plugin implementation - `inst-pir-3c`
4. [ ] - `p1` - **IF** instance part is a UUID - `inst-pir-4`
   1. [ ] - `p1` - DB: SELECT oagw_plugin WHERE id = :uuid - `inst-pir-4a`
   2. [ ] - `p1` - **IF** not found - `inst-pir-4b`
      1. [ ] - `p1` - **RETURN** 503 PluginNotFound - `inst-pir-4b1`
   3. [ ] - `p1` - Validate plugin_type in DB matches schema prefix type - `inst-pir-4c`
   4. [ ] - `p1` - **RETURN** custom Starlark plugin definition - `inst-pir-4d`
5. [ ] - `p1` - **RETURN** error: invalid plugin identifier format - `inst-pir-5`

### Plugin Chain Composition Algorithm

- [ ] `p1` - **ID**: `cpt-cf-oagw-algo-chain-composition`

**Input**: Upstream plugins configuration, route plugins configuration

**Output**: Ordered plugin chain for execution

**Steps**:
1. [ ] - `p1` - Initialize chain as empty list - `inst-cc-1`
2. [ ] - `p1` - **IF** upstream.plugins.items is not null - `inst-cc-2`
   1. [ ] - `p1` - Resolve each plugin identifier and append to chain - `inst-cc-2a`
3. [ ] - `p1` - **IF** route.plugins.items is not null - `inst-cc-3`
   1. [ ] - `p1` - Resolve each plugin identifier and append to chain - `inst-cc-3a`
4. [ ] - `p1` - Separate chain into: auth plugins, guard plugins, transform plugins - `inst-cc-4`
5. [ ] - `p1` - Validate: at most one auth plugin total - `inst-cc-5`
6. [ ] - `p1` - **RETURN** composed chain: [auth] + [guards...] + [transforms...] - `inst-cc-6`

### Starlark Plugin Execution Algorithm

- [ ] `p1` - **ID**: `cpt-cf-oagw-algo-starlark-execution`

**Input**: Starlark source_code, plugin config, execution context (request/response/error)

**Output**: Modified context or rejection/custom response

**Steps**:
1. [ ] - `p1` - Create Starlark sandbox environment with timeout and memory limits - `inst-se-1`
2. [ ] - `p1` - Build ctx object with request/response/error accessors and safe mutators - `inst-se-2`
3. [ ] - `p1` - Inject ctx.config from plugin instance configuration - `inst-se-3`
4. [ ] - `p1` - Execute appropriate function based on phase: on_request, on_response, or on_error - `inst-se-4`
5. [ ] - `p1` - **IF** execution exceeds timeout - `inst-se-5`
   1. [ ] - `p1` - Terminate execution, return timeout error - `inst-se-5a`
6. [ ] - `p1` - **IF** execution exceeds memory limit - `inst-se-6`
   1. [ ] - `p1` - Terminate execution, return resource error - `inst-se-6a`
7. [ ] - `p1` - **IF** plugin calls ctx.reject(status, code, msg) - `inst-se-7`
   1. [ ] - `p1` - Halt chain, return error response - `inst-se-7a`
8. [ ] - `p1` - **IF** plugin calls ctx.respond(status, body) - `inst-se-8`
   1. [ ] - `p1` - Halt chain, return custom response - `inst-se-8a`
9. [ ] - `p1` - **IF** plugin calls ctx.next() - `inst-se-9`
   1. [ ] - `p1` - Continue to next plugin in chain - `inst-se-9a`
10. [ ] - `p1` - Apply any mutations made via ctx.request/response mutators - `inst-se-10`
11. [ ] - `p1` - **RETURN** modified context - `inst-se-11`

### Plugin Garbage Collection Algorithm

- [ ] `p2` - **ID**: `cpt-cf-oagw-algo-plugin-gc`

**Input**: GC TTL configuration (default: 30 days)

**Output**: Deleted unlinked plugins past TTL

**Steps**:
1. [ ] - `p2` - Scan all upstream and route plugins.items to collect referenced plugin UUIDs - `inst-gc-1`
2. [ ] - `p2` - Update linked plugins: set gc_eligible_at = NULL, last_used_at = NOW() - `inst-gc-2`
3. [ ] - `p2` - Mark unlinked plugins: set gc_eligible_at = NOW() + TTL where gc_eligible_at IS NULL - `inst-gc-3`
4. [ ] - `p2` - Delete plugins where gc_eligible_at IS NOT NULL AND gc_eligible_at < NOW() - `inst-gc-4`
5. [ ] - `p2` - Emit metrics: deleted_count, scan_duration - `inst-gc-5`
6. [ ] - `p2` - **RETURN** GC summary - `inst-gc-6`

## 4. States (CDSL)

### Custom Plugin Lifecycle State Machine

- [ ] `p2` - **ID**: `cpt-cf-oagw-state-plugin-lifecycle`

**States**: created, linked, unlinked, gc_eligible, deleted

**Initial State**: created

**Transitions**:
1. [ ] - `p2` - **FROM** created **TO** linked **WHEN** plugin is referenced by an upstream or route plugins.items - `inst-pl-1`
2. [ ] - `p2` - **FROM** linked **TO** unlinked **WHEN** all upstream/route references are removed - `inst-pl-2`
3. [ ] - `p2` - **FROM** unlinked **TO** gc_eligible **WHEN** GC scan sets gc_eligible_at = NOW() + TTL - `inst-pl-3`
4. [ ] - `p2` - **FROM** gc_eligible **TO** linked **WHEN** plugin is re-referenced before TTL expires - `inst-pl-4`
5. [ ] - `p2` - **FROM** gc_eligible **TO** deleted **WHEN** gc_eligible_at < NOW() and GC job runs - `inst-pl-5`
6. [ ] - `p2` - **FROM** created **TO** deleted **WHEN** explicit DELETE and plugin is not referenced - `inst-pl-6`
7. [ ] - `p2` - **FROM** unlinked **TO** deleted **WHEN** explicit DELETE - `inst-pl-7`

**Effects**:
- created: plugin exists but not yet attached to any upstream/route
- linked: plugin is actively used; gc_eligible_at = NULL, last_used_at updated
- unlinked: plugin has no references; gc_eligible_at will be set on next GC scan
- gc_eligible: plugin scheduled for deletion after TTL; can be rescued by re-linking
- deleted: plugin permanently removed from database

## 5. Definitions of Done

### Implement Plugin Type System

- [ ] `p1` - **ID**: `cpt-cf-oagw-dod-plugin-types`

The system **MUST** provide three plugin types: Auth (credential injection, one per upstream), Guard (validation/policy enforcement, can reject, multiple per level), Transform (request/response mutation, multiple per level). Plugin execution order **MUST** be: Auth → Guards → Transform(on_request) → Upstream → Transform(on_response/on_error). Plugin chain composition **MUST** follow: upstream plugins execute before route plugins.

**Implements**:
- `cpt-cf-oagw-flow-plugin-chain-execution`
- `cpt-cf-oagw-algo-chain-composition`

**Touches**:
- Entities: `AuthPlugin`, `GuardPlugin`, `TransformPlugin`, `PluginChain`

### Implement Plugin Identifier Resolution

- [ ] `p1` - **ID**: `cpt-cf-oagw-dod-plugin-resolution`

The system **MUST** resolve plugin identifiers: named builtin identifiers (e.g., `...~x.core.oagw.logging.v1`) via builtin registry, anonymous UUID identifiers (e.g., `...~{uuid}`) via oagw_plugin table lookup. Plugin type in GTS schema **MUST** match plugin_type in database.

**Implements**:
- `cpt-cf-oagw-algo-plugin-resolution`

**Touches**:
- DB: `oagw_plugin`
- Entities: `PluginRegistry`

### Implement Starlark Sandbox Execution

- [ ] `p1` - **ID**: `cpt-cf-oagw-dod-starlark-sandbox`

The system **MUST** execute custom Starlark plugins in a sandbox with no network I/O, no file I/O, no imports, enforced timeout, and enforced memory limits. The ctx API **MUST** provide safe mutators for request/response/error with control flow (next, reject, respond). Log output from ctx.log **MUST** pass through redaction and size-limiting.

**Implements**:
- `cpt-cf-oagw-algo-starlark-execution`

**Touches**:
- Entities: `StarlarkRuntime`, `PluginContext`

### Implement Plugin Immutability

- [ ] `p1` - **ID**: `cpt-cf-oagw-dod-plugin-immutability`

Custom plugins **MUST** be immutable after creation. No PUT/UPDATE endpoint for plugins. Updates **MUST** be performed by creating a new plugin version and re-binding references. The management API **MUST NOT** expose an update endpoint.

**Implements**:
- `cpt-cf-oagw-flow-create-plugin`

**Touches**:
- API: `POST /api/oagw/v1/plugins` (no PUT)
- DB: `oagw_plugin`

### Implement Plugin CRUD API

- [ ] `p2` - **ID**: `cpt-cf-oagw-dod-plugin-crud`

The system **MUST** provide create, get, list, and delete operations for plugin definitions via `/api/oagw/v1/plugins`. The system **MUST** provide a read-only endpoint to retrieve plugin source via `/api/oagw/v1/plugins/{id}/source`. Delete **MUST** return 409 PluginInUse with referenced_by lists when plugin is referenced.

**Implements**:
- `cpt-cf-oagw-flow-create-plugin`
- `cpt-cf-oagw-flow-delete-plugin`
- `cpt-cf-oagw-flow-get-plugin-source`

**Touches**:
- API: `POST/GET/DELETE /api/oagw/v1/plugins[/{id}]`, `GET /api/oagw/v1/plugins/{id}/source`
- DB: `oagw_plugin`

### Implement Plugin Garbage Collection

- [ ] `p2` - **ID**: `cpt-cf-oagw-dod-plugin-gc`

The system **MUST** implement periodic garbage collection for unlinked custom plugins. Unlinked plugins **MUST** be marked with gc_eligible_at = NOW() + TTL (default 30 days). Plugins past TTL **MUST** be automatically deleted by a background job. Re-linking a plugin before TTL **MUST** rescue it from GC.

**Implements**:
- `cpt-cf-oagw-algo-plugin-gc`
- `cpt-cf-oagw-state-plugin-lifecycle`

**Touches**:
- DB: `oagw_plugin.gc_eligible_at`, `oagw_plugin.last_used_at`
- Entities: background GC job

## 6. Acceptance Criteria

- [ ] Three plugin types (Auth, Guard, Transform) are supported with correct execution order
- [ ] Auth plugin executes once per request before guards; only one auth plugin per upstream
- [ ] Guard plugins can reject requests; rejection halts the chain
- [ ] Transform plugins execute on_request before upstream call and on_response/on_error after
- [ ] Plugin chain composition: upstream plugins execute before route plugins
- [ ] Builtin plugins are resolved by named GTS identifier from builtin registry
- [ ] Custom plugins are resolved by UUID from oagw_plugin table
- [ ] Plugin type in GTS schema must match plugin_type in database
- [ ] Custom Starlark plugins execute in sandbox: no network I/O, no file I/O, no imports
- [ ] Starlark execution is terminated on timeout or memory limit exceeded
- [ ] ctx API provides safe mutators for request/response/error
- [ ] ctx.reject() halts chain and returns error; ctx.respond() halts chain and returns custom response
- [ ] Custom plugins are immutable: no update endpoint exists
- [ ] Plugin create returns 201 with anonymous GTS identifier
- [ ] Plugin delete returns 409 PluginInUse when referenced by upstream/route
- [ ] Plugin delete returns 204 when plugin is unreferenced
- [ ] Plugin source retrieval returns Starlark source as text/plain
- [ ] Unlinked plugins are marked for GC after TTL; GC job deletes expired plugins
- [ ] Re-linking a plugin before TTL rescues it from GC
- [ ] Plugin name uniqueness is enforced per tenant

## 7. Not Applicable Sections

**Compliance (COMPL)**: Not applicable because plugin management is an infrastructure extensibility mechanism. Starlark sandbox prevents plugins from accessing regulated data directly.

**Privacy (COMPL)**: Not applicable because plugins operate on request/response data in transit. PII handling is the responsibility of the plugin author and is constrained by sandbox restrictions.

**User-Facing Architecture (UX)**: Not applicable because this is a backend plugin management system with no user-facing frontend.
