# ADR-0011: Domain Type Registry Storage

**Date**: 2026-02-03

**Status**: Proposed

**ADR ID**: `fdd-settings-service-adr-domain-type-registry`

## Context and Problem Statement

The Settings Service maintains a domain type registry that stores metadata for each supported domain type (TENANT, USER). We need to determine where to store this registry data and how to manage dynamic domain type registration.

## Decision Drivers

* Must support predefined domain types (TENANT, USER)
* Must allow runtime registration of custom domain types
* Registry data includes: domain_id type, validation endpoint, deletion event type
* Must be accessible for validation and event subscription
* Should support versioning for domain type evolution
* Must coordinate with GTS for type definitions

## Considered Options

* **Option 1**: Database storage with migration-based seeding
* **Option 2**: GTS Registry as single source of truth
* **Option 3**: Hybrid with database cache and GTS backing

## Decision Outcome

Chosen option: "Option 1 - Database storage with migration-based seeding", because it provides fast local access, simplifies validation logic, and avoids dependency on external GTS Registry for runtime operations while still allowing GTS integration for type definitions.

### Consequences

* Good, because local database access is fast and reliable
* Good, because migrations ensure predefined types are always present
* Good, because no external dependency for runtime validation
* Bad, because domain type definitions are not shared across services
* Bad, because requires database migration for new predefined types

## Related Design Elements

**Requirements**:

* `fdd-settings-service-fr-dynamic-domain-types` - Dynamic domain type registration
* `fdd-settings-service-fr-domain-objects` - Domain object association
