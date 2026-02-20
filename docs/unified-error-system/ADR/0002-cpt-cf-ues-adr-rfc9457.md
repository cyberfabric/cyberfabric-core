---
status: accepted
date: 2026-02-11
deciders: CyberFabric Core Team
---

# Adopt RFC 9457 Problem Details as Error Response Format

**ID**: `cpt-cf-ues-adr-rfc9457`

## Context and Problem Statement

CyberFabric modules return error responses to REST API consumers. We need a standardized JSON error response format that is consistent across all modules, supports extensibility for module-specific data, and aligns with industry standards to minimize integration friction for API consumers.

## Decision Drivers

* Must provide a consistent error response structure across all modules
* Must support extensibility for module-specific metadata without breaking the base schema
* Must align with industry standards to reduce integration friction
* Must be self-describing — consumers can understand error structure without per-module documentation
* Must support content negotiation via standard media types

## Considered Options

* RFC 9457 Problem Details (`application/problem+json`)
* Custom JSON error format
* GraphQL-style errors (with `extensions` object)
* gRPC Status with details (protobuf `google.rpc.Status`)

## Decision Outcome

Chosen option: "RFC 9457 Problem Details", because it is an IETF standard specifically designed for HTTP API error responses, provides a well-defined extension mechanism for module-specific data, uses a registered media type (`application/problem+json`), and is widely supported by API tooling and client libraries.

### Consequences

* Good, because RFC 9457 is the IETF standard for HTTP API error responses — widely recognized
* Good, because `application/problem+json` media type enables content negotiation
* Good, because extension members (our `metadata` field) allow module-specific data without breaking base schema
* Good, because API tooling (OpenAPI generators, client SDKs) often have built-in RFC 9457 support
* Bad, because RFC 9457 defines `detail` as a standard field, which we intentionally omit for security (see `cpt-cf-ues-adr-security-first`)
* Bad, because RFC 9457's `instance` field (URI identifying specific occurrence) is not used — we use `trace_id` instead

### Confirmation

* All error responses serialize to `application/problem+json` content type
* Problem struct fields match RFC 9457 required members (`type`, `title`, `status`)
* Extension members carried in `metadata` field per RFC 9457 extension mechanism
* Integration tests validate JSON structure against RFC 9457 schema

## Pros and Cons of the Options

### RFC 9457 Problem Details

IETF standard (RFC 9457, formerly RFC 7807) for machine-readable error responses in HTTP APIs. Defines `type`, `title`, `status`, `detail`, `instance` as standard fields with support for extension members.

* Good, because IETF standard with wide industry adoption
* Good, because registered media type (`application/problem+json`)
* Good, because extension members allow custom fields without schema conflicts
* Good, because client libraries and API tooling often support it natively
* Good, because well-defined semantics for each field
* Neutral, because `detail` and `instance` fields are optional — we omit them intentionally
* Bad, because slight deviation from standard by omitting `detail` (documented in separate ADR)

### Custom JSON error format

Project-specific JSON error format (e.g., `{ "error_code": "...", "message": "...", "data": {} }`).

* Good, because full control over field naming and structure
* Good, because can be tailored exactly to project needs
* Bad, because no industry standard — every consumer must learn our format
* Bad, because no registered media type — cannot use content negotiation
* Bad, because no tooling support — client SDK generators cannot auto-generate error handling
* Bad, because maintenance burden — must document and evolve format ourselves

### GraphQL-style errors

JSON error format with `message`, `locations`, `path`, and `extensions` object, following the GraphQL error specification.

* Good, because familiar to GraphQL developers
* Good, because `extensions` object provides extensibility
* Bad, because designed for GraphQL, not REST — `locations` and `path` fields are irrelevant
* Bad, because no registered media type for REST usage
* Bad, because mixing GraphQL conventions in a REST API creates confusion

### gRPC Status with details

Google's `google.rpc.Status` protobuf message with typed `details` field for error metadata.

* Good, because strongly typed via protobuf
* Good, because rich error model with `google.rpc.ErrorInfo`, `google.rpc.BadRequest`, etc.
* Bad, because protobuf-native — requires serialization bridge for REST/JSON responses
* Bad, because overkill for HTTP-first API — adds protobuf dependency for error handling only
* Bad, because not idiomatic for REST APIs

## More Information

- [RFC 9457 — Problem Details for HTTP APIs](https://www.rfc-editor.org/rfc/rfc9457.html)
- [Google AIP-193 — Errors](https://google.aip.dev/193)

## Traceability

- **PRD**: [PRD.md](../PRD.md)
- **DESIGN**: [DESIGN.md](../DESIGN.md)

This decision directly addresses the following requirements or design elements:

* `cpt-cf-ues-fr-rfc9457-schema` — Establishes RFC 9457 as the error response format standard
* `cpt-cf-ues-constraint-rfc9457` — Mandates `application/problem+json` content type
* `cpt-cf-ues-contract-problem-json` — Defines the Problem JSON response contract
