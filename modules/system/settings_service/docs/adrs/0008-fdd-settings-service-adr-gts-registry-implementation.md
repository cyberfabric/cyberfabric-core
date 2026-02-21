# ADR-0008: GTS Type Registry Implementation

**Date**: 2026-02-03

**Status**: Proposed

**ADR ID**: `fdd-settings-service-adr-gts-registry-implementation`

## Context and Problem Statement

The Settings Service needs to integrate with a GTS Type Registry for schema storage, version resolution, and validation. We need to determine whether to use an embedded registry within the service or connect to an external GTS Registry service.

## Decision Drivers

* Need schema storage and version resolution
* Must support schema validation at runtime
* Type registration should be reliable and consistent
* Schema queries must be performant
* Must coordinate with other services using GTS
* Deployment complexity should be minimized

## Considered Options

* **Option 1**: Embedded GTS Registry within Settings Service
* **Option 2**: External GTS Registry Service with HTTP/gRPC API
* **Option 3**: Hybrid with local cache and external registry

## Decision Outcome

Chosen option: "Option 2 - External GTS Registry Service", because it enables schema sharing across services, provides centralized schema management, and aligns with GTS specification recommendations for multi-service deployments.

### Consequences

* Good, because schemas are shared across all services
* Good, because centralized management simplifies schema governance
* Good, because external registry can be scaled independently
* Bad, because introduces network dependency for schema operations
* Bad, because requires additional service deployment and management

## Related Design Elements

**Principles**:

* `fdd-settings-service-constraint-gts-transition` - GTS schema versioning requirement

**Requirements**:

* `fdd-settings-service-fr-gts-versioning` - GTS-based type versioning support
* `fdd-settings-service-fr-gts-base-setting-type` - Base type registration
* `fdd-settings-service-fr-gts-extension-point` - Derived type registration
