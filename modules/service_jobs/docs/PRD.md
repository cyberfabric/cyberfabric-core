# PRD — Service Jobs

## 1. Overview

### 1.1 Purpose

Provide a platform capability for executing native Rust functions asynchronously with robust job lifecycle management, status tracking, and failure handling.

### 1.2 Background / Problem Statement

Platform services frequently need to execute operations that:
- Take longer than acceptable HTTP response times
- Require retry and failure handling
- Need progress tracking and status reporting
- Continue running even if the initiating request disconnects
- Survive service restarts without losing work
- Can be initiated at user or system level

Currently, services implement ad-hoc async patterns leading to inconsistent behavior, duplicate code, and unreliable job management. Common pain points include:

- **Lost work on restart**: Tokio tasks are lost when the service restarts
- **No retry logic**: Each service implements its own retry patterns (or none)
- **No visibility**: Difficult to debug failed background operations
- **Duplicate execution**: No idempotency guarantees across restarts

### 1.3 Goals (Business Outcomes)

- Reduce boilerplate code for async operations across platform services
- Improve reliability of background operations through consistent retry and failure handling
- Enable observability and debugging of long-running operations
- Provide a foundation for scheduled and event-triggered background work

### 1.4 Glossary

| Term | Definition |
|------|------------|
| **Job** | A unit of async work with a defined lifecycle (pending → running → succeeded/failed/canceled/dead_lettered). |
| **Job Handler** | A native Rust function that implements the job's business logic. |
| **Job Queue** | The backing store for pending and in-progress jobs. |
| **Job Worker** | A Tokio task within the service process that pulls jobs from the queue and executes handlers. Workers are not separate processes. |
| **Idempotency Key** | A client-provided identifier to prevent duplicate job creation. |
| **Dead Letter Queue (DLQ)** | Storage for jobs that failed after exhausting retries. |
| **Root Tenant** | The platform-level tenant used for system jobs that are not scoped to a specific customer tenant. |

## 2. Actors

### 2.1 Human Actors

#### Platform Service Developer

**ID**: `fdd-service-jobs-actor-developer`

**Role**: Primary users who define job handlers and submit jobs from platform services.
**Needs**: Simple APIs for job submission, clear error handling, minimal boilerplate.

#### Platform Operator

**ID**: `fdd-service-jobs-actor-operator`

**Role**: Monitor job health, handle failures, manage DLQ.
**Needs**: Visibility into job status, ability to retry/cancel jobs, DLQ management.

### 2.2 System Actors

#### Platform Service

**ID**: `fdd-service-jobs-actor-service`

**Role**: Internal services that submit and manage jobs programmatically.

## 3. Operational Concept & Environment

No module-specific environment constraints beyond project defaults.

## 4. Scope

### 4.1 In Scope

- Job submission and lifecycle management APIs (native)
- Native Rust job handler registration and execution
- Configurable retry policies with exponential backoff
- Job status tracking and querying
- Progress reporting for long-running jobs
- Dead letter queue for failed jobs
- Job cancellation
- Idempotent job submission
- Basic scheduling (delayed execution)

### 4.2 Out of Scope

- Tenant-defined functions (see Serverless Runtime module)
- Job submission, cancellation, and lifecycle management via external APIs (only read-only status/result/list queries are exposed via REST)
- Complex workflow orchestration

## 5. Functional Requirements

### 5.1 P1 (Critical)

#### Async Job Submission

- [ ] `p1` - **ID**: `fdd-service-jobs-req-submit`

The system **MUST** allow services to submit jobs and receive a job ID immediately while execution proceeds in the background.

**Rationale**: Enable non-blocking submission of async work.
**Actors**: `fdd-service-jobs-actor-developer`, `fdd-service-jobs-actor-service`

#### Job Status Tracking

- [ ] `p1` - **ID**: `fdd-service-jobs-req-status`

The system **MUST** track job status through its lifecycle with APIs to query current status:
- `pending` - queued, not yet started
- `running` - currently executing
- `succeeded` - completed successfully
- `failed` - failed, may be retried if retries remain
- `canceled` - canceled by request
- `dead_lettered` - failed after retry exhaustion, moved to DLQ for manual review

**Rationale**: Enable visibility into job progress and completion.
**Actors**: `fdd-service-jobs-actor-developer`, `fdd-service-jobs-actor-operator`

#### Native Rust Handler Execution

- [ ] `p1` - **ID**: `fdd-service-jobs-req-handler`

The system **MUST** execute job handlers implemented as native Rust async functions:

```rust
async fn handler(ctx: JobContext, input: T) -> Result<O, JobError>
```

**Rationale**: Provide type-safe, performant job execution.
**Actors**: `fdd-service-jobs-actor-developer`

#### Retry with Backoff

- [ ] `p1` - **ID**: `fdd-service-jobs-req-retry`

The system **MUST** support configurable retry policies with:
- Maximum retry attempts
- Initial backoff delay
- Maximum backoff delay
- Backoff multiplier

**Rationale**: Enable resilient job execution without custom retry logic.
**Actors**: `fdd-service-jobs-actor-developer`

#### Job Result Retrieval

- [ ] `p1` - **ID**: `fdd-service-jobs-req-result`

The system **MUST** store job results (success or error) and provide APIs to retrieve them after completion.

**Rationale**: Allow callers to obtain job output.
**Actors**: `fdd-service-jobs-actor-developer`, `fdd-service-jobs-actor-service`

#### Dead Letter Queue

- [ ] `p1` - **ID**: `fdd-service-jobs-req-dlq`

The system **MUST** move jobs to a dead letter queue after retry exhaustion, preserving job details for analysis and manual recovery.

**Rationale**: Prevent failed jobs from being lost, enable manual intervention.
**Actors**: `fdd-service-jobs-actor-operator`

#### Job Cancellation

- [ ] `p1` - **ID**: `fdd-service-jobs-req-cancel`

The system **MUST** support canceling pending and running jobs. Running jobs receive a cancellation token for cooperative cancellation.

**Rationale**: Allow aborting unnecessary work.
**Actors**: `fdd-service-jobs-actor-developer`, `fdd-service-jobs-actor-operator`

#### Idempotent Submission

- [ ] `p1` - **ID**: `fdd-service-jobs-req-idempotent`

The system **MUST** support idempotency keys to prevent duplicate job creation when the same key is submitted multiple times.

**Rationale**: Prevent duplicate work on retry or network issues.
**Actors**: `fdd-service-jobs-actor-developer`, `fdd-service-jobs-actor-service`

#### REST Job Status API

- [ ] `p1` - **ID**: `fdd-service-jobs-req-rest-status`

The system **MUST** expose a REST API for querying job status and results. The API provides:
- Get job status by ID (status, progress, timestamps)
- Get job result by ID (output or error details)
- List jobs with filtering (by handler, status, date range)

Job submission, cancellation, and DLQ management remain internal (native Rust API only).

**Rationale**: Enable external clients, UIs, and cross-service consumers to poll job status without requiring in-process Rust access.
**Actors**: `fdd-service-jobs-actor-developer`, `fdd-service-jobs-actor-operator`, `fdd-service-jobs-actor-service`

#### Tenant-Scoped Access Control

- [ ] `p1` - **ID**: `fdd-service-jobs-req-tenant-scope`

The system **MUST** enforce tenant-scoped access control for job submission, status queries, result retrieval, cancellation, and DLQ management. Every job has a `tenant_id`; idempotency keys are scoped by tenant and handler. System-level jobs (e.g., platform maintenance) run under the root tenant.

**Rationale**: Prevent cross-tenant data access and ensure isolation.
**Actors**: `fdd-service-jobs-actor-developer`, `fdd-service-jobs-actor-operator`

### 5.2 P2 (Important)

#### Progress Reporting

- [ ] `p2` - **ID**: `fdd-service-jobs-req-progress`

The system **MUST** allow job handlers to report progress (percentage, status message) during execution for long-running jobs.

**Rationale**: Provide visibility into job execution for users and operators.
**Actors**: `fdd-service-jobs-actor-developer`, `fdd-service-jobs-actor-operator`

#### Job Timeout

- [ ] `p2` - **ID**: `fdd-service-jobs-req-timeout`

The system **MUST** enforce configurable timeouts per job type, canceling jobs that exceed their timeout.

**Rationale**: Prevent runaway jobs from consuming resources indefinitely.
**Actors**: `fdd-service-jobs-actor-developer`

#### Completion Callbacks

- [ ] `p2` - **ID**: `fdd-service-jobs-req-callbacks`

The system **MUST** support callbacks when jobs complete (success or failure). Callbacks are defined as methods on the handler trait (`on_success`, `on_failure`), not per-job, ensuring they survive restarts since handlers re-register on startup.

**Rationale**: Enable reactive workflows triggered by job completion.
**Actors**: `fdd-service-jobs-actor-developer`

#### Restart Recovery

- [ ] `p2` - **ID**: `fdd-service-jobs-req-restart`

The system **MUST** detect and recover jobs that were running when the service restarted. Orphaned jobs (status=running but worker is gone) are reset to pending for re-execution.

**Rationale**: Ensure work is not lost on service restart.
**Actors**: `fdd-service-jobs-actor-service`

#### Checkpointing

- [ ] `p2` - **ID**: `fdd-service-jobs-req-checkpoint`

The system **MUST** allow restartable jobs to save checkpoint data during execution. When recovered after restart, the checkpoint data is available to the handler so it can resume from where it left off.

**Rationale**: Enable efficient recovery of long-running jobs.
**Actors**: `fdd-service-jobs-actor-developer`

#### Handler Discovery

- [ ] `p2` - **ID**: `fdd-service-jobs-req-discovery`

The system **MUST** maintain a registry of job handlers discoverable by `handler_id`. This enables looking up the correct handler when recovering orphaned jobs after restart. Handlers register using the Global Type System (GTS) pattern.

**Rationale**: Enable restart recovery by mapping persisted jobs to handlers.
**Actors**: `fdd-service-jobs-actor-service`

#### Large File Download Handler

- [ ] `p2` - **ID**: `fdd-service-jobs-req-download`

The system **MUST** provide a built-in handler for downloading large files with automatic checkpointing. Uses HTTP Range requests to resume downloads after restart.

```rust
job_service.submit(DownloadFileInput {
    url: "https://example.com/large-model.bin",
    dest: "/data/models/model.bin",
    chunk_size: Some(1024 * 1024), // 1MB chunks (optional)
}).await?;
```

**Rationale**: Common use case that demonstrates checkpointing capabilities.
**Actors**: `fdd-service-jobs-actor-developer`

#### Delayed Execution

- [ ] `p2` - **ID**: `fdd-service-jobs-req-delay`

The system **MUST** support scheduling jobs for future execution with a specified delay or execution time.

**Rationale**: Enable scheduled background work.
**Actors**: `fdd-service-jobs-actor-developer`

### 5.3 P3 (Nice-to-have)

#### Job Dependencies

- [ ] `p3` - **ID**: `fdd-service-jobs-req-deps`

The system **MUST** support declaring dependencies between jobs, where a job only runs after its dependencies complete.

**Rationale**: Enable simple workflow patterns.
**Actors**: `fdd-service-jobs-actor-developer`

#### Job Prioritization

- [ ] `p3` - **ID**: `fdd-service-jobs-req-priority`

The system **MUST** support job priority levels so urgent jobs are processed before lower-priority jobs.

**Rationale**: Enable priority-based scheduling.
**Actors**: `fdd-service-jobs-actor-developer`, `fdd-service-jobs-actor-operator`

## 6. Non-Functional Requirements

### 6.1 Module-Specific NFRs

#### Submission Latency

- [ ] `p1` - **ID**: `fdd-service-jobs-req-submission-latency`

The system **MUST** complete job submission within 50ms at p99.

**Threshold**: p99 latency ≤ 50ms
**Rationale**: Job submission should not block request handlers.
**Architecture Allocation**: See DESIGN.md § 1.2 NFR Allocation

#### Execution Start Time

- [ ] `p2` - **ID**: `fdd-service-jobs-req-execution-start`

The system **MUST** start job execution within 1 second of submission under normal load.

**Threshold**: Time from submission to execution start ≤ 1 second (normal load)
**Rationale**: Jobs should start promptly.
**Architecture Allocation**: See DESIGN.md § 1.2 NFR Allocation

#### Throughput

- [ ] `p2` - **ID**: `fdd-service-jobs-req-throughput`

The system **MUST** support at least 1,000 job submissions per second per instance.

**Threshold**: ≥ 1,000 submissions/second/instance
**Rationale**: Support high-volume use cases.
**Architecture Allocation**: See DESIGN.md § 1.2 NFR Allocation

#### Result Retention

- [ ] `p2` - **ID**: `fdd-service-jobs-req-retention`

The system **MUST** retain job results for at least 24 hours (configurable).

**Threshold**: Default retention ≥ 24 hours
**Rationale**: Allow async result retrieval within reasonable window.
**Architecture Allocation**: See DESIGN.md § 3.14 Job Retention and Cleanup

#### Security and Isolation

- [ ] `p1` - **ID**: `fdd-service-jobs-req-security`

The system **MUST** validate job inputs, enforce tenant isolation for all persisted data, and require authenticated/authorized access for all job APIs. Job payloads **MUST NOT** include secrets; secrets must be provided via environment/configuration.

**Threshold**: Zero cross-tenant data leaks; all API calls require authenticated context
**Rationale**: Protect data confidentiality and integrity.
**Architecture Allocation**: See DESIGN.md § 3.18 Security Considerations

## 7. Public Library Interfaces

### 7.1 Public API Surface

#### Job Service API

- [ ] `p1` - **ID**: `fdd-service-jobs-interface-service`

**Type**: Rust module/trait
**Stability**: stable
**Description**: Primary interface for job submission, status queries, and management.
**Breaking Change Policy**: Major version bump required.

#### Job Handler Trait

- [ ] `p1` - **ID**: `fdd-service-jobs-interface-handler`

**Type**: Rust trait
**Stability**: stable
**Description**: Trait that job handlers implement to define execution logic.
**Breaking Change Policy**: Major version bump required.

#### Job Status REST API

- [ ] `p1` - **ID**: `fdd-service-jobs-interface-rest-status`

**Type**: REST API
**Stability**: stable
**Description**: Read-only REST endpoints for querying job status, results, and listing jobs. Requires authenticated requests with valid tenant context.
**Breaking Change Policy**: Major version bump required.

### 7.2 External Integration Contracts

#### Job Status REST Contract

- [ ] `p1` - **ID**: `fdd-service-jobs-contract-rest-status`

**Direction**: provided by library
**Protocol/Format**: HTTP/REST, JSON responses
**Compatibility**: Backward-compatible within major version; new fields may be added to responses without breaking change.

## 8. Use Cases

#### UC: Generate Report Asynchronously

- [ ] `p2` - **ID**: `fdd-service-jobs-req-report`

**Actor**: `fdd-service-jobs-actor-developer`

**Preconditions**:
- Report handler is registered
- User has requested a report

**Main Flow**:
1. Service submits job with report parameters and idempotency key
2. System returns job ID immediately
3. Worker picks up job and executes handler
4. Handler reports progress during generation
5. Handler completes, result stored
6. User polls for status and retrieves result

**Postconditions**:
- Report URL available in job result
- Job status is `succeeded`

**Alternative Flows**:
- **Handler fails**: Job retried per policy, moved to DLQ if exhausted
- **User cancels**: Job marked `canceled`, handler receives cancellation signal

#### UC: Restart Recovery of Long-Running Job

- [ ] `p2` - **ID**: `fdd-service-jobs-req-restart-recovery`

**Actor**: `fdd-service-jobs-actor-service`

**Preconditions**:
- A restartable job with checkpointing is running
- Service crashes or restarts unexpectedly

**Main Flow**:
1. Service restarts and initializes JobService
2. System detects orphaned jobs (status=running, stale claimed_at)
3. Orphaned jobs are reset to pending
4. Worker claims the recovered job
5. Handler receives last checkpoint data via `ctx.checkpoint`
6. Handler resumes processing from checkpoint rather than restarting

**Postconditions**:
- Job completes successfully without duplicating already-checkpointed work
- Job status is `succeeded`

**Alternative Flows**:
- **No checkpoint saved**: Handler starts from the beginning
- **Handler not registered**: Job remains pending until handler re-registers

#### UC: DLQ Management After Retry Exhaustion

- [ ] `p2` - **ID**: `fdd-service-jobs-req-dlq-management`

**Actor**: `fdd-service-jobs-actor-operator`

**Preconditions**:
- A job has failed and exhausted all configured retries

**Main Flow**:
1. Worker detects no retries remaining after final failure
2. System moves job to dead letter queue (status=`dead_lettered`)
3. `on_failure` callback executes on the handler (if defined)
4. Operator queries DLQ via job list API (filter by status)
5. Operator investigates failure details from job result

**Postconditions**:
- Job is in `dead_lettered` status with full error details preserved
- Job is retained for the DLQ retention period (default: 7 days)

**Alternative Flows**:
- **Transient failure resolves**: Operator fixes root cause and resubmits a new job

#### UC: Fire-and-Forget Stream Processing (Non-Restartable)

- [ ] `p2` - **ID**: `fdd-service-jobs-req-non-restartable`

**Actor**: `fdd-service-jobs-actor-developer`

**Preconditions**:
- A non-restartable handler is registered (non-serializable input)

**Main Flow**:
1. Service submits job with `restartable: false`
2. System tracks job in-memory only (no database writes)
3. Worker executes handler with timeout and cancellation support
4. Handler completes, result is available in-memory

**Postconditions**:
- Job status is `succeeded`
- Job benefits from unified timeout, cancellation, and observability

**Alternative Flows**:
- **Service restarts**: Job is lost silently (by design — input is non-serializable)

## 9. Acceptance Criteria

- [ ] Job submission latency meets `fdd-service-jobs-req-submission-latency` threshold
- [ ] Duplicate submissions with same idempotency key return existing job ID
- [ ] Job execution start time meets `fdd-service-jobs-req-execution-start` threshold
- [ ] Failed jobs are retried according to configured policy
- [ ] Jobs that exhaust retries are moved to DLQ
- [ ] Job status is queryable throughout the job lifecycle
- [ ] Job result retention meets `fdd-service-jobs-req-retention` threshold
- [ ] Pending jobs can be canceled immediately
- [ ] Running jobs receive cancellation signal within 1 second
- [ ] (P2) Orphaned jobs are detected within 5 minutes of service restart
- [ ] (P2) Orphaned jobs are re-queued for execution automatically
- [ ] (P2) Handlers with `on_success`/`on_failure` callbacks execute after restart recovery
- [ ] Job access is tenant-scoped; cross-tenant access is denied
- [ ] REST status endpoint returns correct status for all lifecycle states
- [ ] REST endpoints reject unauthenticated requests with 401
- [ ] REST endpoints reject cross-tenant requests with 404
- [ ] REST list endpoint defaults to `created_at DESC` and `limit=50`
- [ ] REST list endpoint clamps `limit` to a maximum of 200

## 10. Dependencies

| Dependency | Description | Criticality |
|------------|-------------|-------------|
| Observability stack | Metrics, logging, tracing | p1 |
| Persistent storage | Queue and result storage for restartable jobs (e.g., PostgreSQL) | p2 |
| Identity service | Caller authentication and context | p1 |
| GTS / TypesRegistry | Handler registration and discovery for restart recovery | p2 |

## 11. Assumptions

- Job handlers are deterministic for retry safety (or explicitly marked non-idempotent)
- Job payload sizes are bounded (configurable limit, e.g., 1MB)
- For restartable jobs (P2): persistent storage backend is available, inputs/outputs are serializable

## 12. Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Queue backup | High job submission rate could overwhelm workers | Rate limiting, backpressure, priority queues |
| Long-running jobs | Jobs that run too long consume worker capacity | Timeouts, limit concurrent long-running jobs |
| State consistency | Job state could become inconsistent during failures | Transactional state updates, idempotent handlers |
| Callback persistence | Function pointers cannot survive restarts | Callbacks are handler methods, handlers re-register on startup |
| Orphaned jobs | Jobs stuck in "running" after unexpected shutdown | Track worker_id and claimed_at, recover stale jobs on startup |

## 13. Open Questions

- Should handlers be auto-discovered? Could use inventory crate for compile-time registration vs explicit registration in `init()`.

## 14. Traceability

- **Design**: [DESIGN.md](./DESIGN.md)
- **ADRs**: [`ADR-0001` Embedded PostgreSQL-Backed Job System](./ADR-0001-fdd-service-jobs-adr-embedded-pg-job-system.md)
- **Features**: Deferred (no feature specs for this module yet)
