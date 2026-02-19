---
status: accepted
date: 2026-02-14
decision-makers: Mike Yastrebtsov, Alex Henry
---

<!--
=============================================================================
ARCHITECTURE DECISION RECORD (ADR) - based on MADR format
=============================================================================
PURPOSE: Capture WHY a significant technical decision was made - context,
options considered, trade-offs, and consequences.

SCOPE:
✓ Context and problem statement
✓ Decision drivers (constraints, quality attributes)
✓ Options considered with pros/cons
✓ Chosen option with justification
✓ Consequences (good and bad)

NOT IN THIS DOCUMENT (see other templates):
✗ Requirements -> PRD.md
✗ Full architecture description -> DESIGN.md
✗ Implementation details -> features/

RULES:
- ADRs represent actual decision dilemma and decision state
- DESIGN is the primary artifact ("what"); ADRs annotate DESIGN with rationale ("why")
- Avoid "everything is a decision"; write ADRs only when the rationale needs to be explained and recorded for traceability
- Decision history is in git, not in documents
- Use single ADR per decision

STANDARDS ALIGNMENT:
- MADR (Markdown Any Decision Records)
- IEEE 42010 (architecture decisions as first-class elements)
- ISO/IEC 15288 / 12207 (decision analysis process)
  ==============================================================================
  -->

# Group chat ownership and usage attribution model (tenant-owned, sender-attributed)

**ID**: `cpt-cf-mini-chat-adr-group-chat-usage-attribution`

**Scope note**: This ADR is **future-facing (P2+)** for group/shared chat ownership and usage attribution. It does not change Mini Chat P0/P1 single-user chat scope.

## Context and Problem Statement

Future phases introduce group chats (shared conversations) where multiple users in the same tenant can read and write to the same chat. This raises two coupled questions: who is considered the “owner” of a group chat resource, and how usage (tokens, tool calls, premium credits) is attributed for quota enforcement, analytics, and billing.

## Decision Drivers

* Maintain strict tenant isolation and align with platform PDP/PEP authorization model.
* Keep quota and rate-limiting semantics predictable and resistant to abuse in shared contexts.
* Preserve clear, actionable analytics: per-tenant billing plus per-user accountability.
* Avoid introducing complex “budget pool” mechanics and support overhead for initial group chat rollout.
* Ensure compatibility with SSE streaming, cancellation, and audit requirements (no hidden usage).

## Considered Options

* Tenant-owned resource with usage attributed to the requesting sender (per request)
* Shared budget pool per chat/project (team budget)
* Creator/owner pays for all usage in the shared chat

## Decision Outcome

Chosen option: "Tenant-owned resource with usage attributed to the requesting sender (per request)", because it best aligns with tenant-based ownership, preserves per-user quotas and rate limits, minimizes abuse in shared chats, and keeps the accounting model simple while still enabling accurate tenant-level billing aggregation.

### Consequences

* Good, because resource ownership stays unambiguous: all shared chats belong to the tenant, while access is controlled via group membership and permissions.
* Good, because quota and rate limiting remain intuitive: the user who clicks “Send” consumes their own premium credits and tool call limits.
* Good, because analytics remain strong: billing aggregates by tenant, while attribution and troubleshooting remain per-user.
* Bad, because some teams may expect a shared “team budget” model, which will require a later extension (budget pools) if demanded by product.
* Bad, because some automation tasks (thread summary maintenance) require a “system” attribution bucket to avoid charging a specific user.
* Neutral, because system-attributed usage MUST be observable to operators (metrics and audit) so tenant billing is explainable and does not appear as “magic” background spend.

### Confirmation

* Design review: verify shared chat resource model is tenant-owned and uses group membership predicates in PDP constraints.
* Integration tests: simulate two users in the same tenant; confirm per-request quota usage increments only for the sender and that access decisions are enforced correctly.
* Audit validation: verify emitted audit events include both `requester_user_id` and tenant billing attribution (`billing_tenant_id = chat.tenant_id`) and that system tasks use `requester_type=system`.
* Metrics validation: verify system-attributed usage is observable via a bounded-label metric series (no tenant/user labels).

## Pros and Cons of the Options

### Tenant-owned resource with usage attributed to the requesting sender (per request)

Shared chat is tenant-owned (`tenant_id`), and each LLM call is attributed to the user who initiated it (`requester_user_id`). Quotas, premium credits, and rate limits are enforced per user, while tenant-level reporting aggregates total usage for billing and MSP visibility (aggregated, non-content only).

* Good, because it matches the platform’s multi-tenant security model and avoids ambiguous “ownership” semantics.
* Good, because it prevents freeloading: users cannot consume someone else’s quota by participating in shared chats.
* Good, because it preserves existing quota tables keyed by `(tenant_id, user_id, period)` with minimal changes.
* Neutral, because teams that want a pooled budget can be supported later by adding an optional budget layer without breaking existing attribution.
* Bad, because maintenance operations (summaries) need a non-user attribution path (system bucket) to avoid charging a human for background work.

### Shared budget pool per chat/project (team budget)

A chat or project has a budget entity; all participants draw from it. Per-user accounting is optional or secondary.

* Good, because it maps to “team pays together” expectations and avoids user-to-user friction over quotas.
* Bad, because it introduces additional complexity: budget lifecycle, replenishment, per-team policies, and abuse controls.
* Bad, because it complicates enforcement: rate limiting becomes ambiguous (per-user vs per-budget) and can encourage burst abuse.
* Bad, because it adds support load: “where did our budget go” becomes a recurring issue.

### Creator/owner pays for all usage in the shared chat

The chat creator is treated as an “owner” and all usage is billed/limited against them.

* Good, because it is simple to explain at first glance.
* Bad, because it is unfair in collaborative settings and becomes a common source of abuse (“let someone else create the chat”).
* Bad, because it makes quotas unpredictable for the creator and discourages adoption of sharing features.
* Bad, because it complicates governance: ownership transfers and admin overrides become operational necessities.

## More Information

* This ADR defines accounting semantics only. Authorization semantics for group chats (membership, read/write/manage roles, share links) are a separate decision and should be captured in a dedicated ADR when Projects/Sharing (P2) is designed.
* This attribution model is also used in P0 for system-initiated background tasks (thread summary refresh, doc summary generation): they use `requester_type=system` and are charged to a tenant operational bucket (not a user), while still being included in tenant-level billing and audit logs.

## Traceability

* **PRD**: [PRD.md](../PRD.md) (mini-chat)
* **DESIGN**: [DESIGN.md](../DESIGN.md) (mini-chat)

This decision directly addresses the following requirements or design elements:

* `cpt-cf-mini-chat-fr-group-chats` - Defines ownership and usage attribution rules required for shared chat functionality (P2+).
* `cpt-cf-mini-chat-nfr-cost-control` - Ensures quota enforcement is predictable and resistant to shared-context abuse.
* `cpt-cf-mini-chat-nfr-tenant-isolation` - Keeps all shared resources tenant-owned with PDP-enforced access paths.
* `cpt-cf-mini-chat-design-quota-usage-accounting` - Specifies how `quota_usage` and audit events are keyed and attributed in shared chats.
