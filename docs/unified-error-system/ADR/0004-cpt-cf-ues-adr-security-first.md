---
status: accepted
date: 2026-02-11
deciders: CyberFabric Core Team
---

# Security-First Error Responses: No Detail Field, Trace-ID Only

**ID**: `cpt-cf-ues-adr-security-first`

## Context and Problem Statement

RFC 9457 defines a `detail` field for human-readable explanations of errors and recommends including `instance` URIs. The W3C Trace Context standard defines a full `traceparent` header with trace-id, span-id, and trace-flags. We need to decide how much information to expose in error responses, balancing debugging utility against security risks of information disclosure.

## Decision Drivers

* Error responses must not leak sensitive internal information (CWE-209)
* Developers commonly put SQL errors, stack traces, and internal hostnames in `detail` fields
* Full W3C traceparent exposes span-id (internal call hierarchy) and trace-flags (sampling strategy)
* API consumers need enough context for programmatic handling and support escalation
* Server-side observability must retain full error details for debugging

## Considered Options

* Security-first: remove `detail`, expose trace-id only, sanitized metadata
* Full RFC 9457: include `detail` and `instance` fields with content guidelines
* Tiered disclosure: different detail levels for internal vs external consumers

## Decision Outcome

Chosen option: "Security-first: remove `detail`, expose trace-id only, sanitized metadata", because it eliminates the most common vector for sensitive data leakage (`detail` field), limits trace context exposure to the minimum needed for correlation (trace-id only), and relies on structured `metadata` fields that are type-safe and auditable via struct definitions.

### Consequences

* Good, because eliminates the `detail` field — the most common source of sensitive data leakage in error responses
* Good, because trace-id (32 hex chars) provides sufficient correlation without exposing internal call hierarchy (span-id) or sampling strategy (trace-flags)
* Good, because `metadata` is populated from declared struct fields — auditable, type-safe, no dynamic injection
* Good, because `#[gts_error(skip_metadata)]` allows internal fields to be excluded from responses while retained for logging
* Bad, because deviates from RFC 9457 by omitting the `detail` field — API consumers expecting standard RFC 9457 may miss it
* Bad, because reduced client-side debugging context — developers must use trace_id to look up full details in observability tools

### Confirmation

* Problem struct does not have a `detail` field — compile-time enforcement
* Only `trace_id` (not full traceparent) appears in responses — validated by integration tests
* Security scan confirms no credentials, PII, SQL errors, stack traces, or internal hostnames in error responses
* `#[gts_error(skip_metadata)]` usage reviewed in code review for sensitive fields

## Pros and Cons of the Options

### Security-first: remove `detail`, expose trace-id only

Remove RFC 9457 `detail` field entirely from the Problem struct. Expose only the trace-id portion (32 hex chars) of the W3C trace context, not the full traceparent. All metadata comes from declared struct fields with `skip_metadata` for internal-only fields.

* Good, because eliminates the primary vector for sensitive data leakage
* Good, because trace-id provides correlation without exposing internal architecture
* Good, because struct fields as metadata source is auditable and type-safe
* Good, because `skip_metadata` gives explicit control over what reaches the client
* Bad, because non-standard RFC 9457 response (missing `detail`)
* Bad, because less client-side context for debugging

### Full RFC 9457: include `detail` and `instance`

Include `detail` (human-readable explanation) and `instance` (URI for specific occurrence) per RFC 9457. Add content guidelines to prevent sensitive data.

* Good, because fully compliant with RFC 9457
* Good, because richer client-side debugging context
* Bad, because `detail` is a free-text string — guidelines are unenforceable at compile time
* Bad, because developers routinely put `.to_string()` on internal errors into `detail`
* Bad, because `instance` URI generation adds complexity without clear value over trace_id
* Bad, because security relies on developer discipline, not structural enforcement

### Tiered disclosure: different detail levels

Expose full details to internal consumers (service-to-service), sanitized details to external consumers. Determined by request origin or authentication level.

* Good, because internal services get richer debugging context
* Good, because external consumers are protected
* Bad, because request origin is spoofable — security boundary is unreliable
* Bad, because two response formats doubles testing surface
* Bad, because complexity of maintaining two detail levels per error
* Bad, because internal services should also use trace_id for correlation — full details belong in logs

## More Information

**Error detail levels by audience**:

| Audience | Access | Detail Level |
|----------|--------|--------------|
| External clients | API response | `type`, `title`, `status`, `trace_id`, `metadata` (sanitized) |
| Internal services | API response | Same as external — use `trace_id` for correlation |
| Developers/QA | Observability tools | Full details via `trace_id` (logs, traces, error chains) |

**Why `detail` was removed**: Developers commonly leak sensitive data (SQL errors, stack traces, hostnames) through free-text `detail` fields. The `title` field provides a static description (per error type, not per occurrence). The `metadata` field provides structured, auditable context. Server-side logging with `trace_id` provides full technical details for debugging.

## Traceability

- **PRD**: [PRD.md](../PRD.md)
- **DESIGN**: [DESIGN.md](../DESIGN.md)

This decision directly addresses the following requirements or design elements:

* `cpt-cf-ues-nfr-no-sensitive-data` — Structural enforcement via no `detail` field and auditable metadata
* `cpt-cf-ues-nfr-sanitized-metadata` — Metadata from struct fields only, with `skip_metadata` for internal data
* `cpt-cf-ues-nfr-server-side-logging` — Full details logged server-side, correlated via trace_id
* `cpt-cf-ues-principle-security-first` — Security-first design principle for error responses
* `cpt-cf-ues-constraint-no-detail` — Intentional omission of RFC 9457 `detail` field
* `cpt-cf-ues-constraint-trace-id-scope` — Only trace-id exposed, not full traceparent
