<!-- cpt:#:adr -->
# ADR-0001: Embedded PostgreSQL-Backed Job Queue: Build vs Adopt

<!-- cpt:id:adr covered_by="DESIGN" -->
**ID**: `cpt-service-jobs-adr-embedded-pg-job-system`

<!-- cpt:##:meta -->
## Meta

<!-- cpt:paragraph:adr-title -->
**Title**: ADR-0001 Embedded PostgreSQL-Backed Job Queue: Build vs Adopt
<!-- cpt:paragraph:adr-title -->

<!-- cpt:paragraph:date -->
**Date**: 2026-02-09
<!-- cpt:paragraph:date -->

<!-- cpt:paragraph:status -->
**Status**: Proposed
<!-- cpt:paragraph:status -->
<!-- cpt:##:meta -->

<!-- cpt:##:body -->
## Body

<!-- cpt:context -->
**Context**:

The platform needs a lightweight async job execution system for native Rust handlers with retry, cancellation, progress reporting, and restart recovery. Several existing Rust job queue libraries exist. Should we adopt one of them, or build a purpose-built embedded system? If we adopt, how do we integrate tenant isolation and comply with the Secure ORM policy, given that no existing library supports multi-tenancy and all use raw SQL internally?

This decision directly addresses the following requirements from PRD/DESIGN:

* `cpt-service-jobs-constraint-no-external` — Uses PostgreSQL we already operate; no new infrastructure
* `cpt-service-jobs-principle-local-workers` — Workers run as Tokio tasks in-process, matching our deployment model
* `cpt-service-jobs-principle-two-types` — Restartable jobs via persistent queue; non-restartable jobs via in-memory channel
* `cpt-service-jobs-fr-submit` — Transactional enqueue (job commits atomically with business logic)
* `cpt-service-jobs-fr-restart` — Heartbeat-based orphan detection with fenced reclamation
* `cpt-service-jobs-fr-tenant-scope` — Tenant isolation via input envelope + session variable in execution transaction

External libraries evaluated:

* Underway — PostgreSQL-backed durable jobs with step functions
* Graphile Worker RS — PostgreSQL-backed with LISTEN/NOTIFY wakeups
* rust-task-queue — Redis-backed with auto-scaling
* kafru — SurrealDB-backed distributed tasks with cron scheduling
* backie — Async background jobs on Tokio with PostgreSQL

Implementation paths under consideration:

* **Option A: Purpose-built system** — Custom job queue using Tokio tasks, PostgreSQL via Secure ORM
* **Option B: Upstream contribution to Underway** — Submit metadata + execution hook PR to Underway; adopt if merged
* **Option C: Fork Underway** — Maintain a thin fork with Secure ORM integration

### Core Incompatibility: Secure ORM vs External Job Libraries

The Secure ORM policy (`docs/modkit_unified_system/06_secure_orm_db_access.md`) states:
* "Modules cannot access raw database connections/pools"
* "No plain SQL in handlers/services/repos. Raw SQL is allowed only in migration infrastructure."

Underway (and every other evaluated library) requires a raw `PgPool` and executes raw SQL internally. This is not a gap that can be papered over with naming conventions — the module must provide a raw pool to Underway, and Underway uses it for 81+ raw SQL queries. This is a hard policy violation.
<!-- cpt:context -->

<!-- cpt:decision-drivers -->
**Decision Drivers**:

* Correctness of queue mechanics (claiming, fencing, retry) is the highest-risk area — distributed job queues are notoriously hard to implement correctly
* Must comply with the Secure ORM policy: no raw SQL in module code, no raw database connections/pools in module code (`docs/modkit_unified_system/06_secure_orm_db_access.md`)
* Must integrate tenant isolation — but the mechanism can be application-level, not necessarily Secure ORM on every query
* Must support two work types (restartable + non-restartable) under a single API
* Must avoid new infrastructure dependencies
* Must run embedded within the service process as Tokio tasks
* PR review identified three design gaps (transactional enqueue, LISTEN/NOTIFY, heartbeat leases) that existing libraries already solve
<!-- cpt:decision-drivers -->

<!-- cpt:options repeat="many" -->
**Option 1: Purpose-Built System (Option A)**

- Description: Build custom queue mechanics using Secure ORM for all database access. Custom job system using Tokio tasks, PostgreSQL via Secure ORM.
- Pros:
  - Full Secure ORM compliance — no raw SQL, no raw pools, no policy exceptions needed
  - Full Secure ORM integration on every query (tenant isolation enforced at the database layer)
  - Two-type model natively supported
  - GTS handler discovery integrated naturally
- Cons:
  - Must implement and maintain all queue mechanics ourselves
  - PR review identified three correctness gaps (transactional enqueue, heartbeat fencing, LISTEN/NOTIFY) — high risk of getting these wrong
  - Significant engineering investment to reach the correctness level Underway already provides
- Trade-offs: Full policy compliance and native tenant isolation at the cost of significant engineering risk in implementing correct queue mechanics (claiming, fencing, retry). The three correctness gaps identified in PR review represent hard distributed-systems problems that Underway has already solved.
<!-- cpt:options repeat="many" -->

<!-- cpt:options repeat="many" -->
**Option 2: Upstream Contribution to Underway (Option B)**

- Description: Submit a metadata + execution hook PR to Underway (see Upstream Contribution Strategy below). If merged, adopt Underway with a platform-approved adapter that encapsulates the raw pool. Underway is a PostgreSQL-backed durable job library with step-function workflows. 156 stars, v0.2.0, actively maintained (last commit Jan 2026). Uses sqlx.
- Pros:
  - Leverages Underway's battle-tested queue mechanics
  - Upstream PR benefits the broader community
  - Cleaner tenant integration via metadata column + execution hook
  - Transactional enqueue — `enqueue` accepts any `PgExecutor`, including an active transaction
  - Heartbeat-based lease with fencing prevents split-brain on reclaimed tasks (fixed Jan 2026)
  - Step-function model allows multi-stage workflows with per-step checkpointing
  - Advisory locks provide per-task concurrency control
  - `FOR UPDATE SKIP LOCKED` for atomic claiming
  - Uses sqlx — same driver as our stack, compatible PgPool
- Cons:
  - Upstream may reject the PR — uncertain timeline
  - Still requires a raw `PgPool` (even if encapsulated, it exists in the module's dependency tree)
  - Requires a policy exception or platform-level wrapper for the pool
  - Hardcoded `underway` schema with 81+ raw SQL queries — cannot route through Secure ORM
  - `InProgressTask` struct and INSERT/RETURNING queries are sealed — no custom columns without forking
  - No multi-tenancy concept; requires application-level workaround
  - Pre-1.0 (v0.2.0) — API may change
- Neutral: Polling-based dispatch (no LISTEN/NOTIFY for new-task wakeups)
- Trade-offs: Gains correctness from a battle-tested library at the cost of a Secure ORM policy exception and dependency on upstream acceptance. The upstream contribution path is designed to be general-purpose (not tenant-specific) to maximize acceptance likelihood.

#### Upstream Contribution Strategy

Underway's sealed internals make tenant integration workable but awkward. A small upstream contribution would make it clean for all multi-tenant users. The proposal is designed to be **general-purpose** (not tenant-specific) to maximize acceptance likelihood.

**Proposed Upstream PR: Task Metadata + Execution Hook**

**1. Add a `metadata` column to `underway.task`:**

```sql
ALTER TABLE underway.task ADD COLUMN metadata JSONB NOT NULL DEFAULT '{}';
```

General-purpose per-task context that Underway stores and returns, but does not interpret. Useful for: tenant IDs, trace context, audit info, custom routing.

**2. Thread `metadata` through the API:**

- `Queue::enqueue()` accepts optional `metadata: serde_json::Value`
- INSERT includes `metadata` column
- Dequeue RETURNING includes `metadata`
- `InProgressTask` carries `metadata: serde_json::Value`

**3. Add an `ExecutionHook` trait:**

```rust
/// Called by the worker between dequeue and task execution.
/// Use this to set session variables, propagate trace context, etc.
pub trait ExecutionHook: Send + Sync + 'static {
    fn before_execute(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        metadata: &serde_json::Value,
    ) -> impl Future<Output = Result<()>> + Send;
}
```

Worker calls `hook.before_execute(&mut tx, &in_progress_task.metadata)` before `task.execute(tx, input)`.

**4. Accept hook in Worker/Queue builder:**

```rust
Worker::new(queue, task)
    .execution_hook(MyTenantHook)  // optional
    .run()
    .await;
```

**Upstream Pitch:**

> "Add per-task metadata and an execution hook for context propagation. This enables multi-tenancy (set session variables from metadata), distributed tracing (propagate trace IDs), audit logging, and custom per-task setup — without modifying the Task trait or breaking existing users."

**If Upstream Rejects:**

Maintain a thin fork with these changes isolated to:
- `queue.rs`: ~20 lines (metadata in INSERT, RETURNING, InProgressTask)
- `worker.rs`: ~10 lines (hook call before execute)
- One migration file

Total diff: ~50 lines. Merge conflicts with upstream are unlikely because the changes touch data flow, not control flow. Rebase cost is low.

**Migration Path:**

1. **Immediate (no fork):** Use `TenantEnvelope` input wrapper + `SET LOCAL` in `Task::execute`. Works today.
2. **Target (upstream PR):** Submit metadata + execution hook PR. If merged, refactor from input envelope to metadata + hook. Cleaner separation of concerns.
3. **Fallback (fork):** If PR is rejected, maintain thin fork. Switch tenant_id from input envelope to metadata column. Same result, better ergonomics.
<!-- cpt:options repeat="many" -->

<!-- cpt:options repeat="many" -->
**Option 3: Fork Underway (Option C)**

- Description: Maintain a thin fork (~50 lines diff) with metadata column, execution hook, and Secure ORM pool integration. Fork can replace raw SQL queries with Secure ORM where feasible, leveraging Underway's correctness properties.
- Pros:
  - Guaranteed to work — no dependency on upstream acceptance
  - Fork can replace raw SQL queries with Secure ORM where feasible
  - Leverages Underway's correctness properties
- Cons:
  - Maintenance burden — must track upstream changes
  - Even with modifications, Underway's internal queries remain raw SQL (replacing all 81+ queries is effectively a rewrite)
- Trade-offs: Guaranteed availability (no upstream dependency) at the cost of ongoing fork maintenance. The fork is intentionally thin (~50 lines), but still requires tracking upstream changes and carries the same raw SQL policy tension as Option B.
<!-- cpt:options repeat="many" -->

<!-- cpt:options repeat="many" -->
**Option 4: Graphile Worker RS**

- Description: Rust port of Node.js Graphile Worker. 69 stars, v0.8.x, actively maintained (last commit Feb 2026). PostgreSQL-backed with LISTEN/NOTIFY wakeups.
- Pros:
  - LISTEN/NOTIFY provides sub-3ms job pickup latency — eliminates the polling anti-pattern
  - Local queue batching reduces DB round-trips under load
  - Lifecycle hooks (JobStart, JobComplete, JobFail) enable observability integration
  - Exponential backoff capped at attempt 10 prevents astronomical delays
- Cons:
  - No heartbeat-based fencing — uses timeout-based recovery (same weakness as our original design)
  - Uses its own private schema (`graphile_worker._private_jobs`) — incompatible with Secure ORM
  - No tenant isolation — single-tenant by design
- Trade-offs: Best-in-class job pickup latency via LISTEN/NOTIFY, but lacks heartbeat fencing (a critical correctness property) and is incompatible with both Secure ORM and multi-tenancy.
<!-- cpt:options repeat="many" -->

<!-- cpt:options repeat="many" -->
**Option 5: Other Libraries (rust-task-queue, kafru, backie)**

- Description: Three additional libraries were evaluated and rejected early due to fundamental incompatibilities.

  **rust-task-queue** — Redis-backed task queue with auto-scaling. 9 stars, v0.1.5, last commit Jan 2026.
  - Pros: Sophisticated 5-metric auto-scaling
  - Cons: Requires Redis (new infrastructure dependency); very early stage (v0.1.5, 9 stars, single maintainer); no PostgreSQL option, no transactional enqueue

  **kafru** — SurrealDB-backed distributed task queue with cron scheduling. 4 stars, v1.0.4, last commit Mar 2025.
  - Pros: Built-in cron scheduling
  - Cons: Requires SurrealDB (new and uncommon dependency); no automatic retry mechanism; minimal adoption (4 stars), no heartbeat/lease mechanism

  **backie** — Async background jobs on Tokio with PostgreSQL via Diesel. 47 stars, v0.9.0. **Archived April 2024.**
  - Pros: Clean trait-based design, `FOR UPDATE SKIP LOCKED`
  - Cons: Archived and unmaintained; uses Diesel (our stack is sqlx-based); no cancellation, no priority queues
- Trade-offs: All three introduce new infrastructure dependencies or are unmaintained. None meet baseline requirements for PostgreSQL-backed transactional enqueue with heartbeat fencing.
<!-- cpt:options repeat="many" -->

<!-- cpt:decision-outcome -->
**Decision Outcome**:

**Undecided.** Underway is the strongest external candidate for queue mechanics (transactional enqueue, heartbeat fencing, advisory locks, atomic claiming), but it is **fundamentally incompatible with the Secure ORM policy** as-is. The decision is between three paths to resolve this (Options A, B, C).

The primary open questions are: (a) Is a Secure ORM policy exception acceptable for an internal queue library, or is strict compliance required? (b) If Underway, is the upstream contribution path viable on our timeline? (c) If purpose-built, what is the acceptable risk for implementing queue mechanics (claiming, fencing, retry) correctly?

### Tenant Isolation Strategy

#### Why RLS Does Not Work

A deep review of Underway's source code ruled out PostgreSQL Row-Level Security:

1. **INSERT lists columns explicitly** (`queue.rs:551-574`) — a `tenant_id` column added to `underway.task` would never be populated by Underway's INSERT
2. **Dequeue RETURNING is hardcoded** (`queue.rs:1006-1014`) — `InProgressTask` is a fixed struct; custom columns are not returned after dequeue
3. **Workers use a shared PgPool** (`worker.rs:824`) — no `SET LOCAL app.tenant_id` before dequeue, so RLS with `current_setting()` would cause workers to see zero tasks

#### Proposed Approach (Options B/C): Input Envelope + Execution-Scoped Session Variable

If Underway is adopted (via upstream contribution or fork), tenant isolation uses the following approach. A purpose-built system (Option A) would use Secure ORM natively and would not need this workaround.

**Enqueue path** — All task inputs are wrapped in a `TenantEnvelope<T>` containing `tenant_id` and the actual `payload`. The `JobService` constructs the envelope internally, taking `tenant_id` from the authenticated `SecurityContext` — never from the caller's input. This prevents tenant ID spoofing at submission. Transactional enqueue works naturally — the envelope is serialized as JSON.

**Execution path** — A `TenantAwareTask<T>` adapter wraps the inner task, implementing the Underway `Task` trait. On execution, it unwraps the envelope, calls `modkit_db::secure::set_tenant_context` on the transaction to set the session variable, then delegates to the inner task. Workers dequeue freely across all tenants (no RLS on `underway.task`). Tenant scoping applies within execution via the platform-provided `set_tenant_context`, which sets the session variable that Secure ORM reads for all business-logic queries.

**Status query path** — A database view (`service_jobs.job_status_v`) projects `tenant_id` from the input JSONB as a first-class UUID column, along with `job_id`, `handler_id`, `status`, `correlation_id`, and timestamps. JSONB extraction and supporting indexes are defined in migration code (where raw SQL is permitted). A SeaORM entity backed by this view uses the `Scopable` derive with `tenant_col = "tenant_id"`, enabling `SecureConn` queries with automatic tenant filtering. This is **fully Secure ORM compliant** — the REST handler uses standard `SecureConn` queries with no raw SQL.

REST endpoints serve **restartable jobs only**. Non-restartable jobs have no database rows and are queryable only via the in-process `JobService` Rust API on the submitting instance.

#### Isolation Guarantees

| Path | Mechanism | Strength |
|---|---|---|
| Enqueue | `tenant_id` injected from `SecurityContext` by `JobService` (never caller-supplied) | Application-level, enforced at API boundary |
| Execution | `modkit_db::secure::set_tenant_context` on transaction | Database session-level (Secure ORM enforced on business tables) |
| Status query (REST) | `SecureConn` + `Scopable` entity on `service_jobs.job_status_v` view | Secure ORM (automatic `WHERE tenant_id IN (...)`) |
| Status query (non-restartable) | In-process Rust API, same-instance only | Application-level (no DB, no cross-instance) |
| Cross-tenant dequeue | Workers see all tenants | By design — workers are shared |

### Comparative Feature Matrix

| Capability | Underway | Graphile Worker RS | rust-task-queue | kafru | backie | Purpose-Built |
|---|---|---|---|---|---|---|
| Backend | PostgreSQL | PostgreSQL | Redis | SurrealDB | PostgreSQL | PostgreSQL |
| Transactional enqueue | Yes | Yes | No | No | No | Must implement |
| LISTEN/NOTIFY | Shutdown only | Yes (sub-3ms) | N/A | No | No | Must implement |
| SKIP LOCKED | Yes | Yes | N/A | No | Yes | Must implement |
| Heartbeat / fencing | Yes (fenced) | No | Yes (60s) | No | Timeout-based | Must implement |
| Retry + backoff | Yes | Yes (exp, capped) | Yes | No | Yes | Must implement |
| Tenant isolation | No | No | No | No | No | Native (Secure ORM) |
| Secure ORM compliance | **No** (raw PgPool) | **No** (raw pool) | N/A | N/A | **No** (Diesel) | **Yes** |
| Two work types | No | No | No | No | No | Yes |
| Cron scheduling | Yes | Yes | No | Yes | No | Must implement |
| Maintained | Yes | Yes | Yes | Stale | Archived | N/A |
<!-- cpt:decision-outcome -->

#### Non-Applicable Domains

- **Compliance (COMPL)**: Not applicable — this ADR does not introduce regulatory or legal obligations. Secure ORM compliance (an internal platform policy) is analyzed in the decision criteria above.
- **UX**: Not applicable — all options present the same programmatic Rust API; there are no user-facing interface differences.

**Consequences**:
<!-- cpt:list:consequences -->
- Positive: Non-restartable jobs remain in-memory only, preserving the two-type model
- Positive: REST status queries can use a database view + `Scopable` entity regardless of the queue backend
- Positive: Whichever option is chosen, the `JobService` API surface remains the same for module consumers
- Negative: REST status endpoints only serve restartable jobs; non-restartable job status is in-process only
- Negative: If Underway is adopted (Options B/C), a Secure ORM policy exception or platform-level wrapper is required for the raw `PgPool`
- Follow-up: Integration test — enqueue within a transaction that rolls back; job must not exist
- Follow-up: Integration test — tenant A cannot see tenant B's jobs via `SecureConn` query on `job_status_v` view
- Follow-up: Integration test — REST status handler uses `SecureConn` (no raw SQL in handler code)
- Follow-up: Integration test — worker heartbeat stops; stale task is reclaimed by another worker with incremented attempt
- Follow-up: Integration test — fenced update; old worker's completion attempt is rejected after reclamation
- Follow-up: Load test — submission latency p99 ≤ 50ms, throughput ≥ 1000 jobs/sec
- Follow-up: Policy test — no raw SQL in module handler/service/repository code (enforceable via dylint)
- Follow-up: Decision — Build vs Adopt. Requires answering: (a) Is a Secure ORM policy exception acceptable for an internal queue library, or is strict compliance required? (b) If Underway, is the upstream contribution path viable on our timeline? (c) If purpose-built, what is the acceptable risk for implementing queue mechanics (claiming, fencing, retry) correctly?
- Follow-up: LISTEN/NOTIFY (review item 7) — Underway uses polling, not LISTEN/NOTIFY. Graphile Worker RS does. Relevant to all three options; a purpose-built system could include LISTEN/NOTIFY from the start.
- Follow-up: Non-restartable job wrapper — Design the in-memory channel path that shares the `JobService` API with the restartable backend. Both types present the same `submit`/`get_status`/`cancel` interface via the Rust API. REST status endpoints serve restartable jobs only; non-restartable jobs are in-process, same-instance only.
- Follow-up: GTS integration — Map GTS handler IDs to queue names at registration time (applies to all options).
<!-- cpt:list:consequences -->

**Links**:
<!-- cpt:list:links -->
- [`PRD`](./PRD.md)
- [`DESIGN`](./DESIGN.md)
- [Underway GitHub](https://github.com/maxcountryman/underway)
<!-- cpt:list:links -->
<!-- cpt:##:body -->

<!-- cpt:id:adr -->
<!-- cpt:#:adr -->

