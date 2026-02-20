# PRD — Unified Error System

## 1. Overview

### 1.1 Purpose

The Unified Error System provides a standardized, machine-readable error handling framework for all CyberFabric REST API responses. Every API error carries a GTS type identifier, an optional trace ID for debugging, and is registered in the types registry — ensuring consistency, traceability, and security across all modules.

### 1.2 Background / Problem Statement

CyberFabric is a modular monolith where each module independently defines and returns errors to API consumers. Without a unified approach, error responses vary in structure, field naming, and detail level across modules. This inconsistency forces API consumers to implement per-module error parsing logic and makes cross-service debugging difficult.

Additionally, ad-hoc error responses risk leaking sensitive information — SQL errors, stack traces, internal hostnames — through unstructured detail fields. A standardized error system eliminates these risks by enforcing a fixed response schema with sanitized metadata and server-side-only logging of sensitive details.

The system adopts RFC 9457 (Problem Details for HTTP APIs) as the wire format and GTS (Global Type System) identifiers as the error classification scheme, providing both industry-standard compliance and CyberFabric-specific error taxonomy.

### 1.3 Goals (Business Outcomes)

- 100% of REST API error responses conform to the unified Problem schema (baseline: 0% — no unified format exists; target: 100% within first release integrating modkit-errors)
- All error types registered in the types registry at module startup (baseline: no registration mechanism; target: every module registers on first release adopting the macro)
- API consumers can programmatically handle errors using the `type` field without per-module logic (baseline: consumers implement per-module parsing; target: single parsing path within first release)

### 1.4 Glossary

| Term | Definition |
|------|------------|
| GTS | Global Type System — a hierarchical type identification scheme using dot-separated segments |
| Problem | The standardized error response struct conforming to RFC 9457 |
| GTS type chain | A multi-segment GTS identifier where each segment can introduce additional fields (e.g., `gts.cf.core.errors.err.v1~cf.system.logical.not_found.v1~`) |
| Base error schema | The root GTS segment (`gts.cf.core.errors.err.v1~`) that anchors all error type chains |
| Error registration | The process of declaring error types in the types registry at module startup |
| Trace ID | A 32-character hex string (W3C trace-id portion) used for request correlation across services |

## 2. Actors

### 2.1 Human Actors

#### Module Developer

**ID**: `cpt-cf-ues-actor-module-dev`

**Role**: Rust developer who defines error types within CyberFabric modules using the `#[gts_error]` macro and maps domain errors to Problem responses.
**Needs**: Compile-time validation of error definitions, simple conversion from domain errors to API responses, clear rules for GTS ID format and metadata fields.

### 2.2 System Actors

#### REST API Consumer

**ID**: `cpt-cf-ues-actor-api-consumer`

**Role**: External or internal client application that receives error responses from CyberFabric REST APIs and handles them programmatically.

#### Types Registry

**ID**: `cpt-cf-ues-actor-types-registry`

**Role**: Internal CyberFabric module that stores and serves all registered GTS error type definitions, enabling runtime discovery of error schemas.

#### Observability Platform

**ID**: `cpt-cf-ues-actor-observability`

**Role**: External monitoring and tracing system (e.g., Jaeger, Grafana) that correlates error responses with server-side traces using trace IDs.

## 3. Operational Concept & Environment

> **Note**: Project-wide runtime, OS, architecture, lifecycle policy, and module integration patterns are defined in the root PRD and guidelines. No module-specific environment constraints apply to the Unified Error System beyond CyberFabric defaults.

## 4. Scope

### 4.1 In Scope

- Standardized error response schema for all REST API endpoints (RFC 9457 compliant)
- GTS type identifier format and 2-segment chain model for error classification
- Compile-time error definition mechanism via proc macro
- Automatic trace ID population from OpenTelemetry span context
- Error type registration in the types registry at module startup
- HTTP response headers for error responses (`X-Trace-Id`, `X-Error-Code`, `Content-Type`, `Retry-After`)
- Platform-level (system) error catalog: transport, runtime, HTTP, gRPC, logical
- Module-level error definition conventions

### 4.2 Out of Scope

- Changing internal domain error types (only API-facing errors are standardized)
- Modifying logging infrastructure
- Changing HTTP status code semantics
- gRPC-native error handling (gRPC errors are mapped to HTTP equivalents for the unified system)
- Background task / non-HTTP error formatting (background errors are logged server-side using standard tracing; the Problem schema applies only to HTTP API responses)

## 5. Functional Requirements

### 5.1 Error Identification

#### GTS Type Identifier

- [ ] `p1` - **ID**: `cpt-cf-ues-fr-gts-type-id`

Every REST API error response **MUST** carry a valid GTS type identifier in the `type` field, formatted as a `gts://` URI with a 2-segment chain (base schema + specific error).

**Rationale**: Machine-readable error classification enables programmatic handling by API consumers without string matching on human-readable messages.
**Actors**: `cpt-cf-ues-actor-module-dev`, `cpt-cf-ues-actor-api-consumer`

#### Trace ID Correlation

- [ ] `p1` - **ID**: `cpt-cf-ues-fr-trace-id`

Every error response **MUST** support an optional trace ID (32 hex chars, W3C trace-id portion only) for request correlation and debugging. When no trace context is available, the field **MUST** be `None` (omitted from JSON) — empty string (`""`) **MUST NOT** be emitted.

**Rationale**: Enables API consumers and support teams to correlate error responses with server-side observability data. Empty strings create ambiguity for consumers checking `if trace_id`.
**Actors**: `cpt-cf-ues-actor-api-consumer`, `cpt-cf-ues-actor-observability`

#### Error Registration

- [ ] `p1` - **ID**: `cpt-cf-ues-fr-error-registration`

All error types **MUST** be registered in the types registry at module startup, providing GTS ID, HTTP status, and title for each error. Registration **MUST** be best-effort: if the types registry is unavailable, the module **MUST** still start and serve REST normally, logging a warning and optionally retrying in the background.

**Rationale**: Runtime discovery of error schemas enables tooling, documentation generation, and validation of error contracts. However, types-registry availability must not affect core module functionality.
**Actors**: `cpt-cf-ues-actor-module-dev`, `cpt-cf-ues-actor-types-registry`

### 5.2 Error Response Schema

#### RFC 9457 Compliance

- [ ] `p1` - **ID**: `cpt-cf-ues-fr-rfc9457-schema`

All error responses **MUST** conform to the RFC 9457 Problem Details schema with fields: `type` (GTS URI), `title` (static string), `status` (HTTP status code), optional `trace_id`, and optional `metadata` (extension members).

**Rationale**: Industry-standard error format reduces integration friction and aligns with HTTP API best practices.
**Actors**: `cpt-cf-ues-actor-api-consumer`

#### Machine-Readable Error Codes

- [ ] `p1` - **ID**: `cpt-cf-ues-fr-machine-readable`

Error responses **MUST** provide machine-readable error codes via the GTS `type` field, enabling programmatic error handling without parsing human-readable text.

**Rationale**: API consumers need deterministic error handling logic that does not break when error messages are rephrased.
**Actors**: `cpt-cf-ues-actor-api-consumer`

### 5.3 Error Definition

#### Compile-Time Error Definition

- [ ] `p1` - **ID**: `cpt-cf-ues-fr-compile-time-def`

The system **MUST** provide a compile-time mechanism for defining error types that makes GTS IDs visible in source code and generates Problem conversion logic automatically.

**Rationale**: Compile-time validation catches GTS ID format errors, missing fields, and type mismatches before deployment.
**Actors**: `cpt-cf-ues-actor-module-dev`

#### Ergonomic Identifier API

- [ ] `p2` - **ID**: `cpt-cf-ues-fr-ergonomic-api`

The system **MUST** generate short, ergonomic accessor constants (e.g., `Errors::TYPES_REGISTRY_NOT_FOUND`) from error definitions so that callers do not need to construct full GTS strings at call sites. Developer experience at the call site **SHOULD** be comparable to the existing `declare_errors!` macro.

**Rationale**: Long GTS URIs are not practical for use in match arms and error construction; short identifiers improve readability and reduce errors.
**Actors**: `cpt-cf-ues-actor-module-dev`

#### Metadata From Struct Fields

- [ ] `p2` - **ID**: `cpt-cf-ues-fr-metadata-fields`

Error metadata **MUST** be populated exclusively through struct fields declared on the error type — no runtime builder or `.with_metadata()` API.

**Rationale**: Struct fields provide a single, auditable source for what data appears in API responses, preventing accidental sensitive data leakage through dynamic metadata injection.
**Actors**: `cpt-cf-ues-actor-module-dev`

### 5.4 System Error Catalog

#### Platform Error Catalog

- [ ] `p1` - **ID**: `cpt-cf-ues-fr-system-errors`

The system **MUST** provide a pre-defined catalog of platform-level errors covering transport, runtime, HTTP, gRPC, and logical error categories with standardized GTS types, HTTP statuses, and titles.

**Rationale**: Common platform errors (not found, unauthorized, timeout, etc.) should be reusable across all modules without redefinition.
**Actors**: `cpt-cf-ues-actor-module-dev`

## 6. Non-Functional Requirements

### 6.1 Module-Specific NFRs

#### No Sensitive Data in Responses

- [ ] `p1` - **ID**: `cpt-cf-ues-nfr-no-sensitive-data`

Error responses **MUST** never expose error chains, full W3C traceparent (only trace-id), credentials, tokens, PII, SQL errors, stack traces, or internal hostnames in any field.

**Threshold**: Zero occurrences of sensitive data in error responses across all modules.
**Rationale**: Sensitive data in error responses is a security vulnerability (CWE-209: Generation of Error Message Containing Sensitive Information).
**Architecture Allocation**: See DESIGN.md § Principles & Constraints for how this is realized.

#### Sanitized Metadata

- [ ] `p1` - **ID**: `cpt-cf-ues-nfr-sanitized-metadata`

User input included in `metadata` **MUST** be sanitized before inclusion in error responses.

**Threshold**: All metadata values pass input sanitization checks.
**Rationale**: Prevents XSS, injection, and information disclosure through error metadata.
**Architecture Allocation**: See DESIGN.md § Principles & Constraints.

#### Server-Side Logging

- [ ] `p1` - **ID**: `cpt-cf-ues-nfr-server-side-logging`

Full error details (error chains, stack traces, internal state) **MUST** be logged server-side with `trace_id` for correlation, returning only the sanitized Problem response to clients.

**Threshold**: Every error with a trace_id has corresponding server-side log entries with full details.
**Rationale**: Enables debugging without exposing internals to API consumers.
**Architecture Allocation**: See DESIGN.md § Sequences.

### 6.2 NFR Exclusions

- Performance: No module-specific performance requirements beyond project defaults. Error conversion is a synchronous in-memory operation with negligible latency.
- Authentication / Authorization: Not applicable — this library formats error responses; authentication and authorization are handled by `modkit-auth` at the platform level.
- Audit: Not applicable — audit logging is a platform-level concern. This library includes `trace_id` for correlation but does not implement audit infrastructure.
- Privacy by Design: Not applicable — this library does not process, store, or transmit personal data. It formats error responses that explicitly exclude PII (see `cpt-cf-ues-nfr-no-sensitive-data`).
- Safety: Not applicable — pure information system with no physical interaction, no medical or industrial control, and no potential for harm to people, property, or environment.
- Availability / Recovery: Not applicable — stateless library distributed as a Cargo crate dependency. No runtime availability, failover, or disaster recovery concerns.
- Usability: Not applicable — no user-facing UI. Developer experience is addressed via the ergonomic API requirement (`cpt-cf-ues-fr-ergonomic-api`).
- Maintainability / Support: Not applicable at PRD level — standard Rust crate conventions apply. Documentation and support follow CyberFabric platform defaults.
- Compliance: Not applicable — internal development library with no direct regulatory, legal, or certification requirements beyond CyberFabric platform-level compliance.
- Data Ownership / Quality / Lifecycle: Not applicable — stateless library with no data storage, processing, or retention.
- Operations / Deployment / Monitoring: Not applicable — distributed as a Cargo crate dependency with no deployment topology or monitoring infrastructure of its own.

### 6.3 Compatibility Requirements

- Co-existence: During migration, modules using the previous error format (`declare_errors!`) and modules using the unified `#[gts_error]` system **MUST** be able to coexist in the same CyberFabric deployment. API consumers may receive both old-format and new-format error responses until migration is complete.
- Backward compatibility: New metadata fields **MAY** be added to error responses without breaking existing API consumers. Field removal or semantic changes to the `type` URI **MUST** require a major version bump.
- Migration: Adoption is module-by-module. No big-bang migration required.

## 7. Public Library Interfaces

### 7.1 Public API Surface

#### Problem Struct

- [ ] `p1` - **ID**: `cpt-cf-ues-interface-problem`

**Type**: Rust struct (public)
**Stability**: stable
**Description**: The standardized error response type with fields `type`, `title`, `status`, `trace_id`, and `metadata`. Serializes to RFC 9457 compliant JSON.
**Breaking Change Policy**: Major version bump required for field additions/removals.

#### `#[gts_error]` Attribute Macro

- [ ] `p1` - **ID**: `cpt-cf-ues-interface-gts-error-macro`

**Type**: Rust proc macro attribute
**Stability**: stable
**Description**: Compile-time error definition macro that generates GTS constants, `into_problem()` conversion, and `Display`/`Error` trait implementations from annotated structs.
**Breaking Change Policy**: Major version bump required for attribute syntax changes.

#### `trace_id_from_current_span()`

- [ ] `p2` - **ID**: `cpt-cf-ues-interface-trace-id-fn`

**Type**: Rust public function
**Stability**: stable
**Description**: Extracts the W3C trace-id (32 hex chars) from the current OpenTelemetry span context. Returns `None` if no active span or invalid trace-id.
**Breaking Change Policy**: Major version bump required for signature changes.

### 7.2 External Integration Contracts

#### Error Registration Contract

- [ ] `p2` - **ID**: `cpt-cf-ues-contract-error-registration`

**Direction**: required from client (modules register their errors)
**Protocol/Format**: Rust trait method (`on_ready` lifecycle hook)
**Compatibility**: Modules call `ctx.types_registry().register_errors()` with a list of `ErrorDefinition` constants.

#### Problem JSON Response Contract

- [ ] `p1` - **ID**: `cpt-cf-ues-contract-problem-json`

**Direction**: provided by library
**Protocol/Format**: HTTP/REST, `application/problem+json`
**Compatibility**: Backward compatible — new metadata fields may be added by error types without breaking consumers. Field removal requires major version bump.

## 8. Use Cases

#### Define Module Error

- [ ] `p2` - **ID**: `cpt-cf-ues-usecase-define-error`

**Actor**: `cpt-cf-ues-actor-module-dev`

**Preconditions**:
- `modkit-errors` crate is a dependency
- Module has domain-specific error conditions to expose via API

**Main Flow**:
1. Developer defines an error struct with `#[gts_error]` attribute specifying GTS type, base, status, and title
2. Macro generates constants (`GTS_ID`, `STATUS`, `TITLE`, `ERROR_DEF`), `into_problem()`, `Display`, and `Error` implementations at compile time
3. Developer maps domain errors to the struct in a `to_problem()` method
4. Developer registers error definitions in the module's `on_ready` lifecycle hook

**Postconditions**:
- Error type is available for use in API handlers
- Error type is registered in the types registry

**Alternative Flows**:
- **Invalid GTS ID format**: Compilation fails with descriptive error message
- **Missing required attribute**: Compilation fails indicating which attribute is missing

#### Handle API Error Response

- [ ] `p2` - **ID**: `cpt-cf-ues-usecase-handle-error`

**Actor**: `cpt-cf-ues-actor-api-consumer`

**Preconditions**:
- Client has made a REST API request that resulted in an error

**Main Flow**:
1. Client receives HTTP error response with `Content-Type: application/problem+json`
2. Client parses the `type` field to determine the error category
3. Client uses `status` for HTTP-level handling and `metadata` for error-specific context
4. Client optionally logs the `trace_id` for support escalation

**Postconditions**:
- Client has programmatically handled the error based on its GTS type
- Trace ID is available for cross-referencing with server-side logs

**Alternative Flows**:
- **Unknown error type**: Client falls back to HTTP status code handling

## 9. Acceptance Criteria

- [ ] All REST API endpoints return errors conforming to the Problem schema
- [ ] Every error response includes a valid GTS type URI
- [ ] No error response contains sensitive data (credentials, SQL, stack traces, internal hostnames)
- [ ] All module error types are registered in the types registry at startup
- [ ] API consumers can programmatically distinguish error types using the `type` field
- [ ] Trace IDs in error responses correlate with server-side observability data

## 10. Dependencies

| Dependency | Description | Criticality |
|------------|-------------|-------------|
| Types Registry module | Stores and serves registered error type definitions | p1 |
| OpenTelemetry / tracing | Provides trace context for trace ID extraction | p1 |
| `http` crate | Type-safe HTTP status codes (`StatusCode`) | p1 |
| `serde` / `serde_json` | JSON serialization of Problem responses | p1 |

## 11. Assumptions

- All CyberFabric modules use the modkit framework and have access to the `on_ready` lifecycle hook for error registration
- OpenTelemetry tracing is configured in the HTTP middleware layer, providing valid trace context for error responses
- The types registry module may be temporarily unavailable at startup; registration is best-effort and non-blocking (see `cpt-cf-ues-fr-error-registration`)
- Error versioning policy is deferred for MVP: error `code` fields remain stable for the same semantic meaning; version bumps occur only on breaking semantic changes (status code or handling behavior changes). Minor metadata changes (title wording, docs) do not require version bumps

## 12. Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Modules bypass the unified system by constructing raw JSON error responses | Inconsistent error format for API consumers | Enforce via dylint lint rules and code review; framework-level response handlers |
| GTS ID collisions between modules | Ambiguous error classification | Namespace errors by module (e.g., `cf.types_registry.*`, `cf.file_parser.*`); validate uniqueness at registration |
| Trace ID unavailable in non-HTTP contexts | Missing correlation data in error responses | `trace_id` is `Option<String>` — gracefully `None` when no span context exists |

## 13. Open Questions

- None — the design is based on an existing implementation with proven patterns.

## 14. Traceability

- **Design**: [DESIGN.md](./DESIGN.md)
- **ADRs**: [ADR/](./ADR/)
