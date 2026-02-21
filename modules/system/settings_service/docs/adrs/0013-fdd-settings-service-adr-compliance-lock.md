# ADR-0013: Compliance Lock Implementation

**Date**: 2026-02-03

**Status**: Proposed

**ADR ID**: `fdd-settings-service-adr-compliance-lock`

## Context and Problem Statement

The Settings Service supports compliance mode where settings can be locked to read-only state for specific tenants and domain objects. We need to determine how to implement these locks efficiently while ensuring they cannot be bypassed.

## Decision Drivers

* Locks must prevent all modifications until explicitly unlocked
* Must support tenant + domain object scope
* Lock checks must be performant (no significant latency)
* Locks must be auditable with creation/removal events
* Must prevent bypass through batch operations or API variations
* Lock state must be consistent across service instances

## Considered Options

* **Option 1**: Row-level locks in database with check constraints
* **Option 2**: Application-level locks with domain layer enforcement
* **Option 3**: Separate locks table with foreign key constraints

## Decision Outcome

Chosen option: "Option 3 - Separate locks table with foreign key constraints", because it provides clear separation of concerns, enables efficient lock queries, supports audit trail, and allows lock management without modifying setting values table.

### Consequences

* Good, because locks table is optimized for lock queries
* Good, because foreign keys ensure referential integrity
* Good, because lock history can be maintained separately
* Bad, because requires join for every write operation
* Bad, because adds another table to maintain and migrate

## Related Design Elements

**Requirements**:

* `fdd-settings-service-fr-compliance-mode` - Compliance lock support
* `fdd-settings-service-fr-event-generation` - Lock audit events
