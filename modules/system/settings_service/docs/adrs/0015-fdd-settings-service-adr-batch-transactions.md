# ADR-0015: Batch Operation Transaction Boundaries

**Date**: 2026-02-03

**Status**: Proposed

**ADR ID**: `fdd-settings-service-adr-batch-transactions`

## Context and Problem Statement

The Settings Service supports batch update operations for setting values across multiple tenants. We need to determine transaction boundaries for batch operations to balance consistency, performance, and partial success handling.

## Decision Drivers

* Batch operations can affect up to 100 tenants
* Must support partial success with detailed error reporting
* Failed operations should not block successful ones
* Transaction overhead should be minimized
* Must maintain data consistency per tenant
* Audit events should reflect actual committed changes

## Considered Options

* **Option 1**: Per-tenant transactions with independent commits
* **Option 2**: All-or-nothing transaction for entire batch
* **Option 3**: Configurable transaction boundaries per request

## Decision Outcome

Chosen option: "Option 1 - Per-tenant transactions with independent commits", because it enables partial success handling, prevents one tenant's failure from blocking others, and aligns with multi-tenancy isolation principles while maintaining consistency within each tenant scope.

### Consequences

* Good, because partial success enables progress despite individual failures
* Good, because tenant isolation is maintained at transaction level
* Good, because failed tenants can be retried independently
* Bad, because batch operation is not atomic across all tenants
* Bad, because audit events are generated even if some operations fail

## Related Design Elements

**Requirements**:

* `fdd-settings-service-fr-batch-operations` - Batch update operations with partial success
* `fdd-settings-service-fr-event-generation` - Audit events for successful updates
