# ADR-0001: GTS Schema Versioning Strategy

**Date**: 2026-02-03

**Status**: Proposed

**ADR ID**: `fdd-settings-service-adr-gts-schema-versioning`

## Context and Problem Statement

The Settings Service uses GTS (Global Type System) for all setting type schema definitions. We need to determine how to handle schema versioning, evolution, and backward compatibility as setting types evolve over time. The system must support multiple schema versions simultaneously while ensuring existing consumers continue to function without breaking changes.

## Decision Drivers

* Need to support schema evolution without breaking existing consumers
* Must handle multiple schema versions simultaneously in production
* Type resolution must be deterministic and predictable
* Schema validation must use the correct version for each setting type
* Must coordinate with GTS Type Registry for schema management
* Need clear migration path for consumers when schemas evolve

## Considered Options

* **Option 1**: Semantic versioning with GTS Type Registry (v1.0, v1.1, v2.0)
* **Option 2**: Timestamp-based versioning with automatic resolution
* **Option 3**: Single version with in-place schema updates

## Decision Outcome

Chosen option: "Option 1 - Semantic versioning with GTS Type Registry", because it provides clear compatibility guarantees, enables gradual migration, and aligns with GTS specification requirements for type versioning.

### Consequences

* Good, because semantic versioning provides clear compatibility signals (major.minor)
* Good, because GTS Type Registry handles version resolution automatically
* Good, because consumers can specify version constraints and get compatible schemas
* Bad, because requires coordination with GTS team for schema registration
* Bad, because multiple versions increase storage and validation complexity

## Related Design Elements

**Principles**:

* `fdd-settings-service-principle-api-compatibility` - API stability with semantic versioning
* `fdd-settings-service-constraint-gts-transition` - GTS schema versioning requirement

**Requirements**:

* `fdd-settings-service-fr-gts-versioning` - GTS-based type versioning support
* `fdd-settings-service-fr-gts-base-setting-type` - Base GTS type definition
* `fdd-settings-service-fr-gts-extension-point` - Extension point registration
