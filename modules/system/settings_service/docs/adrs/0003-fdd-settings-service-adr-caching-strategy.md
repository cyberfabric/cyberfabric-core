# ADR-0003: Caching Strategy Selection

**Date**: 2026-02-03

**Status**: Proposed

**ADR ID**: `fdd-settings-service-adr-caching-strategy`

## Context and Problem Statement

⚠️ **IMPLEMENTATION NOTE**: This ADR is deferred for the initial implementation. The initial version will use direct database queries for simplicity. Caching optimization will be implemented in a future phase when performance requirements necessitate sub-100ms p95 latency.

The Settings Service needs to achieve sub-100ms response time for 95th percentile of read operations. We need to determine the caching strategy for frequently accessed setting values, tenant hierarchy data, and GTS schema definitions to meet performance requirements. The caching strategy must be configurable to prevent out-of-memory (OOM) issues in production environments with deep tenant hierarchies, and must support efficient cache invalidation with sliding window mechanisms.

## Decision Drivers

* Must achieve sub-100ms response time for 95th percentile reads
* Need to cache tenant hierarchy for inheritance resolution
* GTS schema definitions should be cached to avoid registry lookups
* Cache invalidation must be reliable across distributed instances
* Multi-tenancy requires cache isolation
* Memory usage must be bounded and configurable to prevent OOM
* Tenant hierarchies can be 10+ levels deep, requiring configurable cache depth limits
* Cache invalidation should use sliding window to balance freshness and performance
* All caching parameters must be configurable at module bootstrap
* Database schema should support efficient hierarchy traversal (materialized path model)

## Considered Options

* **Option 1**: Redis for distributed caching with pub/sub invalidation
* **Option 2**: In-memory caching per instance with event-based invalidation and configurable limits
* **Option 3**: Hybrid approach with in-memory L1 and Redis L2 cache

## Decision Outcome

Chosen option: "Option 2 - In-memory caching per instance with event-based invalidation and configurable limits", because it provides the lowest latency for read operations, simplifies deployment, and leverages the existing event bus for cache invalidation without introducing Redis as a dependency.

### Configuration Parameters

The following parameters MUST be configurable at Settings Service module bootstrap:

1. **`cache_tenant_hierarchy_depth`** (integer, default: 5)
   * Maximum number of tenant hierarchy levels to cache per tenant
   * Prevents OOM by limiting cached hierarchy depth
   * Example: depth=5 caches tenant + 4 ancestors, ignoring deeper ancestors
   * Rationale: Most inheritance resolution occurs within 5 levels; deeper hierarchies are rare

2. **`cache_sliding_window_size`** (duration, default: 300 seconds)
   * Time window for sliding window cache invalidation
   * Cached items are invalidated if not accessed within this window
   * Balances cache freshness with hit rate
   * Example: 300s means items unused for 5 minutes are evicted

3. **`cache_max_entries`** (integer, default: 10000)
   * Maximum number of cached setting values per instance
   * Hard limit to prevent unbounded memory growth
   * LRU eviction when limit reached

4. **`cache_gts_schema_ttl`** (duration, default: 3600 seconds)
   * Time-to-live for cached GTS schema definitions
   * Longer TTL acceptable as schemas change infrequently

5. **`cache_enabled`** (boolean, default: true)
   * Master switch to disable all caching (for debugging/testing)

### Sliding Window Invalidation

Cache invalidation uses a sliding window mechanism:

* Each cached item tracks last access timestamp
* Background task runs every `cache_sliding_window_size / 2` seconds
* Items with `last_access < now - cache_sliding_window_size` are evicted
* Event-based invalidation (tenant updates, setting changes) triggers immediate eviction
* Combines time-based and event-based invalidation for optimal freshness

### Consequences

* Good, because in-memory cache provides sub-millisecond access times
* Good, because no additional infrastructure dependency (Redis)
* Good, because event bus already exists for invalidation messages
* Good, because configurable limits prevent OOM in production
* Good, because sliding window balances freshness and performance
* Good, because database schema optimizations (see ADR-0016) reduce cache miss penalty
* Bad, because cache warming required on instance startup
* Bad, because configuration tuning required per deployment environment
* Bad, because sliding window background task adds CPU overhead
* Bad, because deep hierarchies beyond configured depth require database queries

## Related Design Elements

**Principles**:

* `fdd-settings-service-constraint-performance` - Performance requirements

**Requirements**:

* `fdd-settings-service-fr-setting-value-crud` - Read operations requiring caching
* `fdd-settings-service-fr-tenant-inheritance` - Tenant hierarchy caching for resolution

**Related ADRs**:

* ADR-0016 (Database Schema Design for Tenant Hierarchy) - Materialized path model implementation
