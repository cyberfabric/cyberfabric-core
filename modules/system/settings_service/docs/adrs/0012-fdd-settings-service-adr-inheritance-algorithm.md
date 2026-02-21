# ADR-0012: Setting Value Inheritance Algorithm

**Date**: 2026-02-03

**Status**: Proposed

**ADR ID**: `fdd-settings-service-adr-inheritance-algorithm`

## Context and Problem Statement

The Settings Service resolves setting values through tenant hierarchy inheritance chains (10+ levels). We need to determine the algorithm for traversing the hierarchy and resolving inherited values efficiently.

## Decision Drivers

* Must traverse tenant hierarchy up to 10+ levels
* Resolution order: explicit → tenant/generic → inherited → default
* Must respect barrier tenant logic
* Must handle inheritance enabled/disabled per setting type
* Performance critical for read operations (sub-100ms p95)
* Must return value source metadata for debugging

## Considered Options

* **Option 1**: Recursive algorithm with stack-based traversal
* **Option 2**: Iterative algorithm with cached hierarchy paths
* **Option 3**: Database-level recursive CTE queries

## Decision Outcome

Chosen option: "Option 2 - Iterative algorithm with cached hierarchy paths", because it provides predictable performance, avoids stack overflow risks, leverages cached tenant hierarchy data, and enables early termination when value is found.

### Consequences

* Good, because iterative approach has predictable memory usage
* Good, because cached paths eliminate repeated database queries
* Good, because early termination optimizes common cases
* Bad, because requires maintaining tenant hierarchy cache
* Bad, because cache invalidation adds complexity

## Related Design Elements

**Principles**:

* `fdd-settings-service-principle-default-fallback` - Value resolution chain

**Requirements**:

* `fdd-settings-service-fr-tenant-inheritance` - Inheritance resolution
* `fdd-settings-service-fr-setting-value-crud` - Value retrieval with inheritance
