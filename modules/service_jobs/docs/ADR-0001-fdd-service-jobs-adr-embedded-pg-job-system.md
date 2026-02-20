---
status: proposed
date: 2026-02-09
---

# Embedded PostgreSQL-Backed Job Queue: Build vs Adopt

- [ ] `p1` - **ID**: `fdd-service-jobs-adr-embedded-pg-job-system`

## Context and Problem Statement

The platform needs a lightweight async job execution system for native Rust handlers with retry, cancellation, progress reporting, and restart recovery. Several existing Rust job queue libraries exist. Should we adopt one of them, or build a purpose-built embedded system? If we adopt, how do we integrate tenant isolation and comply with the Secure ORM policy, given that no existing library supports multi-tenancy and all use raw SQL internally?

## Decides For Requirements

This decision directly addresses the following requirements from PRD/DESIGN:

* `fdd-service-jobs-design-no-external` — Uses PostgreSQL we already operate; no new infrastructure
* `fdd-service-jobs-design-local-workers` — Workers run as Tokio tasks in-process, matching our deployment model
* `fdd-service-jobs-design-two-types` — Restartable jobs via persistent queue; non-restartable jobs via in-memory channel
* `fdd-service-jobs-req-submit` — Transactional enqueue (job commits atomically with business logic)
* `fdd-service-jobs-req-restart` — Heartbeat-based orphan detection with fenced reclamation
* `fdd-service-jobs-req-tenant-scope` — Tenant isolation via input envelope + session variable in execution transaction

See:
- **PRD**: [PRD.md](./PRD.md)
- **DESIGN**: [DESIGN.md](./DESIGN.md)

## Decision Drivers

* Correctness of queue mechanics (claiming, fencing, retry) is the highest-risk area — distributed job queues are notoriously hard to implement correctly
* Must comply with the Secure ORM policy: no raw SQL in module code, no raw database connections/pools in module code (`docs/modkit_unified_system/06_secure_orm_db_access.md`)
* Must integrate tenant isolation — but the mechanism can be application-level, not necessarily Secure ORM on every query
* Must support two work types (restartable + non-restartable) under a single API
* Must avoid new infrastructure dependencies
* Must run embedded within the service process as Tokio tasks
* PR review identified three design gaps (transactional enqueue, LISTEN/NOTIFY, heartbeat leases) that existing libraries already solve

## Considered Options

### External libraries evaluated

* Underway — PostgreSQL-backed durable jobs with step functions
* Graphile Worker RS — PostgreSQL-backed with LISTEN/NOTIFY wakeups
* rust-task-queue — Redis-backed with auto-scaling
* kafru — SurrealDB-backed distributed tasks with cron scheduling
* backie — Async background jobs on Tokio with PostgreSQL

### Implementation paths under consideration

* **Option A: Purpose-built system** — Custom job queue using Tokio tasks, PostgreSQL via Secure ORM
* **Option B: Upstream contribution to Underway** — Submit metadata + execution hook PR to Underway; adopt if merged
* **Option C: Fork Underway** — Maintain a thin fork with Secure ORM integration

## Decision Outcome

**Undecided.** Underway is the strongest external candidate for queue mechanics (transactional enqueue, heartbeat fencing, advisory locks, atomic claiming), but it is **fundamentally incompatible with the Secure ORM policy** as-is. The decision is between three paths to resolve this.

### Core Incompatibility: Secure ORM vs External Job Libraries

The Secure ORM policy (`docs/modkit_unified_system/06_secure_orm_db_access.md`) states:
* "Modules cannot access raw database connections/pools"
* "No plain SQL in handlers/services/repos. Raw SQL is allowed only in migration infrastructure."

Underway (and every other evaluated library) requires a raw `PgPool` and executes raw SQL internally. This is not a gap that can be papered over with naming conventions — the module must provide a raw pool to Underway, and Underway uses it for 81+ raw SQL queries. This is a hard policy violation.

### Option A: Purpose-Built System

Build custom queue mechanics using Secure ORM for all database access.

* Good, because full Secure ORM compliance — no raw SQL, no raw pools
* Good, because tenant isolation is native (Scopable entities, SecureConn everywhere)
* Good, because two-type model natively supported
* Bad, because must implement and maintain all queue mechanics ourselves
* Bad, because PR review identified three correctness gaps (transactional enqueue, heartbeat fencing, LISTEN/NOTIFY) — high risk of getting these wrong
* Bad, because significant engineering investment for problems Underway already solves

### Option B: Upstream Contribution to Underway

Submit a metadata + execution hook PR to Underway (see § Upstream Contribution Strategy). If merged, adopt Underway with a platform-approved adapter that encapsulates the raw pool.

* Good, because leverages Underway's battle-tested queue mechanics
* Good, because upstream PR benefits the broader community
* Good, because cleaner tenant integration via metadata column + execution hook
* Bad, because upstream may reject the PR — uncertain timeline
* Bad, because still requires a raw `PgPool` (even if encapsulated, it exists in the module's dependency tree)
* Bad, because requires a policy exception or platform-level wrapper for the pool

### Option C: Fork Underway

Maintain a thin fork (~50 lines diff) with metadata column, execution hook, and Secure ORM pool integration.

* Good, because guaranteed to work — no dependency on upstream acceptance
* Good, because fork can replace raw SQL queries with Secure ORM where feasible
* Good, because leverages Underway's correctness properties
* Bad, because maintenance burden — must track upstream changes
* Bad, because even with modifications, Underway's internal queries remain raw SQL (replacing all 81+ queries is effectively a rewrite)

### Shared Consequences (all options)

* Good, because non-restartable jobs remain in-memory only, preserving the two-type model
* Good, because REST status queries can use a database view + `Scopable` entity regardless of the queue backend
* Neutral, because REST status endpoints only serve restartable jobs; non-restartable job status is in-process only

### Confirmation (applies to whichever option is chosen)

* Integration test: enqueue within a transaction that rolls back → job must not exist
* Integration test: tenant A cannot see tenant B's jobs via `SecureConn` query on `job_status_v` view
* Integration test: REST status handler uses `SecureConn` (no raw SQL in handler code)
* Integration test: worker heartbeat stops → stale task is reclaimed by another worker with incremented attempt
* Integration test: fenced update — old worker's completion attempt is rejected after reclamation
* Load test: submission latency p99 ≤ 50ms, throughput ≥ 1000 jobs/sec
* Policy test: no raw SQL in module handler/service/repository code (enforceable via dylint)

## Pros and Cons of the Options

### Underway

PostgreSQL-backed durable job library with step-function workflows. 156 stars, v0.2.0, actively maintained (last commit Jan 2026). Uses sqlx.

* Good, because transactional enqueue — `enqueue` accepts any `PgExecutor`, including an active transaction
* Good, because heartbeat-based lease with fencing prevents split-brain on reclaimed tasks (fixed Jan 2026)
* Good, because step-function model allows multi-stage workflows with per-step checkpointing
* Good, because advisory locks provide per-task concurrency control
* Good, because `FOR UPDATE SKIP LOCKED` for atomic claiming
* Good, because uses sqlx — same driver as our stack, compatible PgPool
* Neutral, because polling-based dispatch (no LISTEN/NOTIFY for new-task wakeups)
* Bad, because hardcoded `underway` schema with 81+ raw SQL queries — cannot route through Secure ORM
* Bad, because `InProgressTask` struct and INSERT/RETURNING queries are sealed — no custom columns without forking
* Bad, because no multi-tenancy concept; requires application-level workaround
* Bad, because pre-1.0 (v0.2.0) — API may change

### Graphile Worker RS

Rust port of Node.js Graphile Worker. 69 stars, v0.8.x, actively maintained (last commit Feb 2026).

* Good, because LISTEN/NOTIFY provides sub-3ms job pickup latency — eliminates the polling anti-pattern
* Good, because local queue batching reduces DB round-trips under load
* Good, because lifecycle hooks (JobStart, JobComplete, JobFail) enable observability integration
* Good, because exponential backoff capped at attempt 10 prevents astronomical delays
* Bad, because no heartbeat-based fencing — uses timeout-based recovery (same weakness as our original design)
* Bad, because uses its own private schema (`graphile_worker._private_jobs`) — incompatible with Secure ORM
* Bad, because no tenant isolation — single-tenant by design

### rust-task-queue

Redis-backed task queue with auto-scaling. 9 stars, v0.1.5, last commit Jan 2026.

* Good, because sophisticated 5-metric auto-scaling
* Bad, because requires Redis — new infrastructure dependency
* Bad, because very early stage (v0.1.5, 9 stars, single maintainer)
* Bad, because no PostgreSQL option, no transactional enqueue

### kafru

SurrealDB-backed distributed task queue with cron scheduling. 4 stars, v1.0.4, last commit Mar 2025.

* Good, because built-in cron scheduling
* Bad, because requires SurrealDB — new and uncommon dependency
* Bad, because no automatic retry mechanism
* Bad, because minimal adoption (4 stars), no heartbeat/lease mechanism

### backie

Async background jobs on Tokio with PostgreSQL via Diesel. 47 stars, v0.9.0. **Archived April 2024.**

* Good, because clean trait-based design, `FOR UPDATE SKIP LOCKED`
* Bad, because **archived and unmaintained**
* Bad, because uses Diesel (our stack is sqlx-based)
* Bad, because no cancellation, no priority queues

### Purpose-built embedded system

Custom job system using Tokio tasks, PostgreSQL via Secure ORM.

* Good, because full Secure ORM compliance — no raw SQL, no raw pools, no policy exceptions needed
* Good, because full Secure ORM integration on every query (tenant isolation enforced at the database layer)
* Good, because two-type model natively supported
* Good, because GTS handler discovery integrated naturally
* Bad, because must implement and maintain all queue mechanics ourselves
* Bad, because PR review identified three correctness gaps (transactional enqueue, heartbeat fencing, LISTEN/NOTIFY) — high risk of getting these wrong
* Bad, because significant engineering investment to reach the correctness level Underway already provides

## Tenant Isolation Strategy

### Why RLS Does Not Work

A deep review of Underway's source code ruled out PostgreSQL Row-Level Security:

1. **INSERT lists columns explicitly** (`queue.rs:551-574`) — a `tenant_id` column added to `underway.task` would never be populated by Underway's INSERT
2. **Dequeue RETURNING is hardcoded** (`queue.rs:1006-1014`) — `InProgressTask` is a fixed struct; custom columns are not returned after dequeue
3. **Workers use a shared PgPool** (`worker.rs:824`) — no `SET LOCAL app.tenant_id` before dequeue, so RLS with `current_setting()` would cause workers to see zero tasks

### Proposed Approach (Options B/C): Input Envelope + Execution-Scoped Session Variable

If Underway is adopted (via upstream contribution or fork), tenant isolation uses the following approach. A purpose-built system (Option A) would use Secure ORM natively and would not need this workaround.

**Enqueue path** — Wrap all task inputs in a tenant envelope:

```rust
#[derive(Serialize, Deserialize)]
pub struct TenantEnvelope<T> {
    pub tenant_id: Uuid,
    pub payload: T,
}
```

The `JobService` constructs `TenantEnvelope { tenant_id, payload: actual_input }` internally, taking `tenant_id` from the authenticated `SecurityContext` — never from the caller's input. This prevents tenant ID spoofing at submission. Transactional enqueue works naturally — the envelope is just JSON.

**Execution path** — Unwrap envelope and set session context:

```rust
impl<T: Task> Task for TenantAwareTask<T> {
    type Input = TenantEnvelope<T::Input>;
    type Output = T::Output;

    async fn execute(
        &self,
        mut tx: Transaction<'_, Postgres>,
        input: Self::Input,
    ) -> Result<Self::Output> {
        // Set tenant context via platform API (no raw SQL in module code)
        modkit_db::secure::set_tenant_context(&mut tx, input.tenant_id).await?;

        self.inner.execute(tx, input.payload).await
    }
}
```

Workers dequeue freely across all tenants (no RLS on `underway.task`). Tenant scoping applies within `execute()` via the platform-provided `set_tenant_context`, which sets the session variable that Secure ORM reads for all business-logic queries.

**Status query path** — Our REST API uses a database view with a `Scopable` SeaORM entity, not direct JSONB queries in handler code:

```sql
-- Migration: create view that projects tenant_id as a first-class column
CREATE VIEW service_jobs.job_status_v AS
SELECT
    t.id                                  AS job_id,
    t.task_queue_name                     AS handler_id,
    (t.input->>'tenant_id')::uuid         AS tenant_id,
    t.state                               AS status,
    t.input->>'correlation_id'            AS correlation_id,
    t.created_at, t.updated_at, t.completed_at
FROM underway.task t;

-- Indexes for performance (also in migration)
CREATE INDEX idx_task_tenant ON underway.task ((input->>'tenant_id'));
CREATE INDEX idx_task_tenant_queue_state ON underway.task (
    (input->>'tenant_id'), task_queue_name, state
);
```

The SeaORM entity on this view uses `#[secure(tenant_col = "tenant_id")]`, enabling `SecureConn::find::<job_status_v::Entity>(&scope)` with automatic tenant filtering. This is **fully Secure ORM compliant** — the JSONB extraction is in the view definition (migration code, where raw SQL is permitted), and the REST handler uses standard `SecureConn` queries.

REST endpoints serve **restartable jobs only**. Non-restartable jobs have no database rows and are queryable only via the in-process `JobService` Rust API on the submitting instance.

### Isolation Guarantees

| Path | Mechanism | Strength |
|---|---|---|
| Enqueue | `tenant_id` injected from `SecurityContext` by `JobService` (never caller-supplied) | Application-level, enforced at API boundary |
| Execution | `modkit_db::secure::set_tenant_context` on transaction | Database session-level (Secure ORM enforced on business tables) |
| Status query (REST) | `SecureConn` + `Scopable` entity on `service_jobs.job_status_v` view | Secure ORM (automatic `WHERE tenant_id IN (...)`) |
| Status query (non-restartable) | In-process Rust API, same-instance only | Application-level (no DB, no cross-instance) |
| Cross-tenant dequeue | Workers see all tenants | By design — workers are shared |

## Upstream Contribution Strategy

Underway's sealed internals make tenant integration workable but awkward. A small upstream contribution would make it clean for all multi-tenant users. The proposal is designed to be **general-purpose** (not tenant-specific) to maximize acceptance likelihood.

### Proposed Upstream PR: Task Metadata + Execution Hook

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

### Upstream Pitch

> "Add per-task metadata and an execution hook for context propagation. This enables multi-tenancy (set session variables from metadata), distributed tracing (propagate trace IDs), audit logging, and custom per-task setup — without modifying the Task trait or breaking existing users."

### If Upstream Rejects

Maintain a thin fork with these changes isolated to:
- `queue.rs`: ~20 lines (metadata in INSERT, RETURNING, InProgressTask)
- `worker.rs`: ~10 lines (hook call before execute)
- One migration file

Total diff: ~50 lines. Merge conflicts with upstream are unlikely because the changes touch data flow, not control flow. Rebase cost is low.

### Migration Path

1. **Immediate (no fork):** Use `TenantEnvelope` input wrapper + `SET LOCAL` in `Task::execute`. Works today.
2. **Target (upstream PR):** Submit metadata + execution hook PR. If merged, refactor from input envelope to metadata + hook. Cleaner separation of concerns.
3. **Fallback (fork):** If PR is rejected, maintain thin fork. Switch tenant_id from input envelope to metadata column. Same result, better ergonomics.

## Comparative Feature Matrix

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

## Open Items

1. **Decision: Build vs Adopt** — The primary open item. Requires answering: (a) Is a Secure ORM policy exception acceptable for an internal queue library, or is strict compliance required? (b) If Underway, is the upstream contribution path viable on our timeline? (c) If purpose-built, what is the acceptable risk for implementing queue mechanics (claiming, fencing, retry) correctly?
2. **LISTEN/NOTIFY** (review item 7): Underway uses polling, not LISTEN/NOTIFY. Graphile Worker RS does. Relevant to all three options — a purpose-built system could include LISTEN/NOTIFY from the start.
3. **Non-restartable job wrapper**: Design the in-memory channel path that shares the `JobService` API with the restartable backend. Both types present the same `submit`/`get_status`/`cancel` interface via the Rust API. REST status endpoints serve restartable jobs only — non-restartable jobs are in-process, same-instance only.
4. **GTS integration**: Map GTS handler IDs to queue names at registration time (applies to all options).
