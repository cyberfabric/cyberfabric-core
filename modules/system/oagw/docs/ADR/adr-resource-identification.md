# ADR: Resource Identification and Discovery

**ID**: `cpt-cf-oagw-adr-resource-identification`

- **Status**: Proposed
- **Date**: 2026-01-28
- **Deciders**: TBD

## Context and Problem Statement

OAGW needs a resource identification strategy that satisfies multiple competing requirements:

1. **Universality**: System should be a generic tool where IDs are auto-generated (like UUIDs) or deterministically derived
2. **Observability**: Requests must be traceable in access logs - correlating inbound requests to upstreams/routes
3. **Discoverability**: Tenants need to find and reuse existing upstream/route configurations
4. **Deduplication**: When tenant defines an upstream (e.g., `api.openai.com:443`), system should match existing configurations rather than create duplicates
5. **Human-readable routing**: Support aliases for IP-based or multi-endpoint upstreams

**Example scenario**: Tenant A creates an upstream for `api.openai.com`. Later, Tenant B wants to use the same upstream. The system should:

- Recognize this is the same logical upstream
- Allow Tenant B to discover and reference it
- Maintain separate tenant-specific configurations (auth, rate limits)
- Keep logs traceable to both the shared upstream and tenant-specific usage

**Multi-endpoint scenario**: Root tenant creates upstreams for `10.0.1.1:443` and `10.0.1.2:443` with alias `my-service`. Sub-tenant creates upstream for `10.0.1.3:443` with same
alias. When subsub-tenant resolves alias `my-service`, system finds 3 upstreams - their configurations must be compatible.

## Decision Drivers

- Auto-generated or deterministic IDs (no manual naming conflicts)
- Log correlation between requests and resources
- Discovery of existing configurations
- Automatic matching/deduplication of equivalent upstreams
- Tenant isolation for configurations while sharing base definitions
- Human-readable proxy URLs via aliases or hostnames

## Considered Options

1. **UUID + Alias + Tags + Tenant Bindings** (Recommended): Three-layer approach with shared definitions and per-tenant bindings
2. **UUID Only**: System-generated UUIDs with no human-readable identifiers
3. **Name-Based IDs**: Human-assigned names as primary identifiers
4. **Hash-Based Deduplication**: Content-addressable IDs derived from upstream configuration

## Decision Outcome

**Chosen**: Option 1 — UUID + Alias + Tags + Tenant Bindings

Separate concerns into three layers:

**Layer 1: Upstream Definition (shared, immutable core)**

- System-generated UUID as primary ID
- Alias for human-readable routing (defaults to hostname)
- Flat tags for discovery (e.g., `openai`, `llm`, `completion`)

**Layer 2: Tenant Binding (per-tenant configuration)**

- Links tenant to upstream definition
- Holds tenant-specific: auth config, rate limits, plugins
- Has its own UUID for log correlation

**Layer 3: Request Context (runtime)**

- Request ID for tracing
- References both binding ID and upstream ID

```
┌─────────────────────────────────────────────────────────────┐
│                    Upstream Definition                       │
│  id: "01234567-..."  (UUID)                                 │
│  alias: "api.openai.com" (defaults to host, or explicit)    │
│  server: { host: "api.openai.com", port: 443 }              │
│  tags: ["openai", "llm", "chat"]                         │
└─────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
┌──────────────────────────┐    ┌──────────────────────────┐
│   Tenant A Binding       │    │   Tenant B Binding       │
│   id: "binding-uuid-a"   │    │   id: "binding-uuid-b"   │
│   upstream_id: "01234..."│    │   upstream_id: "01234..."│
│   auth: { apikey: ... }  │    │   auth: { apikey: ... }  │
│   rate_limit: { ... }    │    │   rate_limit: { ... }    │
└──────────────────────────┘    └──────────────────────────┘
```

### Upstream Alias

Alias provides human-readable identifier for proxy URLs.

**Alias Resolution Rules**:

| Scenario                          | Enforced Alias            | Example                                                |
|-----------------------------------|---------------------------|--------------------------------------------------------|
| Single host                       | `hostname` (without port) | `api.openai.com:443` → alias: `api.openai.com`         |
| Multiple hosts with common suffix | Common domain suffix      | `us.vendor.com`, `eu.vendor.com` → alias: `vendor.com` |
| No common suffix or IP addresses  | Explicit alias required   | `10.0.1.1`, `10.0.1.2` → alias: `my-service`           |

**Algorithm for Common Suffix Extraction**:

```
Given endpoints: ["us.vendor.com", "eu.vendor.com", "ap.vendor.com"]

1. Split each hostname by dots: [["us", "vendor", "com"], ["eu", "vendor", "com"], ["ap", "vendor", "com"]]
2. Find common suffix: ["vendor", "com"]
3. Join with dots: "vendor.com"
4. If common suffix length < 2, require explicit alias
```

**Default Behavior**:

- Single endpoint: alias defaults to `server.endpoints[0].host` (without port)
- Multiple endpoints with common suffix: alias defaults to common suffix (min 2 components)
- IP-based endpoints or no common suffix: explicit alias is mandatory

**Shadowing**: Alias in child tenant shadows same alias in parent tenant (closest wins)

**Alias Resolution Order** (closest to tenant wins):

```
Request from: subsub-tenant
Alias: "api.openai.com"

Resolution order:
1. subsub-tenant's upstreams  ← wins if found
2. sub-tenant's upstreams
3. root-tenant's upstreams
```

**Shadowing example**:

```
Root Tenant:
┌────────────────────────────────────────┐
│  Upstream A                            │
│  alias: "api.openai.com"               │
│  server: api.openai.com:443            │
│  rate_limit: { rate: 10000/min }       │
└────────────────────────────────────────┘

Sub Tenant:
┌────────────────────────────────────────┐
│  Upstream B (shadows A)                │
│  alias: "api.openai.com"               │
│  server: api.openai.com:8443           │  ← different port
│  rate_limit: { rate: 100/min }         │
└────────────────────────────────────────┘
```

When sub-tenant requests `/proxy/api.openai.com/...`:

- Resolves to Upstream B (port 8443)
- Root's Upstream A is shadowed (not used)

**Port differentiation** - use explicit alias when same host, different ports:

```json
// Upstream for api.openai.com:443
{
  "server": { "endpoints": [ { "host": "api.openai.com", "port": 443 } ] },
  "alias": "openai-prod"
}

// Upstream for api.openai.com:8443 (staging)
{
  "server": { "endpoints": [ { "host": "api.openai.com", "port": 8443 } ] },
  "alias": "openai-staging"
}
```

**Alias uniqueness rule**:

- Within same tenant: alias must be unique
- Across tenant hierarchy: child can shadow parent's alias (intentional override)

**Examples**:

**Single hostname (alias auto-generated)**:

```json
{
  "server": { "endpoints": [ { "host": "api.openai.com", "port": 443 } ] }
}
```

→ System sets `alias = "api.openai.com"`

**Multi-region with common suffix (alias auto-generated)**:

```json
{
  "server": {
    "endpoints": [
      { "host": "us.vendor.com", "port": 443 },
      { "host": "eu.vendor.com", "port": 443 },
      { "host": "ap.vendor.com", "port": 443 }
    ]
  }
}
```

→ System sets `alias = "vendor.com"` (common suffix)

**IP-based endpoints (alias required)**:

```json
{
  "server": {
    "endpoints": [
      { "host": "10.0.1.1", "port": 443 },
      { "host": "10.0.1.2", "port": 443 }
    ]
  },
  "alias": "my-internal-service"
}
```

→ Explicit alias mandatory for IP addresses

**Heterogeneous hosts (alias required)**:

```json
{
  "server": {
    "endpoints": [
      { "host": "service-a.com", "port": 443 },
      { "host": "service-b.net", "port": 443 }
    ]
  },
  "alias": "my-service-pool"
}
```

→ No common suffix, explicit alias required

**Multi-endpoint with shared alias** (load balancing pool):

```
Root Tenant:
┌────────────────────────────┐  ┌────────────────────────────┐
│  Upstream A                │  │  Upstream B                │
│  alias: "my-service"       │  │  alias: "my-service"       │
│  server: 10.0.1.1:443      │  │  server: 10.0.1.2:443      │
│  protocol: https           │  │  protocol: https           │
└────────────────────────────┘  └────────────────────────────┘

Sub Tenant:
┌────────────────────────────┐
│  Upstream C                │
│  alias: "my-service"       │
│  server: 10.0.1.3:443      │
│  protocol: https           │
└────────────────────────────┘
```

### Alias Compatibility Validation

When resolving alias through tenant hierarchy (subsub→sub→root), the closest match wins (shadowing). However, **enforced limits from ancestors still apply**.

**Limit enforcement across shadowing**:

```
Root Tenant:
┌────────────────────────────────────────┐
│  Upstream A                            │
│  alias: "api.openai.com"               │
│  rate_limit: { sharing: "enforce",     │
│                rate: 10000/min }       │
└────────────────────────────────────────┘

Sub Tenant:
┌────────────────────────────────────────┐
│  Upstream B (shadows A)                │
│  alias: "api.openai.com"               │
│  rate_limit: { rate: 500/min }         │
└────────────────────────────────────────┘
```

**Resolution for sub-tenant**:

1. Find Upstream B (closest match for alias)
2. Walk hierarchy, collect enforced limits from ancestors with same alias
3. Apply `min(enforced_ancestor_limits, own_limit)` = `min(10000, 500)` = 500/min

**When shadowing doesn't inherit enforcement** - different alias:

```
Root: alias "openai-shared", rate_limit: { sharing: "enforce", rate: 10000 }
Sub:  alias "openai-private" (different alias - no enforcement inheritance)
```

**Multi-endpoint with shared alias** (load balancing pool):

When multiple upstreams share the same alias **within same tenant or explicitly configured for pooling**, they form a load-balance pool:

```
Root Tenant:
┌────────────────────────────┐  ┌────────────────────────────┐
│  Upstream A                │  │  Upstream B                │
│  alias: "my-service"       │  │  alias: "my-service"       │
│  server: 10.0.1.1:443      │  │  server: 10.0.1.2:443      │
│  protocol: https           │  │  protocol: https           │
└────────────────────────────┘  └────────────────────────────┘
```

**Compatibility rules** - upstreams pooled under same alias MUST have identical:

| Field                       | Reason                          |
|-----------------------------|---------------------------------|
| `protocol`                  | Can't mix HTTP and gRPC         |
| `server.endpoints[].scheme` | Can't mix https and wss         |
| `server.endpoints[].port`   | Can't load-balance across ports |

**Resolution flow**:

```
Request: POST /api/oagw/v1/proxy/api.openai.com/v1/chat/completions
Tenant: sub-tenant

1. Search alias "api.openai.com" starting from sub-tenant
2. Find Upstream B in sub-tenant (closest) → use this upstream
3. Collect enforced configs from ancestors with same alias:
   - Root has Upstream A with alias "api.openai.com" 
   - Root's rate_limit.sharing = "enforce" → collect
4. Merge: effective_rate = min(root.enforced:10000, sub:500) = 500
5. Find tenant binding, apply merged config
6. Forward request to Upstream B's server
```

**Incompatibility error**:

```json
{
  "error": {
    "code": "ALIAS_INCOMPATIBLE",
    "message": "Upstreams with alias 'my-service' have incompatible configurations",
    "details": {
      "conflicts": [
        { "field": "protocol", "values": [ "https", "grpc" ], "upstreams": [ "uuid-a", "uuid-c" ] }
      ]
    }
  }
}
```

### Schema Changes

**Upstream Definition**:

```json
{
  "id": {
    "type": "string",
    "format": "uuid",
    "readOnly": true,
    "description": "System-generated unique identifier."
  },
  "alias": {
    "type": "string",
    "pattern": "^[a-z0-9]([a-z0-9.-]*[a-z0-9])?$",
    "description": "Human-readable routing identifier. Defaults to server host if not specified. Required for IP-based endpoints."
  },
  "tags": {
    "type": "array",
    "items": {
      "type": "string",
      "pattern": "^[a-z0-9_-]+$"
    },
    "description": "Flat tags for categorization and discovery (e.g., openai, llm)."
  }
}
```

**Tenant Upstream Binding**:

```json
{
  "id": {
    "type": "string",
    "format": "uuid",
    "description": "Binding identifier for log correlation."
  },
  "tenant_id": {
    "type": "string",
    "format": "uuid"
  },
  "upstream_id": {
    "type": "string",
    "format": "uuid",
    "description": "Reference to upstream definition."
  },
  "auth": { "$ref": "#/definitions/auth" },
  "rate_limit": { "$ref": "#/definitions/rate_limit" },
  "plugins": { "type": "array", "items": { "type": "string" } }
}
```

### API Flow

**Tenant creates upstream (hostname-based)**:

```http
POST /api/oagw/v1/upstreams
Content-Type: application/json

{
  "server": {
    "endpoints": [{ "scheme": "https", "host": "api.openai.com", "port": 443 }]
  },
  "protocol": "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
  "tags": ["openai", "llm", "chat"],
  "auth": { "type": "...", "config": { ... } }
}
```

**Response** (alias defaults to host):

```json
{
  "upstream": {
    "id": "01234567-...",
    "alias": "api.openai.com",
    "matched": true
  },
  "binding": { "id": "binding-uuid", "created": true }
}
```

**Tenant creates upstream (IP-based with explicit alias)**:

```http
POST /api/oagw/v1/upstreams
Content-Type: application/json

{
  "server": {
    "endpoints": [{ "scheme": "https", "host": "10.0.1.1", "port": 443 }]
  },
  "protocol": "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
  "alias": "my-internal-service",
  "tags": ["internal", "api"]
}
```

**Discovery**:

```http
GET /api/oagw/v1/upstreams?tags=openai,llm
GET /api/oagw/v1/upstreams?alias=my-internal-service
GET /api/oagw/v1/upstreams?host=api.openai.com
```

### Proxy URL

```
POST /api/oagw/v1/proxy/{alias}/{path_suffix}
```

**Examples**:

```
# Hostname as alias (default)
POST /api/oagw/v1/proxy/api.openai.com/v1/chat/completions

# Explicit alias for IP-based upstream
POST /api/oagw/v1/proxy/my-internal-service/api/v1/users

# Multi-endpoint pool via shared alias
POST /api/oagw/v1/proxy/my-service/health
```

**Resolution flow**:

```
Inbound: POST /api/oagw/v1/proxy/my-service/api/v1/users
                                 └───┬────┘└─────┬─────┘
                                  alias      path_suffix

1. Search alias "my-service" in tenant hierarchy
2. Collect all matching upstreams
3. Validate compatibility
4. Select endpoint from pool
5. Find tenant binding by (tenant_id, upstream_id)
6. Match route by (upstream_id, method, path_suffix)
7. Apply tenant config (auth, rate_limit, plugins)
8. Forward to upstream
```

**Access log**:

```
timestamp=2026-01-28T17:30:00Z
request_id=req-uuid
tenant_id=tenant-a
binding_id=binding-uuid-a
upstream_id=01234567-...
upstream_alias=my-service
upstream_host=10.0.1.2
route_id=route-uuid
method=POST
path=/api/v1/users
status=200
latency_ms=234
```

### Consequences

**Positive**:

- Human-readable proxy URLs via aliases
- Automatic deduplication of equivalent upstreams
- Tenant isolation with shared base definitions
- Log correlation via binding IDs and upstream IDs
- Discoverable resources via tags and alias search

**Negative**:

- Alias shadowing across tenant hierarchy adds complexity
- Compatibility validation needed for multi-endpoint pools
- Alias uniqueness constraint within tenant scope

### Confirmation

Confirmation will be achieved through:

- Integration tests for alias resolution across tenant hierarchy (shadowing, enforcement)
- Tests for automatic alias generation (hostname, common suffix, IP-based)
- Compatibility validation tests for multi-endpoint alias pools

## Pros and Cons of the Options

### UUID + Alias + Tags + Tenant Bindings

- **Good**: Human-readable routing, automatic deduplication, tenant isolation, flexible discovery
- **Bad**: Alias shadowing complexity, compatibility validation overhead

### UUID Only

- **Good**: Simple, no naming conflicts
- **Bad**: Not human-readable, poor developer experience for proxy URLs

### Name-Based IDs

- **Good**: Human-readable
- **Bad**: Naming conflicts across tenants, no automatic deduplication

### Hash-Based Deduplication

- **Good**: Automatic deduplication, deterministic
- **Bad**: Brittle (config changes break references), not human-readable

## Links

- [OAGW Design Document](../DESIGN.md)
- [Kubernetes Labels and Selectors](https://kubernetes.io/docs/concepts/overview/working-with-objects/labels/)

---

## Use Cases

### UC1: Tenant Hierarchy Configuration Merging

**Scenario**: Partner tenant creates upstream for `api.openai.com` with rate limits and shared auth. Customer tenant wants to use the same upstream with minimal configuration.

```
Partner Tenant (parent):
┌─────────────────────────────────────────────────────────────┐
│  Upstream: api.openai.com                                   │
│  Binding:                                                   │
│    auth: { type: "apikey", sharing: "inherit" }             │
│    rate_limit: { rate: 1000/min, sharing: "inherit" }       │
│    plugins: [logging, metrics]                              │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
Customer Tenant (child):
┌─────────────────────────────────────────────────────────────┐
│  Binding (inherits upstream):                               │
│    auth: null  (inherit from parent)                        │
│    rate_limit: { rate: 100/min }  (override, more strict)   │
│    plugins: [custom-transform]  (extend)                    │
└─────────────────────────────────────────────────────────────┘
```

#### Configuration Sharing Modes

Each configuration field in a binding can specify a sharing mode:

| Mode      | Description                                             |
|-----------|---------------------------------------------------------|
| `private` | Not visible to child tenants (default)                  |
| `inherit` | Child can inherit; child's value overrides if specified |
| `enforce` | Child must use parent's value; cannot override          |

#### Auth & Secrets

Auth configuration references secrets via `cred_store` (Vault). OAGW does not manage secret sharing - that's `cred_store` responsibility.

```json
{
  "auth": {
    "type": "gts.x.core.oagw.auth_plugin.v1~x.core.oagw.apikey.v1",
    "config": {
      "header": "Authorization",
      "prefix": "Bearer ",
      "secret_ref": "cred://openai-api-key"
    }
  }
}
```

**Resolution flow**:

1. OAGW resolves `secret_ref` via `cred_store` API
2. `cred_store` checks if secret is accessible to current tenant (own or shared by ancestor)
3. If accessible → return secret material
4. If not → return error, OAGW returns 401

This means:

- Parent can share a secret with children via `cred_store` policies
- Child references same `secret_ref` - `cred_store` handles access check
- Child can also use own secret with different `secret_ref`

**Example**: Partner shares OpenAI key with customers

```
Partner Tenant:
  - Creates secret "openai-api-key" in cred_store
  - Sets sharing policy: { visibility: "inherit" }
  - Creates upstream binding with secret_ref: "cred://openai-api-key"

Customer Tenant:
  - Creates binding for same upstream
  - Option A: Use same secret_ref "cred://openai-api-key" (cred_store allows)
  - Option B: Use own secret "cred://my-openai-key"
```

#### Field-Level Merge Strategies

**Rate Limit Configuration**:

| Parent Sharing | Child Value | Result                                |
|----------------|-------------|---------------------------------------|
| `private`      | null        | No rate limit from parent             |
| `private`      | set         | Use child's limit                     |
| `inherit`      | null        | Use parent's limit                    |
| `inherit`      | set         | Use **min**(parent, child) - stricter |
| `enforce`      | null        | Use parent's limit                    |
| `enforce`      | set         | Use **min**(parent, child) - stricter |

**Plugins Configuration**:

| Parent Sharing | Child Value | Result                                 |
|----------------|-------------|----------------------------------------|
| `private`      | any         | Use child's plugins only               |
| `inherit`      | null        | Use parent's plugins                   |
| `inherit`      | set         | Concat: parent.plugins + child.plugins |
| `enforce`      | null        | Use parent's plugins                   |
| `enforce`      | set         | Concat: parent.plugins + child.plugins |

#### Binding Schema with Sharing

```json
{
  "id": { "type": "string", "format": "uuid" },
  "tenant_id": { "type": "string", "format": "uuid" },
  "upstream_id": { "type": "string", "format": "uuid" },
  "auth": {
    "type": "object",
    "description": "Auth config with secret_ref resolved by cred_store.",
    "properties": {
      "type": { "type": "string" },
      "config": {
        "type": "object",
        "properties": {
          "secret_ref": {
            "type": "string",
            "format": "uri",
            "pattern": "^cred://",
            "description": "Reference to secret in cred_store. Access checked at runtime."
          }
        }
      }
    }
  },
  "rate_limit": {
    "type": "object",
    "properties": {
      "sharing": {
        "type": "string",
        "enum": [ "private", "inherit", "enforce" ],
        "default": "private"
      },
      "rate": { "type": "integer" },
      "window": { "type": "string" }
    }
  },
  "plugins": {
    "type": "object",
    "properties": {
      "sharing": {
        "type": "string",
        "enum": [ "private", "inherit", "enforce" ],
        "default": "private"
      },
      "items": {
        "type": "array",
        "items": { "type": "string" }
      }
    }
  }
}
```

#### API Examples

**Partner creates upstream with shareable rate limits**:

```http
POST /api/oagw/v1/upstreams
X-Tenant-ID: partner-uuid

{
  "server": {
    "endpoints": [{ "scheme": "https", "host": "api.openai.com", "port": 443 }]
  },
  "protocol": "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
  "tags": ["openai", "llm"],
  "auth": {
    "type": "gts.x.core.oagw.auth_plugin.v1~x.core.oagw.apikey.v1",
    "config": { 
      "header": "Authorization", 
      "prefix": "Bearer ",
      "secret_ref": "cred://openai-api-key"
    }
  },
  "rate_limit": {
    "sharing": "enforce",
    "rate": 1000,
    "window": "minute"
  },
  "plugins": {
    "sharing": "inherit",
    "items": ["gts.x.core.oagw.filter_plugin.v1~x.core.oagw.logging.v1"]
  }
}
```

**Customer binds using shared secret (from cred_store)**:

```http
POST /api/oagw/v1/upstreams
X-Tenant-ID: customer-uuid

{
  "server": {
    "endpoints": [{ "scheme": "https", "host": "api.openai.com", "port": 443 }]
  },
  "protocol": "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
  "auth": {
    "type": "gts.x.core.oagw.auth_plugin.v1~x.core.oagw.apikey.v1",
    "config": { 
      "header": "Authorization", 
      "prefix": "Bearer ",
      "secret_ref": "cred://openai-api-key"
    }
  },
  "rate_limit": {
    "rate": 100,
    "window": "minute"
  }
}
```

**Resolved effective config for customer**:

```json
{
  "upstream_id": "...",
  "effective_config": {
    "auth": {
      "type": "apikey",
      "config": {
        "secret_ref": "cred://openai-api-key",
        "note": "Access validated by cred_store at runtime"
      }
    },
    "rate_limit": {
      "source": "merged",
      "rate": 100,
      "window": "minute",
      "note": "min(parent.enforce:1000, child:100) = 100"
    },
    "plugins": {
      "source": "inherited:partner-uuid",
      "items": [ "logging" ]
    }
  }
}
```

**Customer with own API key**:

```http
POST /api/oagw/v1/upstreams
X-Tenant-ID: customer-uuid

{
  "server": {
    "endpoints": [{ "scheme": "https", "host": "api.openai.com", "port": 443 }]
  },
  "protocol": "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1",
  "auth": {
    "type": "gts.x.core.oagw.auth_plugin.v1~x.core.oagw.apikey.v1",
    "config": { 
      "header": "Authorization", 
      "prefix": "Bearer ",
      "secret_ref": "cred://my-own-openai-key"
    }
  }
}
```

#### Config Resolution Algorithm

```go
func resolveEffectiveConfig(tenantID, upstreamID string) EffectiveConfig {
    // 1. Walk tenant hierarchy from child to root
    hierarchy := getTenantHierarchy(tenantID) // [child, parent, grandparent, ...]
    
    // 2. Collect bindings for this upstream
    bindings := []Binding{}
    for _, tid := range hierarchy {
        if b := findBinding(tid, upstreamID); b != nil {
            bindings = append(bindings, b)
        }
    }
    
    // 3. Merge from root to child (root is base, child overrides)
    result := EffectiveConfig{}
    for i := len(bindings) - 1; i >= 0; i-- {
        b := bindings[i]
        isOwn := (i == 0)
        
        // Auth - no inheritance, each tenant specifies own (secret access via cred_store)
        if isOwn && b.Auth != nil {
            result.Auth = b.Auth
        }
        
        // Rate limit - merge with sharing rules
        result.RateLimit = mergeRateLimit(result.RateLimit, b.RateLimit, isOwn)
        
        // Plugins - merge with sharing rules
        result.Plugins = mergePlugins(result.Plugins, b.Plugins, isOwn)
    }
    
    return result
}

func mergeRateLimit(parent, child *RateLimitConfig, isOwn bool) *RateLimitConfig {
    if parent == nil {
        return child
    }
    if child == nil {
        if parent.Sharing == "private" && !isOwn {
            return nil
        }
        return parent
    }
    
    // Both exist - take stricter (minimum)
    if parent.Sharing == "enforce" || parent.Sharing == "inherit" {
        return &RateLimitConfig{
            Rate:   min(parent.Rate, child.Rate),
            Window: parent.Window,
        }
    }
    return child
}
```
