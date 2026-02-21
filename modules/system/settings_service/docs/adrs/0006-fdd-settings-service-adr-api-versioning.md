# ADR-0006: API Versioning and Deprecation Policy

**Date**: 2026-02-03

**Status**: Proposed

**ADR ID**: `fdd-settings-service-adr-api-versioning`

## Context and Problem Statement

The Settings Service API needs a versioning strategy that allows evolution while maintaining backward compatibility for existing consumers. We need to define how API versions are managed, how breaking changes are introduced, and how deprecated endpoints are communicated and eventually removed.

## Decision Drivers

* Must maintain backward compatibility across minor versions
* Need clear communication of breaking changes
* Deprecated endpoints should have sufficient migration period
* Version management should be simple for consumers
* OpenAPI documentation must reflect all supported versions
* Must align with Hyperspot platform versioning standards

## Considered Options

* **Option 1**: URL path versioning (e.g., /v1/settings, /v2/settings)
* **Option 2**: Header-based versioning with Accept header
* **Option 3**: Semantic versioning with automatic compatibility detection

## Decision Outcome

Chosen option: "Option 1 - URL path versioning", because it provides the clearest version visibility, simplifies routing and documentation, and aligns with REST API best practices and Hyperspot platform standards.

### Consequences

* Good, because version is explicit and visible in URLs
* Good, because routing and documentation are straightforward
* Good, because consumers can easily target specific versions
* Bad, because requires maintaining multiple endpoint implementations
* Bad, because URL changes when major version increments

## Related Design Elements

**Principles**:

* `fdd-settings-service-principle-api-compatibility` - API stability with semantic versioning

**Requirements**:

* `fdd-settings-service-fr-setting-type-definition` - Setting type definition with versioningts
* `fdd-settings-service-fr-setting-value-crud` - CRUD API endpoints
