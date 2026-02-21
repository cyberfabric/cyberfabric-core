# ADR-0014: Soft Deletion Strategy

**Date**: 2026-02-03

**Status**: Proposed

**ADR ID**: `fdd-settings-service-adr-soft-deletion`

## Context and Problem Statement

The Settings Service supports soft deletion with configurable retention periods, allowing recovery of deleted data within the retention window. We need to determine how to implement soft deletion efficiently while supporting queries that exclude deleted items.

## Decision Drivers

* Must support configurable retention periods (default 90 days)
* Deleted items should not appear in normal queries
* Must support recovery within retention period
* Permanent cleanup after retention expiration
* Must maintain referential integrity during soft deletion
* Query performance should not degrade significantly

## Considered Options

* **Option 1**: Tombstone pattern with deleted_at timestamp column
* **Option 2**: Separate archive table for deleted items
* **Option 3**: Soft delete flag with filtered indexes

## Decision Outcome

Chosen option: "Option 1 - Tombstone pattern with deleted_at timestamp column", because it provides simple implementation, maintains referential integrity, enables easy recovery, and allows efficient filtering with partial indexes on non-deleted rows.

### Consequences

* Good, because simple to implement and understand
* Good, because recovery is straightforward (set deleted_at to NULL)
* Good, because partial indexes optimize queries for non-deleted items
* Bad, because deleted items remain in main table increasing size
* Bad, because cleanup requires periodic job to permanently delete

## Related Design Elements

**Requirements**:

* `fdd-settings-service-fr-soft-deletion` - Soft deletion with retention periods
* `fdd-settings-service-fr-data-deletion` - Permanent deletion for compliance
