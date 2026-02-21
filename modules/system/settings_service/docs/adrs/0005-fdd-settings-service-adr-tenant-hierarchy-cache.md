# ADR-0005: Tenant Hierarchy Cache Synchronization

**Date**: 2026-02-03

**Status**: Proposed

**ADR ID**: `fdd-settings-service-adr-tenant-hierarchy-cache`

## Context and Problem Statement

⚠️ **IMPLEMENTATION NOTE**: This ADR is deferred for the initial implementation. The initial version will query the Tenant Management Module database directly for tenant hierarchy data. Event-driven cache synchronization will be implemented in a future phase as a performance optimization.

The Settings Service needs to maintain a local cache of tenant hierarchy data for efficient inheritance resolution. We need to determine how to synchronize this cache with the Tenant Management Module and handle hierarchy changes in real-time.

## Decision Drivers

* Inheritance resolution requires traversing tenant hierarchy (10+ levels)
* Cache must be consistent across all service instances
* Tenant hierarchy changes must be reflected quickly
* Need to handle tenant creation, updates, and deletion
* Must support barrier tenant logic for inheritance
* Cache warming on startup must be efficient

## Considered Options

* **Option 1**: Event-driven synchronization with full hierarchy rebuild
* **Option 2**: Incremental updates via tenant lifecycle events
* **Option 3**: Periodic polling with change detection

## Decision Outcome

Chosen option: "Option 2 - Incremental updates via tenant lifecycle events", because it provides real-time consistency, minimizes data transfer, and leverages the existing event bus infrastructure for reliable delivery.

### Consequences

* Good, because real-time updates ensure cache consistency
* Good, because incremental updates minimize network and processing overhead
* Good, because event bus provides reliable delivery guarantees
* Good, because cache miss handling with DB fallback ensures eventual consistency
* Bad, because requires careful handling of event ordering
* Bad, because cache warming on startup requires full hierarchy fetch
* Bad, because cache misses require synchronous DB lookup and cache refresh

### Cache Miss Handling

When a tenant is not found in the cache during value resolution:

1. **Check Database**: Query the Tenant Management Module database directly for tenant existence
2. **Cache Refresh**: If tenant exists in DB but not in cache (stale cache), trigger event-driven cache synchronization
3. **Return Value**: After cache refresh, proceed with value resolution and return the requested setting value
4. **Metrics**: Track cache miss rate and refresh latency for monitoring

This ensures eventual consistency while maintaining query performance for the common case (cache hit).

## Related Design Elements

**Principles**:

* `fdd-settings-service-principle-default-fallback` - Value resolution requiring hierarchy with cache miss handling

**Requirements**:

* `fdd-settings-service-fr-tenant-inheritance` - Tenant hierarchy inheritance
* `fdd-settings-service-fr-tenant-reconciliation` - Tenant synchronization
* `fdd-settings-service-fr-access-control` - Hierarchy validation for access control

**Related ADRs**:

* ADR-0016 (Database Schema Design for Tenant Hierarchy) - Materialized path in cache table
