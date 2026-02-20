---
status: accepted
date: 2026-02-11
deciders: CyberFabric Core Team
---

# Use GTS Type Identifiers for All REST API Errors

**ID**: `cpt-cf-ues-adr-gts-identifiers`

## Context and Problem Statement

CyberFabric is a modular monolith where each module independently returns errors to API consumers. Without a standardized error classification scheme, consumers must parse human-readable messages or maintain per-module error handling logic. We need a machine-readable, hierarchical error identification system that works across all modules while supporting module-specific error types.

## Decision Drivers

* Must provide machine-readable error classification for programmatic handling
* Must support hierarchical error taxonomy (platform vs module errors)
* Must be extensible — new modules can define new error types without changing the core
* Must be consistent with existing CyberFabric type system conventions (GTS)
* Must support versioning of error schemas

## Considered Options

* GTS type URIs with 2-segment chain model
* Plain string error codes (e.g., `NOT_FOUND`, `AUTH_EXPIRED`)
* Numeric error codes (e.g., `40401`, `50001`)
* Enum-based error codes per module

## Decision Outcome

Chosen option: "GTS type URIs with 2-segment chain model", because it reuses the existing GTS type system already used across CyberFabric, provides hierarchical namespacing that prevents cross-module collisions, supports schema versioning natively, and each segment can introduce metadata fields.

### Consequences

* Good, because error types are consistent with the GTS type system used elsewhere in CyberFabric
* Good, because hierarchical namespacing (`cf.system.*`, `cf.types_registry.*`) prevents collisions between modules
* Good, because version suffix (`v1`, `v1.2`) supports non-breaking evolution of error schemas
* Good, because each segment can define additional metadata fields (e.g., `retry_after` for rate limiting)
* Bad, because GTS URIs are longer than simple string codes, increasing response payload size slightly
* Bad, because developers must learn the GTS segment format when defining new errors

### Confirmation

* All error types in `modkit-errors` and module crates use GTS format
* `#[gts_error]` macro validates GTS segment format at compile time
* Types registry validates GTS ID uniqueness at registration

## Pros and Cons of the Options

### GTS type URIs with 2-segment chain model

Full GTS type identifier using `gts://` prefix and `~`-separated segments. Example: `gts://gts.cf.core.errors.err.v1~cf.system.logical.not_found.v1~`.

* Good, because reuses existing CyberFabric GTS conventions
* Good, because hierarchical namespacing prevents collisions
* Good, because native versioning support per segment
* Good, because each segment can introduce metadata fields
* Good, because parseable — consumers can extract vendor, package, namespace, type programmatically
* Bad, because verbose identifiers increase response size
* Bad, because learning curve for GTS segment format

### Plain string error codes

Simple uppercase string codes like `NOT_FOUND`, `AUTH_TOKEN_EXPIRED`.

* Good, because simple and human-readable
* Good, because small payload size
* Bad, because no hierarchical structure — flat namespace
* Bad, because no built-in versioning
* Bad, because collision risk between modules (e.g., two modules define `NOT_FOUND`)
* Bad, because no metadata extension mechanism

### Numeric error codes

Numeric codes with embedded semantics (e.g., `40401` = 404 + error 01).

* Good, because compact payload
* Good, because sortable
* Bad, because not self-describing — requires documentation lookup
* Bad, because numbering schemes become arbitrary and hard to maintain
* Bad, because no hierarchical structure
* Bad, because no versioning support

### Enum-based error codes per module

Each module defines a Rust enum of error variants, serialized as strings.

* Good, because compile-time exhaustiveness checking
* Good, because type-safe within a module
* Bad, because no cross-module consistency — each module invents its own enum
* Bad, because no hierarchical namespacing
* Bad, because API consumers see different enum formats per module
* Bad, because no metadata extension mechanism per error type

## More Information

The 2-segment chain model keeps all errors at exactly two levels: base schema + specific error. While more segments are possible in rare cases, the default is always 2 segments. The `base` field in `#[gts_error]` determines which GTS schema segment is prepended — it does not inherit `status`. Each error defines its own `status` explicitly.

## Traceability

- **PRD**: [PRD.md](../PRD.md)
- **DESIGN**: [DESIGN.md](../DESIGN.md)

This decision directly addresses the following requirements or design elements:

* `cpt-cf-ues-fr-gts-type-id` — Establishes GTS type URIs as the error identification mechanism
* `cpt-cf-ues-fr-machine-readable` — GTS URIs provide machine-readable, parseable error codes
* `cpt-cf-ues-constraint-gts-format` — Defines the canonical GTS segment format for errors
* `cpt-cf-ues-principle-two-segment` — Establishes the 2-segment chain as the default model
