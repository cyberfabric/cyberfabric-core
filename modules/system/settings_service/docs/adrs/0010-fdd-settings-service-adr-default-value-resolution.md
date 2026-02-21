# ADR-0010: Default Value Resolution Strategy

**Date**: 2026-02-03

**Status**: Proposed

**ADR ID**: `fdd-settings-service-adr-default-value-resolution`

## Context and Problem Statement

The Settings Service follows an "always return a value" philosophy where read operations never fail due to missing values. We need to determine how default values are defined, stored, and resolved in the cascading fallback chain.

## Decision Drivers

* Every setting type must have a default value
* Default values must be validated against GTS schema
* Resolution chain: explicit → tenant/generic → inherited → default
* Default values should be immutable once type is created
* Must support complex default values (objects, arrays)
* Performance impact should be minimal

## Considered Options

* **Option 1**: Schema-based defaults stored in GTS schema definition
* **Option 2**: Configuration-based defaults in service configuration
* **Option 3**: Database-stored defaults with version tracking

## Decision Outcome

Chosen option: "Option 1 - Schema-based defaults stored in GTS schema definition", because it ensures defaults are validated, versioned with the schema, and accessible without additional database queries or configuration lookups.

### Consequences

* Good, because defaults are validated against schema at type creation
* Good, because defaults are versioned with schema evolution
* Good, because no additional storage or configuration required
* Bad, because changing defaults requires new schema version
* Bad, because defaults are immutable for a given schema version

## Related Design Elements

**Principles**:

* `fdd-settings-service-principle-default-fallback` - Always return a value

**Requirements**:

* `fdd-settings-service-fr-tenant-inheritance` - Value resolution with defaults
* `fdd-settings-service-fr-gts-base-setting-type` - Default value in schema
