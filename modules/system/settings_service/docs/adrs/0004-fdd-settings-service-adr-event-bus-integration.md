# ADR-0004: Event Bus Integration Pattern

**Date**: 2026-02-03

**Status**: Proposed

**ADR ID**: `fdd-settings-service-adr-event-bus-integration`

## Context and Problem Statement

The Settings Service needs to integrate with an event bus for consuming tenant lifecycle events, publishing setting change events, and receiving domain object deletion events. We need to determine which event bus technology to use and how to structure the integration.

## Decision Drivers

* Must consume tenant lifecycle events (created, updated, deleted)
* Must publish setting value change events based on trait configuration
* Must handle domain object deletion events for automatic cleanup
* Need reliable message delivery with at-least-once semantics
* Must support event filtering and routing
* Integration should align with Hyperspot platform standards

## Considered Options

* **Option 1**: Kafka with consumer groups and topic-based routing
* **Option 2**: NATS with JetStream for persistence and replay
* **Option 3**: Use Hyperspot's existing Event Bus Module abstraction

## Decision Outcome

Chosen option: "Option 3 - Use Hyperspot's existing Event Bus Module abstraction", because it provides a platform-standard integration, abstracts the underlying technology, and ensures compatibility with other Hyperspot modules without introducing technology-specific dependencies.

### Consequences

* Good, because aligns with Hyperspot platform standards
* Good, because Event Bus Module handles connection management and retries
* Good, because abstraction allows underlying technology to change
* Bad, because limited to features provided by Event Bus Module abstraction
* Bad, because debugging requires understanding Event Bus Module internals

## Related Design Elements

**Principles**:

* `fdd-settings-service-principle-hyperspot-integration` - Hyperspot module integration

**Requirements**:

* `fdd-settings-service-fr-event-generation` - Event generation for setting changes
* `fdd-settings-service-fr-tenant-reconciliation` - Tenant event consumption
* `fdd-settings-service-fr-dynamic-domain-types` - Domain object deletion events
