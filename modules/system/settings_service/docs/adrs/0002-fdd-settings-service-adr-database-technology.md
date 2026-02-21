# ADR-0002: Database Technology Selection

**Date**: 2026-02-03

**Status**: Proposed

**ADR ID**: `fdd-settings-service-adr-database-technology`

## Context and Problem Statement

The Settings Service requires a database backend that supports PostgreSQL, MariaDB, and SQLite through SeaORM abstraction. We need to determine the primary database technology for production deployments while maintaining compatibility with the existing Go implementation schema to enable gradual migration.

## Decision Drivers

* Must support PostgreSQL, MariaDB, and SQLite through SeaORM
* Schema must be compatible with existing Go implementation
* Migrations must be bidirectional for rollback support
* Must maintain referential integrity
* Need appropriate indexes for tenant hierarchy traversal
* Performance requirements for 10,000+ write operations per minute
* Multi-tenancy isolation requirements

## Considered Options

* **Option 1**: PostgreSQL as primary with MariaDB/SQLite support
* **Option 2**: MariaDB as primary with PostgreSQL/SQLite support
* **Option 3**: SQLite for development, PostgreSQL for production

## Decision Outcome

Chosen option: "Option 1 - PostgreSQL as primary with MariaDB/SQLite support", because PostgreSQL provides the best balance of features, performance, and JSON support for setting data storage, while SeaORM abstraction enables MariaDB and SQLite compatibility.

### Consequences

* Good, because PostgreSQL JSONB provides efficient storage and querying for setting data
* Good, because PostgreSQL has excellent multi-tenancy support with row-level security
* Good, because SeaORM abstraction maintains compatibility with other databases
* Bad, because some PostgreSQL-specific features cannot be used
* Bad, because requires testing across all three database backends

## Related Design Elements

**Principles**:

* `fdd-settings-service-constraint-database-compatibility` - Multi-database support requirement

**Requirements**:

* `fdd-settings-service-fr-setting-value-crud` - CRUD operations with database persistence
* `fdd-settings-service-fr-tenant-inheritance` - Tenant hierarchy queries requiring indexes
