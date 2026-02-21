# ADR-004: Tenant-Scoped Caching
**Date:** 2026-02-02

**Status:** Accepted

**ID:** wsg-adr-004-tenant-scoped-caching

## Context and Problem Statement
Search results should be cached to reduce latency and provider costs. How should cache isolation be handled in a multi-tenant environment?

## Decision Drivers
- Data isolation: Tenant A must never see Tenant B's cached results
- Performance: Maximize cache hit rate within tenant boundary
- Security: Prevent cross-tenant data leakage
- Cost: Reduce redundant provider API calls

## Considered Options
1. Global Cache: Single cache namespace, no tenant isolation
2. Tenant-Scoped Cache: Cache keys prefixed with tenant identifier
3. Separate Cache Instances: Physical cache separation per tenant

## Decision Outcome
Chosen option: "Tenant-Scoped Cache", because it provides strong isolation with efficient resource utilization.

## Implementation
- Cache key structure: `ws:{tenant_id}:{search_type}:{provider_id}:{query_hash}`
- Tenant ID is mandatory in all cache operations
- Cache bypass options respect tenant boundaries
- Admin cache invalidation scoped to tenant

## Cache Key Components:
- `tenant_id`: Ensures tenant isolation
- `search_type`: web, news, images, etc.
- `provider_id`: Different providers may return different results
- `query_hash`: SHA-256 of normalized query + sorted options

## Consequences
- Good, because strong tenant isolation at cache level
- Good, because efficient resource sharing (single cache instance)
- Good, because cache invalidation can be tenant-specific
- Bad, because cache hit rate is lower than global cache (no cross-tenant sharing)
- Acceptable, because cross-tenant result sharing would be a security risk
