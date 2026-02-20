# Feature: Multi-Tenant Configuration Hierarchy

- [ ] `p1` - **ID**: `cpt-cf-oagw-featstatus-multi-tenant-config`

- [ ] `p1` - `cpt-cf-oagw-feature-multi-tenant-config`

## 1. Feature Context

### 1.1 Overview

Hierarchical configuration layering, sharing modes (private/inherit/enforce), alias resolution with tenant hierarchy walk and shadowing, and field-specific merge strategies for auth, rate limits, plugins, and tags across ancestor-descendant tenant relationships.

### 1.2 Purpose

CyberFabric is a multi-tenant platform where ancestor tenants (partners, root) define upstreams that descendant tenants (customers) can inherit and selectively override. This feature enables fine-grained configuration sharing without duplicating settings at every level, while allowing ancestors to enforce policies that descendants cannot bypass.

Addresses PRD requirements: `cpt-cf-oagw-fr-config-layering`, `cpt-cf-oagw-fr-config-hierarchy`, `cpt-cf-oagw-fr-alias-resolution`.

### 1.3 Actors

| Actor | Role in Feature |
|-------|-----------------|
| `cpt-cf-oagw-actor-platform-operator` | Configures root-level upstreams with sharing modes and enforced policies |
| `cpt-cf-oagw-actor-tenant-admin` | Creates bindings to ancestor upstreams, overrides inherited configuration where permitted |
| `cpt-cf-oagw-actor-app-developer` | Sends proxy requests resolved through tenant hierarchy alias resolution |

### 1.4 References

- **PRD**: [PRD.md](../PRD.md)
- **Design**: [DESIGN.md](../DESIGN.md)
- **Requirements**: `cpt-cf-oagw-fr-config-layering`, `cpt-cf-oagw-fr-config-hierarchy`, `cpt-cf-oagw-fr-alias-resolution`
- **Design elements**: `cpt-cf-oagw-design-oagw`, `cpt-cf-oagw-adr-request-routing`, `cpt-cf-oagw-adr-resource-identification`, `cpt-cf-oagw-adr-rate-limiting`
- **Dependencies**: `cpt-cf-oagw-feature-core-config-mgmt` (upstream/route entities must exist), `cpt-cf-oagw-feature-proxy-auth` (alias resolution is invoked during proxy flow)

## 2. Actor Flows (CDSL)

### Alias Resolution During Proxy Flow

- [ ] `p1` - **ID**: `cpt-cf-oagw-flow-alias-resolution`

**Actor**: `cpt-cf-oagw-actor-app-developer`

**Success Scenarios**:
- Upstream is resolved by alias walking tenant hierarchy from descendant to root
- Closest match (shadowing) wins as routing target
- Ancestor enforced constraints are collected for effective config merge

**Error Scenarios**:
- Alias not found in any tenant in hierarchy (404 RouteNotFound)
- Upstream found but disabled (503 upstream_disabled)
- Ancestor disabled the alias (upstream not visible)
- Multi-endpoint common-suffix alias without required X-OAGW-Target-Host header (400)

**Steps**:
1. [ ] - `p1` - Extract alias from proxy URL path - `inst-ar-1`
2. [ ] - `p1` - Get tenant hierarchy array [child, parent, grandparent, ..., root] - `inst-ar-2`
3. [ ] - `p1` - DB: SELECT oagw_upstream WHERE alias = :alias AND tenant_id = ANY(:hierarchy) - `inst-ar-3`
4. [ ] - `p1` - Order matches by position in hierarchy (closest first) - `inst-ar-4`
5. [ ] - `p1` - **IF** no matches found - `inst-ar-5`
   1. [ ] - `p1` - **RETURN** 404 RouteNotFound - `inst-ar-5a`
6. [ ] - `p1` - Check enabled inheritance: if any ancestor (higher in hierarchy) has disabled this alias, exclude descendant matches - `inst-ar-6`
7. [ ] - `p1` - Select closest match as routing target - `inst-ar-7`
8. [ ] - `p1` - **IF** selected upstream is disabled - `inst-ar-8`
   1. [ ] - `p1` - **RETURN** 503 upstream_disabled with upstream_id in error detail - `inst-ar-8a`
9. [ ] - `p1` - **IF** multi-endpoint upstream with common-suffix alias and X-OAGW-Target-Host header missing - `inst-ar-9`
   1. [ ] - `p1` - **RETURN** 400 missing_target_host with valid_hosts list - `inst-ar-9a`
10. [ ] - `p1` - **IF** X-OAGW-Target-Host present, validate against endpoint allowlist - `inst-ar-10`
11. [ ] - `p1` - **IF** header value does not match any endpoint - `inst-ar-11`
    1. [ ] - `p1` - **RETURN** 400 unknown_target_host with valid_hosts list - `inst-ar-11a`
12. [ ] - `p1` - Collect ancestor matches with sharing=enforce constraints - `inst-ar-12`
13. [ ] - `p1` - **RETURN** ResolvedAlias (upstream, matched_endpoint, enforced_ancestors) - `inst-ar-13`

### Create Descendant Binding Flow

- [ ] `p1` - **ID**: `cpt-cf-oagw-flow-create-binding`

**Actor**: `cpt-cf-oagw-actor-tenant-admin`

**Success Scenarios**:
- Descendant creates an upstream with the same alias as an ancestor, establishing a binding with optional overrides
- Overrides are gated by permissions (override_auth, override_rate, add_plugins)

**Error Scenarios**:
- Descendant lacks `oagw:upstream:bind` permission (403)
- Descendant attempts to override auth without `oagw:upstream:override_auth` permission (403)
- Descendant attempts to override enforced field (400)

**Steps**:
1. [ ] - `p1` - Tenant admin sends POST /api/oagw/v1/upstreams with alias matching an ancestor upstream - `inst-cb-1`
2. [ ] - `p1` - Extract SecurityContext (tenant_id, permissions) - `inst-cb-2`
3. [ ] - `p1` - Detect that alias matches an ancestor upstream (binding flow) - `inst-cb-3`
4. [ ] - `p1` - **IF** tenant lacks `oagw:upstream:bind` permission - `inst-cb-4`
   1. [ ] - `p1` - **RETURN** 403 Forbidden - `inst-cb-4a`
5. [ ] - `p1` - **IF** request includes auth override and tenant lacks `oagw:upstream:override_auth` - `inst-cb-5`
   1. [ ] - `p1` - **RETURN** 403 Forbidden - `inst-cb-5a`
6. [ ] - `p1` - **IF** request includes rate_limit override and tenant lacks `oagw:upstream:override_rate` - `inst-cb-6`
   1. [ ] - `p1` - **RETURN** 403 Forbidden - `inst-cb-6a`
7. [ ] - `p1` - **IF** request includes plugins and tenant lacks `oagw:upstream:add_plugins` - `inst-cb-7`
   1. [ ] - `p1` - **RETURN** 403 Forbidden - `inst-cb-7a`
8. [ ] - `p1` - **IF** ancestor field has sharing=enforce and descendant attempts override - `inst-cb-8`
   1. [ ] - `p1` - **RETURN** 400 ValidationError: cannot override enforced field - `inst-cb-8a`
9. [ ] - `p1` - DB: INSERT oagw_upstream with descendant-specific overrides (tags treated as local additions) - `inst-cb-9`
10. [ ] - `p1` - **RETURN** 201 Created with binding upstream resource - `inst-cb-10`

## 3. Processes / Business Logic (CDSL)

### Configuration Resolution Algorithm

- [ ] `p1` - **ID**: `cpt-cf-oagw-algo-config-resolution`

**Input**: tenant_id, upstream_id (resolved by alias)

**Output**: Effective configuration (auth, rate_limit, plugins, tags) after hierarchical merge

**Steps**:
1. [ ] - `p1` - Walk tenant hierarchy from descendant to root: [child, parent, ..., root] - `inst-cr-1`
2. [ ] - `p1` - Collect bindings for this upstream alias across hierarchy - `inst-cr-2`
3. [ ] - `p1` - Initialize empty EffectiveConfig - `inst-cr-3`
4. [ ] - `p1` - Merge from root to child (root is base, child overrides) - `inst-cr-4`
5. [ ] - `p1` - **FOR EACH** binding from root to child - `inst-cr-5`
   1. [ ] - `p1` - Merge auth using Auth Merge strategy - `inst-cr-5a`
   2. [ ] - `p1` - Merge rate_limit using Rate Limit Merge strategy - `inst-cr-5b`
   3. [ ] - `p1` - Merge plugins using Plugin Merge strategy - `inst-cr-5c`
   4. [ ] - `p1` - Merge tags using additive union (top-to-bottom) - `inst-cr-5d`
6. [ ] - `p1` - **RETURN** EffectiveConfig - `inst-cr-6`

### Auth Merge Strategy Algorithm

- [ ] `p1` - **ID**: `cpt-cf-oagw-algo-merge-auth`

**Input**: Ancestor auth config (with sharing mode), descendant auth config, is_own flag

**Output**: Effective auth config

**Steps**:
1. [ ] - `p1` - **IF** ancestor auth is null or sharing = private - `inst-ma-1`
   1. [ ] - `p1` - Descendant must provide own auth - `inst-ma-1a`
2. [ ] - `p1` - **IF** ancestor sharing = inherit and descendant specifies auth with secret_ref - `inst-ma-2`
   1. [ ] - `p1` - Use descendant auth (override) - `inst-ma-2a`
3. [ ] - `p1` - **IF** ancestor sharing = inherit and descendant does not specify auth - `inst-ma-3`
   1. [ ] - `p1` - Use ancestor auth - `inst-ma-3a`
4. [ ] - `p1` - **IF** ancestor sharing = enforce - `inst-ma-4`
   1. [ ] - `p1` - Use ancestor auth (cannot override) - `inst-ma-4a`
5. [ ] - `p1` - **RETURN** effective auth config - `inst-ma-5`

### Rate Limit Merge Strategy Algorithm

- [ ] `p1` - **ID**: `cpt-cf-oagw-algo-merge-rate-limit`

**Input**: Ancestor rate_limit config (with sharing mode), descendant rate_limit config, is_own flag

**Output**: Effective rate limit (min-merge for enforce/inherit)

**Steps**:
1. [ ] - `p1` - **IF** ancestor is null, use descendant - `inst-mrl-1`
2. [ ] - `p1` - **IF** descendant is null - `inst-mrl-2`
   1. [ ] - `p1` - **IF** ancestor sharing = private and not is_own, result is null - `inst-mrl-2a`
   2. [ ] - `p1` - **ELSE** use ancestor - `inst-mrl-2b`
3. [ ] - `p1` - **IF** both exist and ancestor sharing = enforce or inherit - `inst-mrl-3`
   1. [ ] - `p1` - effective.rate = min(ancestor.rate, descendant.rate) - `inst-mrl-3a`
   2. [ ] - `p1` - effective.window = ancestor.window - `inst-mrl-3b`
4. [ ] - `p1` - **IF** ancestor sharing = private, use descendant only - `inst-mrl-4`
5. [ ] - `p1` - **RETURN** effective rate limit - `inst-mrl-5`

### Plugin Merge Strategy Algorithm

- [ ] `p1` - **ID**: `cpt-cf-oagw-algo-merge-plugins`

**Input**: Ancestor plugins config (with sharing mode), descendant plugins config, is_own flag

**Output**: Effective plugin chain (ancestor + descendant concatenation)

**Steps**:
1. [ ] - `p1` - Initialize result as empty list - `inst-mp-1`
2. [ ] - `p1` - **IF** ancestor plugins is not null and sharing != private - `inst-mp-2`
   1. [ ] - `p1` - Append ancestor plugin items to result - `inst-mp-2a`
3. [ ] - `p1` - **IF** descendant plugins is not null - `inst-mp-3`
   1. [ ] - `p1` - Append descendant plugin items to result - `inst-mp-3a`
4. [ ] - `p1` - **RETURN** concatenated plugin chain - `inst-mp-4`

### Tags Merge Algorithm

- [ ] `p1` - **ID**: `cpt-cf-oagw-algo-merge-tags`

**Input**: Ancestor tags, descendant tags

**Output**: Effective tags (additive union, no removal)

**Steps**:
1. [ ] - `p1` - Initialize result as empty set - `inst-mt-1`
2. [ ] - `p1` - **IF** ancestor tags is not null, add all to result - `inst-mt-2`
3. [ ] - `p1` - **IF** descendant tags is not null, add all to result - `inst-mt-3`
4. [ ] - `p1` - **RETURN** sorted union of tags - `inst-mt-4`

### Alias Shadowing Resolution Algorithm

- [ ] `p1` - **ID**: `cpt-cf-oagw-algo-alias-shadowing`

**Input**: Tenant hierarchy, alias, upstream matches from DB

**Output**: Selected upstream (closest match) with enforced ancestor constraints

**Steps**:
1. [ ] - `p1` - Order matches by position in tenant hierarchy (closest to requesting tenant first) - `inst-as-1`
2. [ ] - `p1` - Check enabled inheritance: exclude matches where any ancestor has disabled the alias - `inst-as-2`
3. [ ] - `p1` - Select first (closest) match as routing target - `inst-as-3`
4. [ ] - `p1` - Collect remaining matches that have sharing=enforce constraints - `inst-as-4`
5. [ ] - `p1` - **RETURN** selected upstream + enforced ancestors list - `inst-as-5`

## 4. States (CDSL)

### Sharing Mode State Machine

- [ ] `p1` - **ID**: `cpt-cf-oagw-state-sharing-mode`

**States**: private, inherit, enforce

**Initial State**: private (default)

**Transitions**:
1. [ ] - `p1` - **FROM** private **TO** inherit **WHEN** operator updates sharing field to inherit - `inst-sm-1`
2. [ ] - `p1` - **FROM** private **TO** enforce **WHEN** operator updates sharing field to enforce - `inst-sm-2`
3. [ ] - `p1` - **FROM** inherit **TO** private **WHEN** operator updates sharing field to private - `inst-sm-3`
4. [ ] - `p1` - **FROM** inherit **TO** enforce **WHEN** operator updates sharing field to enforce - `inst-sm-4`
5. [ ] - `p1` - **FROM** enforce **TO** inherit **WHEN** operator updates sharing field to inherit - `inst-sm-5`
6. [ ] - `p1` - **FROM** enforce **TO** private **WHEN** operator updates sharing field to private - `inst-sm-6`

**Effects**:
- private: configuration not visible to descendant tenants; descendants must provide their own
- inherit: configuration visible to descendants; descendants can override if they have appropriate permissions
- enforce: configuration visible to descendants; descendants cannot override (hard limit applied via min-merge or direct use)

## 5. Definitions of Done

### Implement Alias Resolution with Tenant Hierarchy

- [ ] `p1` - **ID**: `cpt-cf-oagw-dod-alias-resolution`

The system **MUST** resolve upstreams by alias walking the tenant hierarchy from descendant to root. Closest match wins (shadowing). Alias uniqueness **MUST** be enforced per tenant. Multi-endpoint common-suffix aliases **MUST** require X-OAGW-Target-Host header. The system **MUST** validate X-OAGW-Target-Host against the endpoint allowlist.

**Implements**:
- `cpt-cf-oagw-flow-alias-resolution`
- `cpt-cf-oagw-algo-alias-shadowing`

**Touches**:
- API: `{METHOD} /api/oagw/v1/proxy/{alias}/*`
- DB: `oagw_upstream`
- Entities: `ControlPlaneService`

### Implement Configuration Merge with Sharing Modes

- [ ] `p1` - **ID**: `cpt-cf-oagw-dod-config-merge`

The system **MUST** merge configurations with priority order: upstream (base) < route < tenant. The system **MUST** support three sharing modes (private/inherit/enforce) with field-specific merge strategies: auth override gated by permission, rate limit min-merge, plugin chain append, tags additive union.

**Implements**:
- `cpt-cf-oagw-algo-config-resolution`
- `cpt-cf-oagw-algo-merge-auth`
- `cpt-cf-oagw-algo-merge-rate-limit`
- `cpt-cf-oagw-algo-merge-plugins`
- `cpt-cf-oagw-algo-merge-tags`
- `cpt-cf-oagw-state-sharing-mode`

**Touches**:
- DB: `oagw_upstream.auth`, `oagw_upstream.rate_limit`, `oagw_upstream.plugins`, `oagw_upstream.tags`
- Entities: `ControlPlaneService`

### Implement Descendant Binding with Override Permissions

- [ ] `p1` - **ID**: `cpt-cf-oagw-dod-binding-permissions`

The system **MUST** gate descendant override of inherited configurations by explicit permissions: `oagw:upstream:bind`, `oagw:upstream:override_auth`, `oagw:upstream:override_rate`, `oagw:upstream:add_plugins`. Without permissions, descendants **MUST** use ancestor configuration as-is.

**Implements**:
- `cpt-cf-oagw-flow-create-binding`

**Touches**:
- API: `POST /api/oagw/v1/upstreams`
- Entities: `ControlPlaneService`

### Implement Enabled Inheritance

- [ ] `p1` - **ID**: `cpt-cf-oagw-dod-enabled-inheritance`

The system **MUST** propagate ancestor disable to all descendants: if an ancestor disables an upstream alias, descendants cannot see or use it. Disabled upstreams **MUST** still be visible in management list/get operations.

**Implements**:
- `cpt-cf-oagw-flow-alias-resolution`

**Touches**:
- DB: `oagw_upstream.enabled`
- Entities: `ControlPlaneService`

## 6. Acceptance Criteria

- [ ] Alias resolution walks tenant hierarchy from descendant to root; closest match wins
- [ ] Child tenant can shadow parent's upstream by creating upstream with same alias
- [ ] Shadowing does not bypass ancestor sharing=enforce constraints (enforced rate limits still apply via min-merge)
- [ ] Alias uniqueness is enforced per tenant (not globally)
- [ ] Multi-endpoint common-suffix alias requires X-OAGW-Target-Host header; missing header returns 400 with valid_hosts
- [ ] X-OAGW-Target-Host is validated against endpoint allowlist; unknown host returns 400
- [ ] Auth with sharing=inherit can be overridden by descendant with override_auth permission
- [ ] Auth with sharing=enforce cannot be overridden by descendant
- [ ] Rate limit with sharing=enforce applies min(ancestor, descendant) merge
- [ ] Rate limit with sharing=inherit applies min(ancestor, descendant) when both specified
- [ ] Plugin chain with sharing=inherit/enforce concatenates ancestor + descendant plugins
- [ ] Tags merge is additive union; descendants cannot remove inherited tags
- [ ] Descendant without oagw:upstream:bind permission cannot create binding (403)
- [ ] Descendant without override_auth permission cannot override auth (403)
- [ ] Ancestor disable propagates: disabled ancestor alias hides upstream from descendants
- [ ] Configuration merge priority: upstream (base) < route < tenant (highest)

## 7. Not Applicable Sections

**Compliance (COMPL)**: Not applicable because hierarchical configuration management does not handle regulated data. Audit logging provides the compliance trail for configuration changes.

**Privacy (COMPL)**: Not applicable because configuration sharing modes control visibility of infrastructure settings, not PII. Credential references use UUID-only secret_ref.

**User-Facing Architecture (UX)**: Not applicable because this is a backend multi-tenant configuration system with no user-facing frontend.
