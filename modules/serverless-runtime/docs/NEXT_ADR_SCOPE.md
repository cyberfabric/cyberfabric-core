# ADR — Identified White Spots And Next ADRs Scope

**Source:** Consistency review of `ADR_DOMAIN_MODEL_AND_APIS.md` against `PRD.md`
**Date:** 2026-01-21
**Updated:** 2026-02-08 (comprehensive consistency and coverage audit)

---

## 1. Unaddressed PRD Requirements

### P0 Requirements Not Fully Addressed

| PRD ID | Requirement                                | Gap Description                                                                                                                                                                                                  | Severity    | New |
|--------|--------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-------------|-----|
| BR-006 | Execution identity context                 | No model for selecting execution identity (system / API client / user) per entrypoint; `SecurityContext` is passed through but entrypoint definitions have no field to declare or constrain which identity the execution runs under; critical for scheduled and event-triggered executions that have no real-time caller | **Blocker** | Yes |
| BR-025 | Secure handling of secrets                 | No `secret_ref` type or secret binding model for workflows to reference secrets                                                                                                                                  | **Blocker** |     |
| BR-030 | Execution error boundaries                 | No error boundary mechanism to contain failures within specific workflow sections and prevent cascading failures; the PRD MUST requirement has no corresponding domain concept                                     | **Blocker** | Yes |
| BR-038 | Injection attack prevention                | No input sanitization rules or validation constraints in schema definitions                                                                                                                                      | **Blocker** |     |
| BR-039 | Privilege escalation prevention            | No privilege scope constraints or execution identity validation model                                                                                                                                            | **Blocker** |     |
| BR-008 | Runtime capabilities (HTTP, events, audit) | No SDK/capability interface for workflow authors to invoke platform services                                                                                                                                      | High        |     |
| BR-009 | Durability / suspension policy             | `max_suspension_days` exists on entrypoint but PRD requires tenant-level configurable suspension policy with three handling options (auto-cancel with notification, indefinite suspension, escalation); `TenantRuntimePolicy` lacks this; state machine only shows `suspended → failed` on timeout | High        | Yes |
| BR-013 | Long-running credential refresh            | No model for token refresh or credential lifecycle in security context                                                                                                                                           | High        |     |
| BR-017 | Data protection and privacy controls       | No sensitive field annotations, data classification model, or controls for restricting who can view sensitive inputs/outputs in execution history; broader requirement than BR-025 (secrets)                       | High        | Yes |
| BR-023 | Audit log integrity                        | No model for protecting audit records from unauthorized modification/deletion or ensuring availability within configured retention period                                                                          | High        | Yes |
| BR-026 | State consistency                          | No concurrency control or consistency guarantee model for concurrent operations and system failures; checkpointing strategy alone does not address "no partial updates or corrupted states" requirement            | High        | Yes |
| BR-034 | Audit trail for definition changes         | Implementation considerations mention audit but no audit event schema for definition CRUD operations (create, modify, enable/disable, delete) with required tenant/actor/correlation fields                       | High        | Yes |
| BR-040 | Resource exhaustion protection             | Limits defined but no detection/termination model for spinning loops or memory leaks                                                                                                                             | High        |     |
| BR-033 | Encryption controls                        | No encryption-at-rest or in-transit specifications in the domain model; may be infrastructure-level but the ADR should reference the requirement and declare expectations                                          | Medium      | Yes |

### P1 Requirements Not Addressed

| PRD ID | Requirement                                     | Gap Description                                                                                         | New |
|--------|-------------------------------------------------|---------------------------------------------------------------------------------------------------------|-----|
| BR-123 | Extensible sharing                              | `OwnerRef` defines default visibility but no sharing mechanism or extension point for cross-user/group/tenant sharing | Yes |
| BR-136 | Graceful disconnection handling                 | No adapter health model or API for rejecting starts when adapter disconnected                           |     |
| BR-101 | Debugging with breakpoints                      | No debugging API or breakpoint model                                                                    |     |
| BR-102 | Step-through execution                          | No step-through control model                                                                           |     |
| BR-104 | Child workflows / modular composition           | No parent-child invocation relationship model                                                           |     |
| BR-105 | Parallel execution with concurrency caps        | No parallel execution model or concurrency controls for steps                                           |     |
| BR-108 | External signals and manual intervention        | Partial (suspend/resume exists), but no signal delivery model                                           |     |
| BR-109 | Alerts and notifications                        | No notification/alert model or subscription mechanism                                                   |     |
| BR-114 | Dependency management                           | No dependency declaration or compatibility model                                                        |     |
| BR-115 | Distributed tracing                             | `trace_id` field exists but no integration or propagation model                                         |     |
| BR-117 | Environment customization (timezone, locale)    | No execution environment configuration model                                                            |     |
| BR-119 | Monitoring dashboards                           | No dashboard model (implementation concern)                                                             |     |
| BR-120 | Performance profiling                           | No profiling model or data schema                                                                       |     |
| BR-121 | Blue-green deployment                           | No deployment strategy model                                                                            |     |
| BR-122 | Publishing governance                           | No review/approval workflow model                                                                       |     |
| BR-125 | Workflow visualization                          | No visualization data model or API                                                                      |     |
| BR-127 | Debugging access control with sensitive masking | No sensitive field annotations in schemas                                                               |     |
| BR-129 | Standardized error taxonomy                     | Base error type exists; specific error types not enumerated                                             |     |
| BR-130 | Debug call trace (masked secrets)               | Call trace not modeled; masking rules not defined                                                       |     |

### P2 Requirements Not Addressed

| PRD ID | Requirement                   | Note                         |
|--------|-------------------------------|------------------------------|
| BR-201 | Long-term archival            | Future scope                 |
| BR-202 | Import/export                 | Future scope                 |
| BR-203 | Execution time travel         | Future scope                 |
| BR-204 | A/B testing                   | Future scope                 |
| BR-205 | Canary releases               | Future scope                 |
| BR-206 | Stronger isolation boundaries | Future scope (sandbox model) |

---

## 2. Next ADR Scope (Recommended)

### ADR-2: Security Model (P0 — Blocker)

**Scope:**

- Execution identity model: how entrypoints declare and constrain execution context (system / API client / user);
  how scheduled and event-triggered executions resolve identity (BR-006)
- Credential lifecycle and token refresh model for long-running workflows (BR-013)
- Secret reference model (`secret_ref` type) and secret binding for entrypoints (BR-025)
- Privilege scope constraints and execution identity validation (BR-039)
- Input sanitization rules and injection prevention patterns (BR-038)
- Data protection model: sensitive field annotations, data classification, masking rules (BR-017, BR-127, BR-130)
- Encryption controls: reference infrastructure requirements for at-rest and in-transit encryption (BR-033)
- Audit log integrity model: protection from modification/deletion, retention guarantees (BR-023)
- Audit event schema for definition CRUD and execution lifecycle events with tenant/actor/correlation fields (BR-034)
- Security context propagation to individual workflow/function steps (BR-024)
- Sandbox isolation model and boundaries

**PRD Coverage:** BR-006, BR-013, BR-017, BR-023, BR-024, BR-025, BR-033, BR-034, BR-038, BR-039, BR-127, BR-130, PRD Risks

### ADR-3: Runtime Capabilities SDK (P0 — High Priority)

**Scope:**

- Capability interface for workflows (HTTP client, event publisher, audit logger)
- Platform operation invocation model
- Resource exhaustion detection and termination model (CPU spinning, memory leaks, excessive I/O)
- Adapter health model and disconnection handling (reject new starts, graceful in-flight handling)

**PRD Coverage:** BR-008, BR-040, BR-136

### ADR-4: Debugging and Observability (P1)

**Scope:**

- Debugging API (breakpoints, step-through, inspection)
- Debug session model and access control
- Call trace schema with duration and masked I/O
- Performance profiling data model
- Distributed tracing propagation model

**PRD Coverage:** BR-101, BR-102, BR-115, BR-120, BR-130

### ADR-5: Advanced Workflow Patterns (P1)

**Scope:**

- Parent-child workflow relationship model
- Parallel execution and concurrency control model
- External signal delivery to suspended workflows
- Dependency declaration and compatibility
- Error boundary mechanisms for containing failures within workflow sections (BR-030)
- State consistency model for concurrent operations and system failures (BR-026)
- Suspension timeout policy: tenant-level configurable handling options (auto-cancel, indefinite, escalation) (BR-009)

**PRD Coverage:** BR-009, BR-026, BR-030, BR-104, BR-105, BR-108, BR-114

### ADR-6: Deployment and Governance (P1)

**Scope:**

- Blue-green deployment strategy model
- Publishing governance (review/approval) workflow
- Alerts and notification model
- Execution environment customization (timezone, locale)
- Extensible sharing model for cross-user/group/tenant definition access (BR-123)

**PRD Coverage:** BR-109, BR-117, BR-121, BR-122, BR-123

### ADR-7: Error Taxonomy (P1)

**Scope:**

- Enumerate specific error types for all failure categories
- Error code registry and documentation
- Error-to-retry-policy mapping

**PRD Coverage:** BR-129
