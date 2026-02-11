# Management Operations Flow

## Overview

Management operations (CRUD for upstreams, routes, plugins) are routed directly from API Handler to Control Plane. This document describes the request flow and cache invalidation behavior.

## Request Flow

```
Client Request
  → API Handler (auth, rate limit)
    → Control Plane (validate, write DB, invalidate cache)
      → Response
```

## Component Responsibilities

### API Handler
- Validates Bearer token
- Applies inbound rate limiting
- Routes request to Control Plane based on path pattern:
  - `/api/oagw/v1/upstreams/*` → DP
  - `/api/oagw/v1/routes/*` → DP
  - `/api/oagw/v1/plugins/*` → DP

### Control Plane
- Validates request payload against schema
- Checks tenant authorization (via SecureConn scoping)
- Writes to database
- Invalidates affected cache entries (L1 and L2)
- Returns success/error response

## Example: Create Upstream

### Request

```http
POST /api/oagw/v1/upstreams HTTP/1.1
Host: oagw.example.com
Authorization: Bearer <tenant-token>
Content-Type: application/json

{
  "server": {
    "endpoints": [
      { "scheme": "https", "host": "api.openai.com", "port": 443 }
    ]
  },
  "protocol": "gts.x.core.oagw.protocol.v1~x.core.http.v1",
  "alias": "openai"
}
```

### Flow Steps

1. **API Handler** receives request
   - Validates Bearer token → 401 if invalid
   - Checks `oagw:upstream:create` permission → 403 if denied
   - Routes to Control Plane

2. **Control Plane** processes request
   - Validates JSON schema
   - Checks alias uniqueness within tenant
   - Writes to `oagw_upstream` table (scoped to tenant)
   - Returns `201 Created` with upstream ID

3. **No cache invalidation** (new resource, nothing cached yet)

### Response

```http
HTTP/1.1 201 Created
Content-Type: application/json
Location: /api/oagw/v1/upstreams/gts.x.core.oagw.upstream.v1~7c9e6679...

{
  "id": "gts.x.core.oagw.upstream.v1~7c9e6679-7425-40de-944b-e07fc1f90ae7",
  "tenant_id": "...",
  "alias": "openai",
  "server": { ... },
  "enabled": true,
  "created_at": "2026-02-09T12:00:00Z"
}
```

## Example: Update Upstream

### Request

```http
PUT /api/oagw/v1/upstreams/gts.x.core.oagw.upstream.v1~7c9e6679... HTTP/1.1
Host: oagw.example.com
Authorization: Bearer <tenant-token>
Content-Type: application/json

{
  "server": {
    "endpoints": [
      { "scheme": "https", "host": "api.openai.com", "port": 443 }
    ]
  },
  "protocol": "gts.x.core.oagw.protocol.v1~x.core.http.v1",
  "alias": "openai",
  "rate_limit": {
    "sustained": { "rate": 100, "window": "minute" }
  }
}
```

### Flow Steps

1. **API Handler** validates and routes to DP

2. **Control Plane** processes update
   - Loads existing upstream (tenant-scoped)
   - Validates changes
   - Writes to database
   - **Invalidates caches**:
     - DP L1: `upstream:{tenant_id}:openai`
     - DP L2 (Redis): `upstream:{tenant_id}:openai`
   - Notifies API Handler to notify CP instances

3. **API Handler** notifies Data Plane
   - Sends cache invalidation message to CP instances
   - CP flushes its L1 cache for this upstream

4. **Response** returned

### Cache Invalidation Flow

```
DP writes to DB
  → DP flushes L1 cache
  → DP flushes L2 cache (Redis)
  → DP returns success to API Handler
  → API Handler notifies CP to flush L1 cache
```

### Response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "id": "gts.x.core.oagw.upstream.v1~7c9e6679...",
  "tenant_id": "...",
  "alias": "openai",
  "rate_limit": {
    "sustained": { "rate": 100, "window": "minute" }
  },
  "updated_at": "2026-02-09T12:05:00Z"
}
```

## Example: Delete Upstream

### Request

```http
DELETE /api/oagw/v1/upstreams/gts.x.core.oagw.upstream.v1~7c9e6679... HTTP/1.1
Host: oagw.example.com
Authorization: Bearer <tenant-token>
```

### Flow Steps

1. **API Handler** validates and routes to DP

2. **Control Plane** processes deletion
   - Checks if upstream has dependent routes
   - If `cascade=true`: deletes routes first
   - If `cascade=false` and routes exist: returns `409 Conflict`
   - Deletes upstream from database
   - Invalidates all related cache entries:
     - Upstream config
     - All routes for this upstream

3. **Cache invalidation** propagates to CP

### Response (success)

```http
HTTP/1.1 204 No Content
```

### Response (conflict)

```http
HTTP/1.1 409 Conflict
Content-Type: application/problem+json

{
  "type": "gts.x.core.errors.err.v1~x.oagw.config.upstream_has_routes.v1",
  "title": "Upstream Has Dependent Routes",
  "status": 409,
  "detail": "Cannot delete upstream with active routes. Use cascade=true or delete routes first.",
  "instance": "/api/oagw/v1/upstreams/gts.x.core.oagw.upstream.v1~7c9e6679...",
  "route_count": 3
}
```

## Cache Keys Affected

Management operations invalidate these cache keys:

### Upstream Operations
- DP L1/L2: `upstream:{tenant_id}:{alias}`
- CP L1: Same key (notified by API Handler)

### Route Operations
- DP L1/L2: `route:{upstream_id}:{method}:{path}`
- CP L1: Same key (notified by API Handler)

### Plugin Operations
- DP L1/L2: `plugin:{plugin_id}`
- CP L1: Same key (if plugin was cached)

## Error Handling

Management operations return RFC 9457 Problem Details on error:

- `400 Bad Request`: Invalid JSON schema, validation errors
- `401 Unauthorized`: Missing/invalid Bearer token
- `403 Forbidden`: Insufficient permissions
- `404 Not Found`: Resource does not exist
- `409 Conflict`: Constraint violation (duplicate alias, dependent resources)
- `503 Service Unavailable`: Database or cache service unavailable

All errors include `X-OAGW-Error-Source: gateway` header.

## Related ADRs

- [ADR: Component Architecture](../docs/adr-component-architecture.md)
- [ADR: Request Routing](../docs/adr-request-routing.md)
- [ADR: Control Plane Caching](../docs/adr-data-plane-caching.md)
