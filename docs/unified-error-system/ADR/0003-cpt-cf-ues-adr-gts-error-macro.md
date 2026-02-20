---
status: accepted
date: 2026-02-11
deciders: CyberFabric Core Team
---

# Struct-Based Error Definition with `#[gts_error]` Proc Macro

**ID**: `cpt-cf-ues-adr-gts-error-macro`

## Context and Problem Statement

CyberFabric modules need to define error types with GTS identifiers, HTTP statuses, and metadata fields. The error definition mechanism must make GTS IDs visible in code, enforce correctness at compile time, and generate the boilerplate for converting errors to Problem responses. How should error types be defined and their Problem conversion be generated?

## Decision Drivers

* GTS IDs must be visible and searchable in source code
* Errors with invalid GTS format or missing fields must fail at compile time, not runtime
* Boilerplate for `into_problem()`, `Display`, `Error`, and constants must be auto-generated
* Metadata fields must be type-safe and auditable — no dynamic injection
* Module developers must have a simple, declarative API for defining errors

## Considered Options

* `#[gts_error]` proc macro on explicit structs
* JSON/YAML error catalog with code generation build step
* Rust enum with derive macro
* Runtime builder pattern

## Decision Outcome

Chosen option: "`#[gts_error]` proc macro on explicit structs", because it makes GTS IDs visible as attribute arguments on each struct, provides compile-time validation of all error attributes, generates all boilerplate automatically, and uses struct fields as the single auditable source for metadata content.

### Consequences

* Good, because GTS IDs are visible as string literals on each error struct — searchable via `grep`/`rg`
* Good, because compile-time validation catches format errors, missing attributes, and type mismatches
* Good, because struct fields are the sole source for `metadata` — no runtime injection possible
* Good, because generated `ERROR_DEF` constant enables zero-cost error registration
* Good, because `#[gts_error(skip_metadata)]` and `#[gts_error(as_errors)]` attributes provide fine-grained control over metadata serialization
* Bad, because proc macros add compile time overhead
* Bad, because macro-generated code is harder to debug (expanded code not directly visible)

### Confirmation

* All error types in `modkit-errors` and module crates use `#[gts_error]` attribute
* No error types construct Problem directly — all go through `into_problem()`
* Compile-time tests verify macro rejects invalid GTS format and missing attributes
* Code review checklist includes "no raw Problem construction" rule

## Pros and Cons of the Options

### `#[gts_error]` proc macro on explicit structs

Each error is a dedicated Rust struct with `#[gts_error(type = "...", base = BaseError, status = NNN, title = "...")]`. The macro generates constants, `into_problem()`, `Display`, and `Error` implementations.

* Good, because GTS IDs are visible as attribute arguments — easily searchable
* Good, because compile-time validation of all attributes
* Good, because struct fields are the only source for metadata — auditable and type-safe
* Good, because `ERROR_DEF` constant enables zero-cost registration
* Good, because field attributes (`skip_metadata`, `as_errors`) provide metadata control
* Good, because each error is a distinct type — enables pattern matching in error mapping
* Bad, because proc macro adds compile time cost
* Bad, because expanded code requires `cargo expand` to inspect

### JSON/YAML error catalog with code generation

Error definitions in a JSON or YAML file, with a build script generating Rust types.

* Good, because centralized error catalog in a single file
* Good, because non-Rust tooling can consume the catalog (documentation, SDK generation)
* Bad, because GTS IDs are hidden in a JSON file — not visible in Rust source
* Bad, because build step adds complexity and potential for stale generated code
* Bad, because no compile-time validation of GTS format within Rust
* Bad, because metadata fields must be defined separately from the catalog

### Rust enum with derive macro

A single enum per module with variants for each error type, using a derive macro.

* Good, because exhaustive match checking on the enum
* Good, because single type per module for all errors
* Bad, because GTS IDs must be embedded in variant attributes — less visible than struct-level attributes
* Bad, because enum variants cannot have individually typed fields for metadata
* Bad, because adding a variant is a breaking change for downstream match expressions
* Bad, because no per-error type identity — cannot pass a specific error type to a function

### Runtime builder pattern

Error types constructed at runtime via a builder: `Problem::builder().gts_type("...").status(404).metadata("key", value).build()`.

* Good, because flexible — can construct any error at runtime
* Good, because no macro overhead
* Bad, because GTS IDs are runtime strings — no compile-time validation
* Bad, because metadata can be injected dynamically — no audit trail, risk of sensitive data leakage
* Bad, because no generated constants for registration
* Bad, because verbose — every error construction site must repeat all fields

## More Information

The macro expansion for a typical error struct generates approximately 30 lines of code including constants, `into_problem()`, `Display`, and `Error` implementations. For unit structs (no fields), `metadata` is `None`. For structs with fields, each field is inserted into a `HashMap<String, serde_json::Value>` unless annotated with `#[gts_error(skip_metadata)]`.

## Traceability

- **PRD**: [PRD.md](../PRD.md)
- **DESIGN**: [DESIGN.md](../DESIGN.md)

This decision directly addresses the following requirements or design elements:

* `cpt-cf-ues-fr-compile-time-def` — Establishes proc macro as the compile-time error definition mechanism
* `cpt-cf-ues-fr-metadata-fields` — Metadata comes exclusively from struct fields, not builders
* `cpt-cf-ues-principle-ids-in-code` — GTS IDs are visible as attribute arguments on each struct
* `cpt-cf-ues-principle-metadata-from-fields` — No `.with_metadata()` builder exists
