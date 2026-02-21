# ADR-0016: Database Schema Design for Tenant Hierarchy

**Date**: 2026-02-03

**Status**: Proposed

**ADR ID**: `fdd-settings-service-adr-database-schema-hierarchy`

## Context and Problem Statement

The Settings Service must efficiently query tenant hierarchies for inheritance resolution, potentially traversing 10+ levels. We need to determine the optimal database schema design for storing and querying tenant hierarchy relationships in setting records to minimize query complexity and latency.

## Decision Drivers

* Inheritance resolution requires traversing tenant hierarchy up to 10+ levels
* Read operations must achieve sub-100ms p95 latency
* Hierarchy queries are frequent (every inherited setting read)
* Database must support PostgreSQL, MariaDB, and SQLite via SeaORM
* Schema must enable efficient cache warming (batch hierarchy retrieval)
* Write operations (setting updates) should not be significantly impacted
* Must support efficient prefix-based queries for hierarchy subtrees
* Tenant hierarchies change infrequently (assumption: hierarchy modifications are rare operations)
* Adjustments in the middle of tenant hierarchy tree are seldom expected

## Considered Options

* **Option 1**: Adjacency List Model (parent_tenant_id foreign key)
* **Option 2**: Materialized Path Model (full hierarchy path string)
* **Option 3**: Nested Set Model (left/right boundary integers)

## Decision Outcome

Chosen option: "Option 2 - Materialized Path Model", because it provides the best balance of query performance, simplicity, and compatibility across database backends for read-heavy workloads with deep hierarchies.

### Materialized Path Implementation

**Schema Additions**:

1. **Setting Values Table**:

```sql
ALTER TABLE setting_values ADD COLUMN tenant_path VARCHAR(1024);
CREATE INDEX idx_setting_values_tenant_path ON setting_values(tenant_path);
```

1. **Tenant Hierarchy Cache Table** (see `fdd-settings-service-db-table-tenant-hierarchy`):

```sql
ALTER TABLE tenant_hierarchy_cache ADD COLUMN materialized_path VARCHAR(1024);
CREATE INDEX idx_tenant_hierarchy_materialized_path ON tenant_hierarchy_cache(materialized_path);
```

The materialized path is stored in both tables:

* `tenant_hierarchy_cache`: For efficient hierarchy lookups during value resolution
* `setting_values`: For optimized setting queries with hierarchy context

**Path Format**: `/dc52a314-6d47-44f7-90e8-48d5219cbc61/259a8820-ef53-40e8-b3a7-a0f78a076c06/2e7c1d4f-ecd0-4fcb-9f08-6e9d9f5d909e/`

* Leading and trailing slashes for consistent prefix matching
* Tenant UUIDs separated by slashes (e.g., root tenant UUID / partner UUID / customer UUID)
* Each segment is a UUID (36 characters)
* Maximum depth: ~27 levels (1024 bytes / 37 chars per segment including slash)

**Query Examples**:

1. **Get all ancestor settings for inheritance**:

```sql
SELECT * FROM setting_values 
WHERE setting_type_id = ? 
  AND tenant_path IN (
    '/dc52a314-6d47-44f7-90e8-48d5219cbc61/',
    '/dc52a314-6d47-44f7-90e8-48d5219cbc61/259a8820-ef53-40e8-b3a7-a0f78a076c06/',
    '/dc52a314-6d47-44f7-90e8-48d5219cbc61/259a8820-ef53-40e8-b3a7-a0f78a076c06/2e7c1d4f-ecd0-4fcb-9f08-6e9d9f5d909e/'
  )
ORDER BY LENGTH(tenant_path) DESC;
```

1. **Get all settings in hierarchy subtree**:

```sql
SELECT * FROM setting_values 
WHERE tenant_path LIKE '/dc52a314-6d47-44f7-90e8-48d5219cbc61/259a8820-ef53-40e8-b3a7-a0f78a076c06/%';
```

1. **Cache warming for tenant and ancestors**:

```sql
SELECT * FROM setting_values 
WHERE tenant_path IN (?, ?, ?, ?, ?)  -- batch query
  AND setting_type_id IN (?);
```

**Path Maintenance**:

* Computed on write from `tenant_hierarchy_cache` table
* Materialized path in cache table updated via event-driven synchronization (ADR-0005)
* Setting values inherit path from cache during write operations
* Updated when tenant hierarchy changes (rare operation)
* Validated against Tenant Management Module hierarchy

### Consequences

* Good, because single query retrieves all ancestor settings without recursion
* Good, because prefix queries enable efficient subtree operations
* Good, because compatible with all target databases (no CTE requirement)
* Good, because indexed path enables fast lookups
* Good, because simplifies cache warming with batch queries
* Good, because path structure is consistent and deterministic
* Bad, because requires path recomputation when tenant hierarchy changes
* Bad, because adds storage overhead (~100-500 bytes per record)
* Bad, because path updates require updating all descendant records (rare operation)
* Bad, because maximum hierarchy depth limited by VARCHAR length

### Comparison with Alternatives

**Adjacency List (Option 1)**:

* Pros: Simple writes, minimal storage
* Cons: Requires recursive CTEs (not supported in SQLite), multiple queries, complex cache warming
* Verdict: Poor performance for read-heavy workload

**Nested Set (Option 3)**:

* Pros: Efficient subtree queries, no recursion needed
* Cons: Complex writes (rebalancing), difficult to understand, fragile under concurrent updates
* Verdict: Too complex for benefit gained

## Related Design Elements

**Principles**:

* `fdd-settings-service-constraint-performance` - Query performance requirements
* `fdd-settings-service-constraint-database-compatibility` - Multi-database support

**Requirements**:

* `fdd-settings-service-fr-tenant-inheritance` - Hierarchy traversal for inheritance
* `fdd-settings-service-fr-setting-value-crud` - Efficient read operations

**Related ADRs**:

* ADR-0002 (Database Technology Selection) - Database compatibility requirements
* ADR-0003 (Caching Strategy Selection) - Cache warming and miss handling
* ADR-0012 (Setting Value Inheritance Algorithm) - Hierarchy traversal algorithm
