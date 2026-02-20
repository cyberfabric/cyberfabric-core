# PR #482 Review Comments Checklist

**PR:** [WiP] service job design: PRD, DESIGN
**URL:** https://github.com/cyberfabric/cyberfabric-core/pull/482

---

## Bot Comments

- [x] **1. Job glossary missing lifecycle states** — `PRD.md:43` (coderabbitai, Minor)
  Glossary defines Job lifecycle as "pending → running → completed/failed" but later states include `canceled` and `dead_lettered`. Update glossary to include all terminal states.
  **Fixed**: Glossary now reads `pending → running → succeeded/failed/canceled/dead_lettered`, matching §5.1 status tracking.

- [x] **2. Inconsistent cancellation API usage** — `DESIGN.md:676` (graphite-app, Bug)
  Code calls `ctx.is_cancelled()` but `JobContext` only provides `cancellation_token: CancellationToken` field. Should be:
  ```rust
  if ctx.cancellation_token.is_cancelled() {
      return Err(JobError::cancelled());
  }
  ```
  **Fixed**: Registration example (§3.5) now uses `ctx.cancellation_token.is_cancelled()`.

- [x] **3. Invalid GTS IDs in docs** — `DESIGN.md:371` (qodo-code-review, Bug)
  `DESIGN.md` contains `gts.*` strings that don't meet the required GTS segment structure (vendor.org.package.type.version). Will break `make gts-docs` and `make check`. Replace with valid examples, e.g.:
  - `gts.x.core.service_jobs.handler.v1~x.core.reports.generate_report.v1`
  **Fixed**: Replaced both GTS IDs in §3.2 component diagram with valid 5-component segments: `gts.x.core.service_jobs.handler.v1~x.core.reports.generate_report.v1` and `gts.x.core.service_jobs.handler.v1~x.core.streaming.process_stream.v1`. Updated corresponding Scoped ClientHub labels.

---

## Human Comments (MikeFalcon77)

- [x] **4. Wants ADR + review of existing job systems** — `DESIGN.md:1009`
  Requests an ADR to understand the decision flow. Asks whether we reviewed existing job systems:
  1. Underway — durable background jobs on PostgreSQL, Rails/Sidekiq-style
  2. Graphile Worker RS — high-performance PostgreSQL-backed task queue (Rust)
  3. rust-task-queue — Redis-backed with auto-scaling
  4. kafru — distributed tasks (Celery-like), cron scheduling, SurrealDB
  5. backie — async background job queue on Tokio

- [x] **5. Transactional enqueue semantics missing** — `DESIGN.md:10`
  P1 "in-memory" and P2 "persistent" split doesn't enforce transactional ordering between submitting a restartable job and the business action. Risk of lost jobs when the business transaction rolls back. Need transactional enqueue (like sqlxmq) or explicitly state no transactional guarantees.
  **Fixed**: Functional drivers table states "transactional enqueue (job commits atomically with business logic)". Sequence diagram shows `[within caller's tx]`. ADR-0001 lists transactional enqueue as a core requirement.

- [x] **6. Tight coupling of API and worker execution** — `DESIGN.md:104`
  Running workers as Tokio tasks inside the service lacks separation of execution and persistence. Standard durable queues separate execution from API processes. Current design ties API, business logic, and job execution together, reducing isolation and complicating backpressure.
  **Addressed**: §2.1 adds concrete mitigations: dedicated worker Tokio runtime (separate thread pool from API), concurrency limits, bounded channel with backpressure error on submission, per-job timeouts, health check integration. §3.6 worker config adds `channel_capacity` and `worker_runtime_threads`. Design explicitly notes when to reconsider (CPU-intensive workloads, divergent scaling needs) and that the queue schema supports separate worker processes without redesign.

- [x] **7. Polling latency anti-pattern** — `DESIGN.md:637`
  SKIP LOCKED is correct for atomic claim, but poll-interval-based execution start is a latency anti-pattern. Similar PgSQL job systems (Graphile Worker) use LISTEN/NOTIFY for near real-time wakeups. Add this to the design discussion.
  **Addressed**: ADR-0001 comparative matrix shows LISTEN/NOTIFY support per library. Listed as open item in ADR and future consideration in DESIGN. Acknowledged as a trade-off — 1s polling accepted for now, LISTEN/NOTIFY evaluated as a future improvement.

- [x] **8. Orphan detection is brittle** — `DESIGN.md:708`
  Detecting orphaned running jobs by `claimed_at < now() - 5 minutes` is brittle — legitimately long jobs get bounced back to pending. Need heartbeat/lease extensions with visibility timeouts, or a lease token that workers renew. Current heuristic guarantees at-least-twice delivery for longer jobs.
  **Fixed**: §3.8 explicitly states "This eliminates the brittle `claimed_at < now() - 5 minutes` heuristic." Replaced with heartbeat-based fencing — workers send periodic heartbeats, stale tasks are reclaimed with incremented attempt numbers, old worker writes are fenced out.

- [x] **9. Duplicate submission handling undefined** — `DESIGN.md:777`
  Declares uniqueness on (idempotency_key, handler_id, etc.) but doesn't define how duplicate submissions are handled (409? return existing job? merge metadata?). Clarify: should return existing job result or stable job ID so clients can safely retry without side effects.
  **Fixed**: Added `SubmitOptions` struct and "Idempotent Submission Semantics" block to §3.3. Duplicates return the existing job's `JobId` (no error, no merge). Uniqueness scoped to `(tenant_id, handler_id, idempotency_key)`. Maps to Underway's `concurrency_key` for restartable jobs; in-memory check for non-restartable.
