---
status: proposed
date: 2026-02-10
---

# Worker-to-Host Communication & Client Streaming

- [ ] `p2` - **ID**: `fdd-service-jobs-adr-worker-host-communication`

## Context and Problem Statement

The service jobs system supports multiple worker topologies through the modkit deployment model:

| Topology | Description |
|----------|-------------|
| **InProcess** | Workers run as Tokio tasks inside the API process (default, per `fdd-service-jobs-design-local-workers`) |
| **DedicatedRuntime** | Workers run on a separate Tokio runtime within the same OS process (see DESIGN.md § 3.6) |
| **ApiOnly** | Instance serves REST/gRPC only; no local workers (scales API independently) |
| **WorkerOnly** | Instance runs workers only; no API surface (scales workers independently) |

Workers produce status updates and progress events (`report_progress()`) that must reach two consumers:

1. **Worker → API host**: Progress and status changes must reach the API host that serves REST status queries (`fdd-service-jobs-req-rest-status`) and future streaming endpoints. In the InProcess/DedicatedRuntime topologies, this is local. In ApiOnly/WorkerOnly topologies, this is cross-process.
2. **API → Client**: Clients need to follow job progress in real time. Today, clients poll the `job_status_v` view via REST (DESIGN.md § 3.11). Future requirements demand reconnectable streaming where a client can disconnect, reconnect, and replay missed events from a known cursor.

Reconnection is two-sided. A client can disconnect and reconnect, but a worker can also fail — causing the job to be reclaimed by a different worker on a potentially different host (ADR-0001 § 3.8, heartbeat-based fencing). When a job restarts on a different worker, the event stream must remain coherent: connected clients should see the transition (progress regression, attempt boundary), and reconnecting clients should be able to replay the full history including the reclamation.

The current design (DESIGN.md) relies exclusively on PostgreSQL polling for status queries. This ADR evaluates communication mechanisms for both the worker→API path and the API→client streaming path, including hybrid approaches that combine different technologies for each leg.

### Open item from ADR-0001

ADR-0001 § Open Items, item 2: "LISTEN/NOTIFY (review item 7): Underway uses polling, not LISTEN/NOTIFY. Graphile Worker RS does. Relevant to all three options — a purpose-built system could include LISTEN/NOTIFY from the start." This ADR directly addresses that open item.

## Decides For Requirements

This decision directly addresses the following requirements from PRD/DESIGN:

* `fdd-service-jobs-req-status` — Job status tracking: the communication path determines how status changes propagate
* `fdd-service-jobs-req-rest-status` — REST status queries: polling vs push affects latency and scalability
* `fdd-service-jobs-req-progress` — Progress reporting: `report_progress()` produces events that need a delivery path to consumers
* `fdd-service-jobs-req-restart` — Restart recovery: when a job is reclaimed by a different worker, the event stream must remain coherent
* Future: reconnectable streaming — two-sided: clients disconnect/reconnect without missing events, and workers fail/reclaim without breaking the event stream

See:
- **PRD**: [PRD.md](./PRD.md)
- **DESIGN**: [DESIGN.md](./DESIGN.md)
- **ADR-0001**: [ADR-0001](./ADR-0001-fdd-service-jobs-adr-embedded-pg-job-system.md)

## Decision Drivers

* Must work across all four topologies (InProcess through WorkerOnly farm) without requiring topology-specific application code
* Must support two-sided reconnection: (a) client disconnects and replays missed events from a cursor, and (b) worker fails, job is reclaimed by a different worker on a different host, and the event stream remains coherent to connected and reconnecting clients
* Must preserve the "no new infrastructure" constraint (`fdd-service-jobs-design-no-external`) as a baseline option; options requiring new infrastructure must justify the trade-off
* Latency: sub-second worker→API notification, sub-second API→client push
* Message ordering: events for a single job must arrive in order
* Backpressure: slow consumers (clients, API hosts) must not stall workers
* Must fit with existing modkit patterns: `SseBroadcaster` (see `04_rest_operation_builder.md`), Tokio `broadcast` channels (see `08_lifecycle_stateful_tasks.md`), gRPC streaming (see `09_oop_grpc_sdk_pattern.md`)
* Operational complexity: prefer solutions that reuse existing infrastructure (PostgreSQL) over introducing new systems

## Considered Options

### Option 1: PostgreSQL Polling

Workers write status/progress to the database (UPDATE on `underway.task` or equivalent). API hosts and clients poll the `job_status_v` view at intervals.

* Good, because no new infrastructure — reuses existing PostgreSQL
* Good, because works across all topologies (any process with DB access sees updates)
* Good, because simple to implement and reason about
* Good, because inherently persistent — missed updates are just stale reads, not lost messages
* Good, because message ordering is trivial (single source of truth in DB)
* Bad, because latency is bounded by poll interval (typically 1–5 seconds)
* Bad, because polling at scale creates O(clients × poll_rate) database queries
* Bad, because no push semantics — cannot achieve sub-second notification without aggressive polling
* Bad, because no native reconnectable streaming — clients must poll and diff
* Bad, because progress updates create write amplification (UPDATE per progress tick per job)
* Neutral, because this is the current design — no incremental work for the baseline

### Option 2: PostgreSQL LISTEN/NOTIFY

Workers (or a database trigger) issue `NOTIFY job_progress, '{job_id}:{event_json}'` after writing status/progress. API hosts hold a `LISTEN` connection and receive notifications in real time. Clients connect via SSE; the API host bridges LISTEN events to SSE streams.

* Good, because no new infrastructure — built into PostgreSQL
* Good, because sub-millisecond notification latency (PG delivers NOTIFY to LISTENers immediately after COMMIT)
* Good, because works across all topologies — any process with a PG connection can LISTEN
* Good, because eliminates polling for the worker→API path
* Good, because integrates naturally with the existing database-centric design
* Bad, because NOTIFY payloads are limited to 8000 bytes — sufficient for progress events but constrains future use
* Bad, because NOTIFY is fire-and-forget — if a listener is disconnected, messages are lost
* Bad, because no built-in replay/persistence — reconnecting clients miss events delivered while disconnected
* Bad, because single PG connection per listener — scaling to many API hosts means many persistent connections
* Bad, because NOTIFY is delivered per-connection, not per-channel — busy systems with many job types create fan-out overhead on the listener
* Neutral, because reconnectable streaming requires a supplementary mechanism (event log table or application-level sequence numbers)

### Option 3: NATS with JetStream

Workers publish progress events to NATS subjects (e.g., `jobs.{tenant_id}.{job_id}.progress`). API hosts subscribe to relevant subjects. JetStream provides persistence, replay from sequence number, and consumer groups.

* Good, because sub-millisecond pub/sub latency
* Good, because JetStream provides persistent streams with replay from sequence — native reconnectable streaming
* Good, because consumer groups allow multiple API hosts to share load or each get all events (fan-out)
* Good, because subject-based routing enables fine-grained subscriptions (per-job, per-tenant, wildcard)
* Good, because built-in backpressure (flow control, max pending)
* Good, because message ordering guaranteed per subject
* Good, because lightweight — single NATS server binary, small resource footprint
* Good, because future extensibility: event-driven triggers, webhooks, cross-service eventing all become natural
* Bad, because **new infrastructure dependency** — violates `fdd-service-jobs-design-no-external` constraint
* Bad, because adds operational complexity (deployment, monitoring, backup of JetStream state)
* Bad, because requires NATS client in every worker and API host process
* Bad, because introduces a new failure domain — NATS unavailability affects progress delivery (though job execution itself continues via PG)
* Neutral, because the NATS dependency could be justified if the platform adopts NATS for other eventing needs

### Option 4: Redis Pub/Sub + Streams

Workers publish to Redis pub/sub channels for real-time notification and append to Redis Streams for persistence/replay. API hosts subscribe to pub/sub for instant delivery and read Streams for reconnection replay.

* Good, because sub-millisecond pub/sub latency
* Good, because Redis Streams provide persistent, ordered event logs with consumer groups
* Good, because `XREAD BLOCK` with `$` or sequence ID enables efficient reconnectable streaming
* Good, because widely adopted — mature client libraries, well-understood operationally
* Good, because backpressure via stream max-length trimming
* Bad, because **new infrastructure dependency** — violates `fdd-service-jobs-design-no-external`
* Bad, because dual mechanism (pub/sub for real-time + Streams for replay) adds complexity
* Bad, because Redis is single-threaded — high event volume may bottleneck on a single Redis instance
* Bad, because Redis Streams consumer groups add operational complexity (managing pending entries, ACKs)
* Bad, because data in Redis is not durable by default (AOF/RDB tradeoffs); JetStream (Option 3) is purpose-built for durable streaming

### Option 5: In-Process Tokio Broadcast Channels

Workers send progress events via Tokio `broadcast::channel`. API handlers subscribe to the broadcast channel and fan out to SSE clients.

* Good, because zero-latency, zero-overhead for InProcess/DedicatedRuntime topologies
* Good, because native Rust — no serialization, no network, no external dependencies
* Good, because existing modkit pattern (`08_lifecycle_stateful_tasks.md` documents broadcast + `select!`)
* Good, because natural backpressure via channel capacity (lagged receivers get `RecvError::Lagged`)
* Bad, because **only works within a single OS process** — incompatible with ApiOnly/WorkerOnly topologies
* Bad, because no persistence — receivers that disconnect miss all events
* Bad, because no reconnectable streaming without an additional event log
* Bad, because channel is per-process; multi-instance API deployments each only see events from their local workers
* Neutral, because this is the optimal choice for InProcess but insufficient as the sole mechanism

### Option 6: gRPC Streaming (Worker → API)

Workers establish gRPC streams to the API host (or a dedicated notification service) and push progress events via server-streaming or bidirectional-streaming RPCs. This extends the existing OoP gRPC pattern (`09_oop_grpc_sdk_pattern.md`).

* Good, because fits the established OoP module pattern — workers already use gRPC for cross-process communication
* Good, because low latency — events arrive as soon as they're sent
* Good, because type-safe proto contracts
* Good, because HTTP/2 multiplexing — multiple streams over one connection
* Bad, because requires workers to know the API host's address (service discovery overhead)
* Bad, because connection-oriented — worker must reconnect if the API host restarts; events during disconnection are lost
* Bad, because no persistence or replay — lost connection means lost events
* Bad, because adds gRPC server surface to API hosts specifically for worker notifications (currently API hosts expose REST, not gRPC, to workers)
* Bad, because N workers × M API hosts creates O(N×M) stream management complexity
* Neutral, because only addresses worker→API; client→streaming still needs a separate solution

### Option 7: SSE (Server-Sent Events) for Client-Facing Delivery

API hosts expose an SSE endpoint (e.g., `GET /jobs/:job_id/events`). Clients connect and receive a stream of progress/status events. This uses the existing `SseBroadcaster` pattern from modkit (`04_rest_operation_builder.md`, `.sse_json()` builder).

* Good, because existing modkit pattern — `OperationBuilder::sse_json::<T>()` and `SseBroadcaster` are production-ready
* Good, because HTTP/1.1 compatible — works through all proxies and load balancers
* Good, because built-in reconnection protocol — `Last-Event-ID` header on reconnect
* Good, because simple client implementation (browser `EventSource` API, curl, any HTTP client)
* Good, because one-directional (server→client) matches the use case perfectly
* Good, because per-job or per-tenant subscription granularity via URL path
* Bad, because SSE is client-facing only — does not solve worker→API communication for cross-process topologies
* Bad, because `Last-Event-ID` requires the server to maintain or reconstruct the event history for replay
* Bad, because long-lived HTTP connections consume file descriptors and memory on the API host
* Bad, because no built-in backpressure — slow clients accumulate buffered events in memory
* Neutral, because SSE is the natural choice for the API→client leg regardless of the worker→API mechanism chosen

## Hybrid Approaches

The worker→API and API→client paths have different characteristics and constraints. Hybrid approaches combine the best mechanism for each leg.

### Hybrid A: PG LISTEN/NOTIFY (worker→API) + SSE (API→client)

**Worker→API**: Workers write status/progress to the database. A PostgreSQL trigger fires `NOTIFY` on the relevant channel. Each API host maintains a single `LISTEN` connection and receives notifications.

**API→client**: The API host bridges received NOTIFY events into `SseBroadcaster` channels. Clients connect via SSE and receive events in real time.

**Reconnectable streaming**: An `event_log` table persists events with a monotonic sequence number per job. On SSE reconnect, the API host reads missed events from the table using the `Last-Event-ID` as a cursor, replays them, then switches to live LISTEN/NOTIFY delivery.

```
Worker ──(INSERT/UPDATE)──► PostgreSQL ──(NOTIFY)──► API Host ──(SSE)──► Client
                                │                        │
                          event_log table ◄──(replay)────┘
```

* Good, because no new infrastructure — PostgreSQL only
* Good, because sub-second latency for both legs
* Good, because reconnectable via event_log + Last-Event-ID
* Good, because works across all topologies
* Bad, because event_log table adds write amplification and storage
* Bad, because NOTIFY is fire-and-forget — requires the event_log fallback for reliability

### Hybrid B: NATS JetStream (worker→API) + SSE (API→client)

**Worker→API**: Workers publish events to NATS JetStream subjects. API hosts subscribe with durable consumers.

**API→client**: The API host bridges JetStream events into SSE streams. On client reconnect, the API host replays from JetStream using the client's `Last-Event-ID` mapped to a JetStream sequence number.

```
Worker ──(publish)──► NATS JetStream ──(subscribe)──► API Host ──(SSE)──► Client
```

* Good, because JetStream handles persistence, replay, and ordering natively
* Good, because no custom event_log table needed
* Good, because cleanest reconnectable streaming implementation
* Good, because future-proof for platform-wide eventing
* Bad, because NATS is new infrastructure
* Bad, because operational overhead of running NATS

### Hybrid C: Tokio Broadcast (in-process) + PG LISTEN/NOTIFY (cross-process) + SSE (client)

**InProcess/DedicatedRuntime**: Workers push events directly to a Tokio `broadcast` channel. The `SseBroadcaster` subscribes to this channel. Zero overhead, zero latency.

**ApiOnly/WorkerOnly**: Workers write to the database and trigger `NOTIFY`. API hosts receive via `LISTEN` and bridge into the `SseBroadcaster`.

**Reconnectable streaming**: Same event_log approach as Hybrid A. The SSE endpoint reads from event_log on reconnect regardless of the upstream source.

```
InProcess path:     Worker ──(broadcast)──► SseBroadcaster ──(SSE)──► Client

Cross-process path: Worker ──(INSERT)──► PostgreSQL ──(NOTIFY)──► API Host ──(broadcast)──► SseBroadcaster ──(SSE)──► Client

Reconnect path:     Client ──(Last-Event-ID)──► API Host ──(SELECT from event_log)──► replay ──(SSE)──► Client
```

* Good, because optimal performance in InProcess (zero serialization, zero network)
* Good, because no new infrastructure for any topology
* Good, because each topology uses the best-fit mechanism
* Good, because reconnectable via event_log + Last-Event-ID
* Bad, because two code paths (broadcast vs LISTEN/NOTIFY) increase complexity and testing surface
* Bad, because event_log writes still needed for reconnectable streaming even in InProcess mode
* Neutral, because the abstraction layer (`ProgressSink` trait) can unify both paths behind a single interface

## Comparison Matrix

| Dimension | PG Polling | PG LISTEN/NOTIFY | NATS JetStream | Redis Pub/Sub + Streams | Tokio Broadcast | gRPC Streaming | SSE |
|-----------|-----------|-----------------|----------------|------------------------|----------------|---------------|-----|
| **Worker→API latency** | 1–5s (poll interval) | <10ms | <5ms | <5ms | <1ms (in-process) | <5ms | N/A (client-facing) |
| **API→Client latency** | 1–5s (poll interval) | N/A (needs bridge) | N/A (needs bridge) | N/A (needs bridge) | N/A (needs bridge) | N/A | <10ms |
| **Client reconnect (replay)** | No (poll & diff) | No (fire-and-forget) | Yes (JetStream replay) | Partial (Streams replay, pub/sub lossy) | No (in-memory only) | No (connection-scoped) | Yes (with Last-Event-ID + server-side log) |
| **Worker migration (job reclaim)** | Yes (DB is source) | Yes (NOTIFY from any host) | Yes (any publisher) | Yes (any publisher) | No (channel is per-process) | No (stream is per-connection) | N/A (client-facing) |
| **Works across all topologies** | Yes | Yes | Yes | Yes | InProcess only | Cross-process only | Client-facing only |
| **No new infrastructure** | Yes | Yes | No (NATS) | No (Redis) | Yes | Yes (extends existing gRPC) | Yes |
| **Message ordering (per-job)** | Yes (DB is source of truth) | Yes (COMMIT-ordered) | Yes (per-subject) | Yes (per-stream) | Yes (per-channel) | Yes (per-stream) | Yes (event ID sequence) |
| **Scalability** | Bad (O(clients × polls)) | Good (push-based) | Excellent (consumer groups, partitions) | Good (consumer groups) | Limited (single process) | Moderate (N×M streams) | Good (one conn per client) |
| **Backpressure** | N/A (pull-based) | None (fire-and-forget) | Yes (flow control) | Yes (stream max-length) | Yes (channel capacity, Lagged) | Yes (HTTP/2 flow control) | None (server buffers) |
| **Operational complexity** | Minimal | Low (PG connection management) | Moderate (NATS cluster) | Moderate (Redis + persistence) | Minimal | Moderate (service discovery) | Low (HTTP endpoint) |
| **Fits existing patterns** | Current design | Natural PG extension | New pattern | New pattern | `08_lifecycle` broadcast | `09_oop_grpc_sdk` | `04_rest_operation_builder` SSE |
| **Future extensibility** | Limited | Moderate (trigger-based) | Excellent (subjects, consumers, webhooks) | Good (Streams, consumers) | Limited | Moderate (new RPCs) | Limited (unidirectional) |

| Dimension | Hybrid A (PG L/N + SSE) | Hybrid B (NATS + SSE) | Hybrid C (Broadcast + PG L/N + SSE) |
|-----------|------------------------|----------------------|--------------------------------------|
| **Worker→API latency** | <10ms | <5ms | <1ms (in-process), <10ms (cross-process) |
| **API→Client latency** | <10ms | <10ms | <10ms |
| **Client reconnect (replay)** | Yes (event_log + Last-Event-ID) | Yes (JetStream + Last-Event-ID) | Yes (event_log + Last-Event-ID) |
| **Worker migration (job reclaim)** | Yes (NOTIFY from any host) | Yes (any publisher) | Yes (broadcast local + PG L/N fallback) |
| **Works across all topologies** | Yes | Yes | Yes |
| **No new infrastructure** | Yes | No (NATS) | Yes |
| **Message ordering** | Yes | Yes | Yes |
| **Scalability** | Good | Excellent | Good |
| **Backpressure** | Partial (SSE leg unbuffered) | Good (JetStream + SSE) | Partial (SSE leg unbuffered) |
| **Operational complexity** | Low–Moderate (event_log maintenance) | Moderate (NATS + SSE) | Moderate (two code paths + event_log) |
| **Fits existing patterns** | Yes (PG + SSE both existing) | Partial (NATS new, SSE existing) | Yes (broadcast + PG + SSE all existing) |
| **Implementation complexity** | Moderate | Moderate | Higher (dual path) |

## Analysis of Key Concerns

### Two-Sided Reconnection

Reconnection is not just a client concern. Both ends of the event stream can break independently:

| Side | Cause | Effect on Event Stream |
|------|-------|----------------------|
| **Client reconnect** | Network drop, browser tab sleep, mobile app backgrounded | Client misses events while disconnected; must replay from a cursor |
| **Worker restart** | Worker crash, heartbeat timeout, reclamation by fencing (ADR-0001 § 3.8) | Job resumes on a different worker — possibly on a different host — and the event source changes |
| **Both simultaneously** | Worker crashes while client is also disconnected | Client reconnects to find the job at a different progress point, produced by a different worker |

The streaming design must handle all three cases coherently.

### Client-Side Reconnection (Replay)

For clients that disconnect and reconnect (mobile apps, browser tabs, network interruptions), the system must replay missed events. This is critical for the `report_progress()` use case — a user watching a report generation should not lose progress visibility on a brief network drop.

**Options with native replay**: NATS JetStream (sequence-based), Redis Streams (ID-based).

**Options requiring an event_log supplement**: PG LISTEN/NOTIFY, Tokio broadcast, gRPC streaming, SSE (all need a persistent event log for replay).

**Recommended approach for PG-based hybrids**: An `event_log` table with a per-job monotonic sequence number. The SSE endpoint assigns each event an `id` field matching this sequence. On reconnect, the client sends `Last-Event-ID` and the server replays from `SELECT * FROM event_log WHERE job_id = $1 AND seq > $2 ORDER BY seq`.

```sql
CREATE TABLE service_jobs.event_log (
    job_id    UUID NOT NULL,
    seq       BIGINT NOT NULL,  -- per-job monotonic, assigned by application
    attempt   INT NOT NULL,     -- attempt number from fencing (ADR-0001)
    event     JSONB NOT NULL,   -- {type: "progress", percent: 42, message: "..."}
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (job_id, seq)
);

-- Retention: events older than 1 hour are pruned by background cleanup
CREATE INDEX idx_event_log_created ON service_jobs.event_log (created_at);
```

### Worker-Side Reconnection (Job Restart / Reclamation)

When a worker fails, the job's heartbeat stops and the fencing mechanism (ADR-0001 § 3.8) reclaims the job for a different worker. This creates several streaming challenges:

#### 1. Event source migration

The new worker may be on a different host. If the API host was receiving events via an in-process broadcast channel (Hybrid C), those events were process-local. The new worker's events come from a different process — or a different machine entirely.

**Impact by option**:

| Mechanism | Handles worker migration? | Why |
|-----------|--------------------------|-----|
| PG Polling | Yes | Source of truth is the database; any worker writes there |
| PG LISTEN/NOTIFY | Yes | NOTIFY is per-database, not per-connection; any worker's COMMIT triggers listeners on all API hosts |
| NATS JetStream | Yes | Publish is to a subject, not a connection; any worker publishes to `jobs.{id}.progress` |
| Redis Pub/Sub + Streams | Yes | Same — channel/stream is decoupled from producer identity |
| Tokio Broadcast | **No** | Channel is per-process; new worker on a different host has a different channel |
| gRPC Streaming | **No** | Stream is per-connection; new worker must establish a new stream to the API host |
| SSE (client-facing) | N/A | Client-facing only; not affected by which worker produces events |

This is a critical weakness of pure in-process broadcast (Option 5) for cross-process topologies. Hybrid C addresses it by falling back to PG LISTEN/NOTIFY for cross-process event delivery, but even in InProcess mode, a job reclaimed by a different instance requires the cross-process path.

#### 2. Progress regression and attempt boundaries

When Worker B picks up a reclaimed job, it may restart from a checkpoint (e.g., 30%) after Worker A had reported 50%. The client sees progress go from 50% → 30%. Without context, this looks like a bug.

**Solution — attempt-scoped events**: Every event carries an `attempt` number (from the fencing mechanism). Status transitions between attempts are explicit:

```json
{"seq": 14, "attempt": 0, "type": "progress", "percent": 50, "message": "Processing..."}
{"seq": 15, "attempt": 0, "type": "worker_lost", "reason": "heartbeat_timeout"}
{"seq": 16, "attempt": 1, "type": "reclaimed", "checkpoint_percent": 30}
{"seq": 17, "attempt": 1, "type": "progress", "percent": 30, "message": "Resuming from checkpoint..."}
{"seq": 18, "attempt": 1, "type": "progress", "percent": 60, "message": "Processing..."}
```

The `worker_lost` and `reclaimed` events are synthetic — generated by the system (not the worker) when reclamation occurs. This gives clients full context for the progress regression.

#### 3. Stale events from fenced workers

A fenced worker (attempt 0) may still emit progress events after reclamation but before it discovers it's been fenced. These events arrive at the API host interleaved with the new worker's events (attempt 1). The event pipeline must discard stale events.

**Filtering rule**: For a given job, the event pipeline tracks the current attempt number. Events with `attempt < current_attempt` are dropped. This is trivial to enforce at the `ProgressSink` or `SseBroadcaster` level.

**Impact by option**:

| Mechanism | Stale event filtering | How |
|-----------|----------------------|-----|
| PG-based (Polling, LISTEN/NOTIFY) | At write time | `INSERT INTO event_log ... WHERE attempt = (SELECT max(attempt) FROM ...)` or application-level check |
| NATS JetStream | At consumer | Consumer filters by attempt in message metadata |
| Tokio Broadcast | At receiver | `SseBroadcaster` filters by attempt before sending to client |
| gRPC Streaming | At receiver | API host filters by attempt |

#### 4. SSE connection affinity after worker migration

Consider: Client is connected via SSE to API Host A. The job was running on a worker co-located with API Host A (InProcess). The worker crashes. The job is reclaimed by a worker on Host B.

- **PG LISTEN/NOTIFY**: API Host A's LISTEN connection receives the NOTIFY from Host B's worker's COMMIT. The SSE stream continues seamlessly.
- **Tokio Broadcast only**: API Host A never receives Host B's broadcast events. The SSE stream goes silent. Client must detect timeout and reconnect (possibly to a different API host).
- **Hybrid C (Broadcast + PG L/N)**: Falls back to the PG LISTEN/NOTIFY path. API Host A receives the event and bridges it to the SSE stream.
- **NATS JetStream**: API Host A's subscription receives events regardless of which worker published them.

This analysis reinforces that Hybrid C requires the PG LISTEN/NOTIFY fallback even when the initial worker was InProcess — because reclamation can move the job to any host.

#### 5. Event stream schema

To support both client and worker reconnection, events must carry enough metadata for consumers to maintain coherent state:

```rust
pub struct JobEvent {
    /// Per-job monotonic sequence number (never resets, even across attempts)
    pub seq: u64,
    /// Attempt number from fencing mechanism (0-indexed)
    pub attempt: u32,
    /// Event type
    pub event_type: JobEventType,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

pub enum JobEventType {
    /// Worker reports progress
    Progress { percent: u8, message: Option<String> },
    /// Job status changed (pending, running, succeeded, failed, canceled, dead_lettered)
    StatusChanged { from: JobStatus, to: JobStatus },
    /// Worker lost — heartbeat timeout detected (synthetic, emitted by reclamation logic)
    WorkerLost { reason: String },
    /// Job reclaimed by a new worker (synthetic, emitted by reclamation logic)
    Reclaimed { from_checkpoint: Option<serde_json::Value> },
}
```

The `seq` is global to the job (never resets across attempts), so clients can always use it as a `Last-Event-ID` cursor regardless of worker restarts.

### Cross-Topology Consistency

The design must produce identical observable behavior regardless of topology. A client connected via SSE should see the same event stream whether the worker is InProcess or on a remote WorkerOnly instance. This means:

- The `ProgressSink` abstraction (used by `report_progress()`) must be topology-agnostic
- Events must flow through a consistent pipeline that ends at the SSE broadcaster
- The event_log (if used) must be written in all topologies to support reconnectable streaming

### Write Amplification

Progress events are frequent (every few percent of completion). Writing every progress event to PostgreSQL adds write load. Mitigation strategies:

- **Throttle at source**: `report_progress()` debounces — at most one DB write per second per job (configurable)
- **Batch writes**: Accumulate events in memory, flush periodically
- **event_log TTL**: Short retention (1 hour) with background cleanup keeps table small
- **UNLOGGED table**: For event_log, since durability across PG restart is not required (the job_status_v view has the canonical state)

### Backpressure on the SSE Leg

SSE has no built-in backpressure. A slow client causes the server to buffer events in memory. Mitigations:

- **Per-connection buffer limit**: Drop connection if buffer exceeds threshold (client will reconnect and replay from event_log)
- **Event coalescing**: Replace queued progress events with the latest one (only the most recent percentage matters)
- **Connection timeout**: Close idle SSE connections after a configurable period

## Decision Outcome

**Undecided.** This ADR presents the analysis for review. The recommended direction is:

1. **Short-term (P1, in-process only)**: Tokio broadcast channels for worker→API within the same process. Zero infrastructure cost. Matches `08_lifecycle_stateful_tasks.md` patterns.

2. **Medium-term (P2, cross-process + client streaming)**: Hybrid A (PG LISTEN/NOTIFY + event_log + SSE) or Hybrid C (adding broadcast for in-process optimization). Both satisfy the "no new infrastructure" constraint.

3. **Long-term (if platform adopts NATS)**: Hybrid B (NATS JetStream + SSE) provides the cleanest reconnectable streaming and best scalability, but only justified if NATS is adopted platform-wide for other eventing needs.

The final decision depends on:
- ADR-0001 outcome (purpose-built vs Underway — affects where `report_progress()` writes)
- Whether the platform plans to adopt NATS for other use cases (amortizes infrastructure cost)
- Acceptable latency for progress delivery (polling at 1s may be sufficient for many use cases)

### Confirmation

**Basic delivery:**
* Integration test: worker calls `report_progress(50, "halfway")` → SSE client receives event within 1 second
* Integration test: cross-process topology (WorkerOnly → ApiOnly) — events arrive at SSE client
* Integration test: InProcess topology — events arrive at SSE client without database round-trip (broadcast path)

**Client-side reconnection:**
* Integration test: SSE client disconnects, worker sends 3 events, client reconnects with `Last-Event-ID` → client receives all 3 missed events in order
* Integration test: client reconnects after worker migration — replay includes `worker_lost`, `reclaimed`, and new-attempt progress events in order

**Worker-side reconnection (job reclamation):**
* Integration test: Worker A reports progress to 50%, heartbeat stops → Worker B reclaims (attempt 1) and resumes from checkpoint → connected SSE client receives `worker_lost` event followed by `reclaimed` event followed by Worker B's progress events, all with correct attempt numbers
* Integration test: Worker A (fenced, attempt 0) emits a stale progress event after reclamation → event is discarded by the pipeline; SSE client never receives it
* Integration test: Worker A is InProcess on Host 1, job is reclaimed by Worker B on Host 2 → SSE client connected to Host 1 receives Worker B's events (via PG LISTEN/NOTIFY fallback or equivalent cross-process path)

**Both sides reconnect:**
* Integration test: Worker A crashes while client is disconnected → Worker B reclaims and emits new events → client reconnects with `Last-Event-ID` from before the crash → client receives Worker A's events, `worker_lost`, `reclaimed`, then Worker B's events — full ordered history

**Scale and consistency:**
* Load test: 100 concurrent jobs reporting progress every 500ms → event_log write throughput is sustainable, SSE delivery latency p99 < 500ms
* Topology test: identical SSE event stream observed by client regardless of InProcess vs WorkerOnly topology
* Ordering test: events for a single job arrive in `seq` order even when produced by different workers across attempts

## Pros and Cons of the Options

_See § Considered Options above for detailed per-option analysis._

## More Information

### Related Patterns in the Codebase

| Pattern | Location | Relevance |
|---------|----------|-----------|
| `SseBroadcaster` + `.sse_json()` | [`04_rest_operation_builder.md`](../../../docs/modkit_unified_system/04_rest_operation_builder.md) | Production-ready SSE pattern for API→client streaming |
| Tokio `broadcast` + `select!` | [`08_lifecycle_stateful_tasks.md`](../../../docs/modkit_unified_system/08_lifecycle_stateful_tasks.md) | In-process event fan-out with cooperative shutdown |
| gRPC streaming / OoP SDK | [`09_oop_grpc_sdk_pattern.md`](../../../docs/modkit_unified_system/09_oop_grpc_sdk_pattern.md) | Cross-process communication via gRPC (existing pattern for OoP modules) |
| `job_status_v` view + `SecureConn` | [DESIGN.md § 3.11](./DESIGN.md) | Current polling-based status query path |
| `report_progress()` on `JobContext` | [DESIGN.md § 3.1](./DESIGN.md) | The progress reporting API that needs a delivery path |

### ProgressSink Abstraction (Sketch)

Regardless of the chosen option, `report_progress()` should write to an abstract sink that the topology configures at startup. The sink receives attempt-scoped events so that downstream consumers can filter stale events from fenced workers:

```rust
/// Topology-agnostic progress delivery.
#[async_trait]
pub trait ProgressSink: Send + Sync + 'static {
    /// Deliver a progress event for a job.
    /// `attempt` is the fencing attempt number from the worker's claim.
    async fn send(
        &self,
        job_id: JobId,
        attempt: u32,
        event: ProgressEvent,
    ) -> Result<(), ProgressError>;

    /// Emit a synthetic event (worker_lost, reclaimed).
    /// Called by the reclamation/fencing logic, not by workers.
    async fn send_system_event(
        &self,
        job_id: JobId,
        event: SystemEvent,
    ) -> Result<(), ProgressError>;
}

/// InProcess: Tokio broadcast channel (filters stale attempts at receiver)
pub struct BroadcastSink { tx: broadcast::Sender<JobEvent> }

/// Cross-process: PG write + NOTIFY (filters stale attempts at write time)
pub struct PgNotifySink { pool: PgPool }

/// Composite: broadcast locally + persist for reconnect + filter stale
pub struct CompositeSink { broadcast: BroadcastSink, persistent: PgNotifySink }
```

The `attempt` parameter is injected by the `JobContext` from the worker's claim metadata — handler code does not set it. Handler code remains simple: `ctx.report_progress(50, "halfway")`. The `JobContext` knows its attempt number and passes it to the sink.

Synthetic events (`WorkerLost`, `Reclaimed`) are emitted by the reclamation logic (the system that detects stale heartbeats and reassigns jobs), not by workers. This ensures these events are emitted exactly once per reclamation, even if the old worker is still running.

### Traceability

- **PRD**: [PRD.md](./PRD.md) — `fdd-service-jobs-req-status`, `fdd-service-jobs-req-rest-status`, `fdd-service-jobs-req-progress`
- **DESIGN**: [DESIGN.md](./DESIGN.md) — § 3.1 `report_progress()`, § 3.11 status view, § 3.16 deployment topology
- **ADR-0001**: [ADR-0001](./ADR-0001-fdd-service-jobs-adr-embedded-pg-job-system.md) — Open Items § LISTEN/NOTIFY
