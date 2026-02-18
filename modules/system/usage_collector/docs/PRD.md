# PRD — Usage Collector

## 1. Overview

### 1.1 Purpose

A centralized usage ledger (metering system) for reliably collecting, persisting, and exposing raw usage records from all platform sources, with exactly-once semantics, tenant isolation, and type-safe usage definitions. The Usage Collector acts as the authoritative record of resource consumption — it does not aggregate, interpret, or act on the data. Downstream systems (billing, quota enforcement, monitoring, analytics) consume raw usage records and apply their own aggregation rules, policies, and business logic.

### 1.2 Background / Problem Statement

The Usage Collector (UC) serves as the single source of truth for all platform usage data. UC focuses purely on usage capture, storage, and raw data exposure; aggregation, pricing, rating, reconciliation, report generation, and invoicing are handled by downstream consumers. It supports diverse collection patterns optimized for different throughput needs, multiple storage backends, and provides type-safe usage definitions through a schema-based type system.

The service addresses the fragmentation problem where different consumers (billing, monitoring, quota enforcement) implement their own collection logic, leading to inconsistent data and duplicated effort. By centralizing usage collection into a single metering ledger, the platform ensures that all consumers operate on the same accurate, deduplicated raw data and apply their own domain-specific logic independently.

Key problems:

- **Fragmented tracking**: Each consumer implements own collection leading to inconsistent data
- **High-volume ingestion**: Per-event synchronous calls are inefficient at high throughput due to protocol overhead and blocking behavior
- **No custom units**: Cannot meter new resource types (AI tokens) without code changes
- **Storage lock-in**: No flexibility for different retention and performance needs

### 1.3 Goals (Business Outcomes)

- All billable platform services emit usage through UC
- Single source of truth: all downstream consumers (billing, monitoring, quota enforcement) operate on the same raw usage records from UC
- Custom unit registration in less than 5 minutes without code changes
- High-volume services can emit 10,000+ events per second without blocking
- 99.95%+ monthly availability

### 1.4 Glossary

| Term | Definition |
|------|------------|
| Usage Record | A single data point representing resource consumption by a tenant |
| Counter | A monotonically increasing metric (e.g., total API calls) |
| Gauge | A point-in-time metric that can go up or down (e.g., current memory usage) |
| Measuring Unit | A registered schema defining how a usage type is measured (e.g., "ai-credits", "vCPU-hours") |
| Storage Adapter | A plugin that connects UC to a specific storage backend |
| Idempotency Key | A client-provided identifier ensuring exactly-once processing of a usage record |
| Backfill | The process of retroactively submitting historical usage data to fill gaps caused by outages, pipeline failures, or corrections |
| Grace Period | A configurable time window during which late-arriving events are accepted via normal ingestion without requiring explicit backfill |
| Reconciliation | The process of comparing usage data across pipeline stages or external sources to detect gaps and inconsistencies (performed by external systems; UC exposes metadata to support this) |
| Amendment | A correction to previously recorded usage data, either by replacing events in a time range or deprecating individual events |
| Rate Limit | A constraint on the volume of requests a source can submit within a time window |
| Load Shedding | The deliberate dropping or deferral of low-priority work to preserve system stability under overload |
| Record Metadata | An optional, extensible JSON object attached to a usage record, allowing usage sources to include context-specific properties (e.g., LLM model name, token type, geographic region) that are opaque to UC and interpreted by downstream consumers |
| Snapshot Read | A query that sees data as it existed at a specific point in time, providing consistency across paginated requests despite concurrent data modifications |

## 2. Actors

### 2.1 Human Actors

#### Platform Operator

**ID**: `cpt-cf-uc-actor-platform-operator`

**Role**: Configures storage adapters, retention policies, custom measuring units, and monitors system health.
**Needs**: Ability to manage storage backends, define retention policies, register custom units, and monitor system health without code changes.

#### Tenant Administrator

**ID**: `cpt-cf-uc-actor-tenant-admin`

**Role**: Queries raw usage data for their tenant.
**Needs**: Access to raw usage records filtered by type and resource for their tenant only, with time-range filtering.

#### Platform Developer

**ID**: `cpt-cf-uc-actor-platform-developer`

**Role**: Integrates services with UC using SDKs or APIs to emit usage data.
**Needs**: Well-documented SDKs and APIs for emitting usage data with minimal integration effort.

### 2.2 System Actors

#### Usage Source

**ID**: `cpt-cf-uc-actor-usage-source`

**Role**: Any platform service, infrastructure adapter, or gateway that emits usage records (e.g., LLM Gateway, Compute Service, API Gateway).

#### Billing System

**ID**: `cpt-cf-uc-actor-billing-system`

**Role**: Consumes raw usage records from UC for aggregation, rating, pricing, and invoice generation.

#### Quota Enforcement System

**ID**: `cpt-cf-uc-actor-quota-enforcement`

**Role**: Consumes real-time usage data to enforce tenant resource limits and quotas.

#### Monitoring System

**ID**: `cpt-cf-uc-actor-monitoring-system`

**Role**: Consumes usage metrics for dashboards, alerting, and operational visibility.

#### Types Registry

**ID**: `cpt-cf-uc-actor-types-registry`

**Role**: Provides schema validation for usage types and custom measuring units.

#### Storage Backend

**ID**: `cpt-cf-uc-actor-storage-backend`

**Role**: Persists usage records (ClickHouse, PostgreSQL, or external system via adapter).

## 3. Operational Concept & Environment

No module-specific environment constraints beyond project defaults.

## 4. Scope

### 4.1 In Scope

- Client-side SDK with batching (primary ingestion path)
- API for usage ingestion and querying (Rust API, gRPC, HTTP)
- Counter and gauge metric semantics
- Per-tenant, per-subject (user, service account, etc.), and per-resource usage attribution
- Pluggable storage adapter framework (ClickHouse, PostgreSQL, custom)
- Custom measuring unit registration via API
- Usage query API for raw record retrieval with filtering and pagination
- Configurable retention policies
- Idempotency and deduplication for exactly-once semantics
- Backfill API for retroactive submission of historical usage data
- Late-arriving event handling with configurable grace period
- Per-tenant and per-source ingestion rate limiting with configurable overrides
- Priority-based load shedding under sustained overload

### 4.2 Out of Scope

- **Data Aggregation**: Multi-dimensional aggregation (by time windows, rollups, grouping) is the responsibility of downstream consumers. UC exposes raw usage records; consumers apply their own aggregation logic.
- **Reconciliation & Gap Detection**: Monitoring for data gaps, heartbeat tracking, watermark analysis, and cross-stage count reconciliation are handled by external observability/reconciliation systems. UC exposes metadata (event counts, timestamps per source) that external systems can consume for this purpose.
- **Report Generation**: Usage reports, dashboards, and visualizations are handled by Monitoring/Analytics systems; UC provides raw data access.
- **Rules & Exceptions**: Business rules, usage policies, threshold-based actions, and exception handling are the responsibility of downstream consumers.
- **Billing/Rating Logic**: Pricing calculation handled by downstream Billing System.
- **Invoice Generation**: Handled by Billing System.
- **Quota Enforcement Decisions**: Handled by Quota Enforcement System; UC provides data only.
- **Usage Prediction/Forecasting**: Deferred to future phase.
- **Multi-Region Replication**: Deferred to future phase.

## 5. Functional Requirements

### 5.1 Usage Ingestion

#### Usage Record Ingestion

- [ ] `p1` - **ID**: `cpt-cf-uc-fr-usage-ingestion`

The system **MUST** accept usage records via multiple transport mechanisms (Rust API, gRPC, HTTP), with SDK providing automatic batching for high-throughput scenarios (10,000+ events per second).

**Rationale**: Different usage sources have different integration and throughput needs; providing multiple transport options ensures all sources can efficiently emit data.
**Actors**: `cpt-cf-uc-actor-usage-source`, `cpt-cf-uc-actor-platform-developer`

#### Idempotency and Deduplication

- [ ] `p1` - **ID**: `cpt-cf-uc-fr-idempotency`

The system **MUST** support idempotency keys to ensure exactly-once processing, preventing duplicate records and incorrect aggregations.

**Rationale**: Network retries and batching can produce duplicate submissions; deduplication ensures billing accuracy.
**Actors**: `cpt-cf-uc-actor-usage-source`

#### Per-Record Extensible Metadata

- [ ] `p2` - **ID**: `cpt-cf-uc-fr-record-metadata`

The system **MUST** support an optional, extensible metadata field on each usage record, allowing usage sources to attach arbitrary key-value properties as a JSON object. The system **MUST** persist metadata as-is and return it in query results without interpretation. The system **MUST** enforce a configurable maximum size limit on the metadata field (default 8 KB per record) and **MUST** reject records exceeding the limit with an actionable error.

The system **MUST NOT** index, aggregate, or interpret metadata contents — metadata is opaque to the Usage Collector. Downstream consumers (billing, reporting, analytics) are responsible for extracting and processing metadata fields according to their own domain logic.

**Rationale**: Different usage sources need to attach context-specific properties to usage records (e.g., LLM model name, token type, request category, geographic region) that enable downstream reporting and analytics. Storing metadata per-record at ingestion time avoids the need to correlate usage records with external context stores and supports use cases like detailed LLM usage reporting and cost attribution by model. This follows the industry-standard pattern used by metering platforms (OpenMeter, Lago, Orb, Amberflo) where per-event properties are stored alongside the core measurement.
**Actors**: `cpt-cf-uc-actor-usage-source`, `cpt-cf-uc-actor-platform-developer`, `cpt-cf-uc-actor-billing-system`, `cpt-cf-uc-actor-monitoring-system`

### 5.2 Metric Semantics

#### Counter Metric Semantics

- [ ] `p1` - **ID**: `cpt-cf-uc-fr-counter-semantics`

The system **MUST** enforce counter semantics (monotonically increasing values), validate increments, compute deltas, and detect/reject counter violations.

**Rationale**: Counters represent cumulative totals (e.g., total API calls); violations indicate data corruption or misconfigured sources.
**Actors**: `cpt-cf-uc-actor-usage-source`

#### Gauge Metric Semantics

- [ ] `p1` - **ID**: `cpt-cf-uc-fr-gauge-semantics`

The system **MUST** support gauge metrics (point-in-time values) without monotonicity validation, storing values as-is.

**Rationale**: Gauges represent instantaneous measurements (e.g., current memory usage) that naturally fluctuate.
**Actors**: `cpt-cf-uc-actor-usage-source`

### 5.3 Attribution & Isolation

#### Tenant Attribution

- [ ] `p1` - **ID**: `cpt-cf-uc-fr-tenant-attribution`

The system **MUST** attribute all usage records to a tenant derived from security context, ensuring attribution is immutable and used for isolation.

**Rationale**: Accurate tenant attribution is the foundation for billing, quota enforcement, and data isolation.
**Actors**: `cpt-cf-uc-actor-usage-source`

#### Resource Attribution

- [ ] `p1` - **ID**: `cpt-cf-uc-fr-resource-attribution`

The system **MUST** support attributing usage to specific resource instances within a tenant, including resource ID, type, and lineage.

**Rationale**: Granular resource attribution enables per-resource billing and usage analysis.
**Actors**: `cpt-cf-uc-actor-usage-source`

#### Subject Attribution

- [ ] `p1` - **ID**: `cpt-cf-uc-fr-subject-attribution`

The system **MUST** support attributing usage to a subject within a tenant, identified by a `subject_id` and `subject_type` pair. Subject types include but are not limited to users and service accounts. Subject attribution **MUST** be derived from the authenticated security context when available, or explicitly provided by the usage source when reporting usage for batch operations or background jobs.

The system **MUST** support both direct subject attribution (subject X consumed resource Y) and indirect attribution (background job initiated by subject X consumed resource Y). Subject attribution is optional on a per-usage-record basis to accommodate system-level resource consumption that is not attributable to a specific subject.

**Rationale**: Per-subject attribution enables chargeback, detailed usage analytics, per-subject quota enforcement, and helps organizations understand which subjects (users, service accounts) are driving consumption. This is essential for multi-user tenants who need to allocate costs or enforce limits at the subject level, and for tracking consumption by automated service accounts alongside human users.
**Actors**: `cpt-cf-uc-actor-usage-source`, `cpt-cf-uc-actor-tenant-admin`, `cpt-cf-uc-actor-billing-system`

#### Tenant Isolation

- [ ] `p1` - **ID**: `cpt-cf-uc-fr-tenant-isolation`

The system **MUST** enforce strict tenant isolation ensuring usage data is never accessible across tenants, failing closed on authorization failures.

**Rationale**: Tenant data isolation is a security and compliance requirement.
**Actors**: `cpt-cf-uc-actor-platform-operator`, `cpt-cf-uc-actor-tenant-admin`, `cpt-cf-uc-actor-platform-developer`, `cpt-cf-uc-actor-usage-source`, `cpt-cf-uc-actor-billing-system`, `cpt-cf-uc-actor-quota-enforcement`, `cpt-cf-uc-actor-monitoring-system`, `cpt-cf-uc-actor-types-registry`, `cpt-cf-uc-actor-storage-backend`

#### Source Authorization

- [ ] `p1` - **ID**: `cpt-cf-uc-fr-source-authorization`

The system **MUST** identify the source of each usage record through the platform's authentication infrastructure and **MUST** validate that the source is authorized to report usage for the specific usage type being submitted. Source-to-usage-type bindings **MUST** be defined through the GTS type system, with permitted usage types registered in the Types Registry as part of the source's type definition. The system **MUST** reject usage records from sources that are not authorized for the given usage type.

**Rationale**: Without source-level authorization, any module or integration could report usage for resource types it does not own (e.g., a File Parser reporting LLM token usage), leading to inaccurate metering and potential billing manipulation.
**Actors**: `cpt-cf-uc-actor-usage-source`, `cpt-cf-uc-actor-types-registry`, `cpt-cf-uc-actor-platform-operator`

#### Attribution Authorization

- [ ] `p1` - **ID**: `cpt-cf-uc-fr-attribution-authorization`

The system **MUST** verify that the resource and subject referenced in a usage record are within the caller's SecurityContext reach. When a usage source attributes usage to a specific resource ID or subject (subject_id + subject_type), the system **MUST** validate that the authenticated caller is authorized to attribute usage to those entities. The system **MUST** reject usage records that reference resources or subjects outside the caller's SecurityContext scope, failing closed on authorization failures.

This requirement complements source authorization (`cpt-cf-uc-fr-source-authorization`): source authorization validates *what type* of usage a source may report, while attribution authorization validates *on whose behalf* (which subjects and resources) the usage is reported.

**Rationale**: Source authorization alone ensures a source can report a given usage type, but does not prevent a source from attributing usage to arbitrary subjects or resources beyond its organizational reach. Without attribution authorization, a compromised or misconfigured source could attribute usage to subjects or resources it does not manage, leading to incorrect billing, quota manipulation, and potential cross-boundary data pollution.
**Actors**: `cpt-cf-uc-actor-usage-source`, `cpt-cf-uc-actor-platform-operator`

### 5.4 Storage & Retention

#### Data Persistence Guarantees and Boundaries

- [ ] `p1` - **ID**: `cpt-cf-uc-fr-persistence-guarantees`

The system does **NOT** guarantee absolute persistence of all usage events under all conditions. The system provides limited persistence guarantees with explicit boundaries between guaranteed durability (post-ingestion) and acceptable data loss (pre-ingestion, overload scenarios).

**Persistence guarantee applies only AFTER successful ingestion acknowledgment** (`cpt-cf-uc-nfr-exactly-once`, `cpt-cf-uc-nfr-fault-tolerance`). **Before acknowledgment**, data loss is possible and acceptable under SDK buffer exhaustion, process termination, rate limiting, and priority-based load shedding (`cpt-cf-uc-fr-sdk-retry`, `cpt-cf-uc-fr-load-shedding`). All loss scenarios **MUST** be observable via metrics and alerts.

**Design Trade-off:** This design prioritizes system availability and ingestion performance over absolute durability. The Usage Collector remains available and responsive under overload conditions, accepting billing-critical usage data while shedding lower-priority events. The alternative—blocking all ingestion until buffers clear—would cause cascading failures in usage sources and block revenue-critical operations.

**Rationale**: Explicit acknowledgment of persistence boundaries prevents false assumptions about data durability and sets correct expectations for operators and downstream consumers.
**Actors**: `cpt-cf-uc-actor-platform-operator`, `cpt-cf-uc-actor-usage-source`, `cpt-cf-uc-actor-platform-developer`, `cpt-cf-uc-actor-billing-system`

#### Pluggable Storage Framework

- [ ] `p1` - **ID**: `cpt-cf-uc-fr-pluggable-storage`

The system **MUST** support multiple storage backends (ClickHouse, PostgreSQL, custom adapters) with configurable routing by usage type.

**Rationale**: Different usage types have different retention and performance needs; pluggable storage avoids lock-in.
**Actors**: `cpt-cf-uc-actor-platform-operator`, `cpt-cf-uc-actor-storage-backend`

#### Retention Policy Management

- [ ] `p1` - **ID**: `cpt-cf-uc-fr-retention-policies`

The system **MUST** support configurable retention policies (global, per-tenant, per-usage-type) with automated enforcement.

**Rationale**: Retention policies balance storage costs with compliance and operational needs.
**Actors**: `cpt-cf-uc-actor-platform-operator`

#### Storage Health Monitoring

- [ ] `p2` - **ID**: `cpt-cf-uc-fr-storage-health`

The system **MUST** monitor storage adapter health, buffer records during failures, retry with backoff, and alert on persistent issues.

**Rationale**: Storage failures must not result in data loss; buffering and retry ensure durability.
**Actors**: `cpt-cf-uc-actor-platform-operator`, `cpt-cf-uc-actor-storage-backend`

### 5.5 Querying

#### Usage Query API

- [ ] `p1` - **ID**: `cpt-cf-uc-fr-query-api`

The system **MUST** provide an API for querying raw usage records with filtering by time range, tenant, subject (subject_id and/or subject_type), resource, and usage type with cursor-based pagination. The API returns raw records; aggregation is the responsibility of downstream consumers.

**Rationale**: Downstream consumers need flexible access to raw usage data to apply their own aggregation, rating, and analysis logic. Per-subject filtering enables chargeback scenarios and per-subject usage analytics.
**Actors**: `cpt-cf-uc-actor-billing-system`, `cpt-cf-uc-actor-monitoring-system`, `cpt-cf-uc-actor-tenant-admin`

#### Stable Query Result Ordering

- [ ] `p1` - **ID**: `cpt-cf-uc-fr-stable-ordering`

The system **MUST** return query results in a stable, deterministic order across all API endpoints. The ordering **MUST** be consistent across pagination requests to prevent records from being missed or duplicated when combined with cursor-based pagination.

**Rationale**: Stable ordering is essential for cursor-based pagination to work correctly. Without deterministic ordering, cursors cannot reliably mark positions in the result set, leading to missing or duplicate records across pages. This is critical for billing accuracy where downstream consumers must process complete, non-duplicated datasets.
**Actors**: `cpt-cf-uc-actor-billing-system`, `cpt-cf-uc-actor-monitoring-system`, `cpt-cf-uc-actor-tenant-admin`

#### Cursor-Based Pagination

- [ ] `p1` - **ID**: `cpt-cf-uc-fr-cursor-pagination`

The system **MUST** implement cursor-based pagination for all query APIs. Each page response **MUST** include an opaque cursor token that marks the position after the last record in the current page. Clients **MUST** pass this cursor in subsequent requests to retrieve the next page. Cursors **MUST** remain valid for at least 24 hours after issuance.

**Rationale**: Offset-based pagination (`LIMIT/OFFSET`) is unreliable when data is being inserted concurrently — new insertions shift offsets, causing records to be skipped or duplicated across pages. Cursor-based pagination provides stable position markers that are unaffected by concurrent writes. This is essential for billing systems that must process complete usage datasets without gaps or duplicates.
**Actors**: `cpt-cf-uc-actor-billing-system`, `cpt-cf-uc-actor-monitoring-system`, `cpt-cf-uc-actor-tenant-admin`, `cpt-cf-uc-actor-platform-developer`

#### Snapshot Read Consistency

- [ ] `p3` - **ID**: `cpt-cf-uc-fr-snapshot-reads`

The system **MUST** support snapshot reads, allowing clients to query data with a consistent point-in-time view. When a query is initiated with snapshot isolation, all subsequent pagination requests in that query session **MUST** see the dataset as it existed at the snapshot timestamp, regardless of concurrent insertions, updates, or backfill operations.

**Rationale**: Even with cursor-based pagination, concurrent data modifications (late-arriving events, backfill operations) can cause inconsistencies across paginated queries. Snapshot reads provide the strongest consistency guarantee: a billing system paginating through a month of data sees the exact same records on every page, as they existed when the query started. This is marked p3 because cursor-based pagination with stable ordering provides sufficient consistency for most use cases, but snapshot isolation is valuable when absolute consistency is required for auditing or financial reconciliation.
**Actors**: `cpt-cf-uc-actor-billing-system`, `cpt-cf-uc-actor-monitoring-system`, `cpt-cf-uc-actor-tenant-admin`

### 5.6 Backfill & Amendment

#### Late-Arriving Event Handling

- [ ] `p2` - **ID**: `cpt-cf-uc-fr-late-events`

The system **MUST** accept usage events with timestamps within a configurable grace period (default 24 hours, configurable per tenant and per usage type) via the standard ingestion path, applying normal deduplication and schema validation.

**Rationale**: In distributed systems, clock skew, batch processing delays, and asynchronous architectures cause events to routinely arrive after their actual timestamp. A grace period allows these events to be processed without requiring explicit backfill operations.
**Actors**: `cpt-cf-uc-actor-usage-source`, `cpt-cf-uc-actor-platform-operator`

#### Backfill API

- [ ] `p2` - **ID**: `cpt-cf-uc-fr-backfill-api`

The system **MUST** provide a backfill API that allows operators to retroactively submit historical usage data for a specific time range (scoped to a single tenant and usage type). The backfill operation **MUST** replace all existing events in the specified time range with the provided replacement events, ensuring atomic replacement to prevent partial or inconsistent states.

The backfill API **MUST** be isolated from the real-time ingestion path with independent rate limits and lower processing priority to prevent backfill operations from degrading real-time ingestion performance.

The system **MUST** support both blocking and non-blocking backfill modes:

- **Blocking mode**: Real-time ingestion for the affected time range is temporarily unavailable during the backfill operation. Real-time events that fall within the backfill range are rejected with conflict errors, allowing clients to retry after the backfill completes. This mode provides the strongest consistency guarantees and ensures the backfilled data precisely replaces the target range.
- **Non-blocking mode**: Real-time ingestion continues uninterrupted during the backfill operation. The system resolves conflicts between concurrent real-time events and backfill operations according to a configurable conflict resolution policy. This mode prioritizes availability of real-time ingestion over strict replacement semantics.

The choice of mode **MUST** be configurable per backfill operation, allowing operators to select the appropriate trade-off based on the severity and business impact of the gap being corrected.

**Rationale**: When usage data is lost due to outages, pipeline failures, or misconfigured sources, operators need a mechanism to retroactively submit corrected data for an entire time range. Different scenarios have different criticality: correcting a minor analytics gap may tolerate concurrent real-time events and should not block critical usage reporting, while correcting billing-critical data may require strict replacement semantics with temporary ingestion blocking. Supporting both modes ensures backfill operations never prevent critical real-time usage reporting when that is the priority, while still providing strong consistency guarantees when needed.
**Actors**: `cpt-cf-uc-actor-platform-operator`, `cpt-cf-uc-actor-usage-source`

#### Individual Event Amendment

- [ ] `p2` - **ID**: `cpt-cf-uc-fr-event-amendment`

The system **MUST** support amending individual usage events (updating properties except tenant ID and timestamp) and deprecating individual events (marking them as inactive while retaining them for audit). Downstream consumers **MUST** be able to distinguish active from deprecated records when querying.

**Interaction with backfill**: If a backfill operation targets a time range that contains previously amended events, the backfill **MUST** archive the amended events along with all other events in that range. Amendment history is not preserved — the backfill's replacement events become the sole active record. This means backfill unconditionally supersedes any prior amendments within its range, keeping the correction model simple: amendments are for surgical fixes to individual events, while backfill is a wholesale replacement that starts from a clean slate.

**Rationale**: Not all corrections require full timeframe backfill. Individual event amendments handle cases like incorrect resource attribution or value errors on specific events.
**Actors**: `cpt-cf-uc-actor-platform-operator`

#### Backfill Time Boundaries

- [ ] `p3` - **ID**: `cpt-cf-uc-fr-backfill-boundaries`

The system **MUST** enforce configurable time boundaries for backfill operations: a maximum backfill window (default 90 days) beyond which backfill requests are rejected, and a future timestamp tolerance (default 5 minutes) to account for clock drift. Backfill requests exceeding the maximum window **MUST** require elevated authorization.

**Rationale**: Unbounded backfill creates risks for data integrity and billing accuracy. Time boundaries constrain the blast radius of backfill operations while allowing legitimate corrections. Different limits for automated retry (grace period) vs. operator-initiated backfill (90 days) match different use cases. The 5-minute future tolerance follows Stripe's pattern for handling clock drift in distributed systems.
**Actors**: `cpt-cf-uc-actor-platform-operator`

#### Backfill Event Archival

- [ ] `p2` - **ID**: `cpt-cf-uc-fr-backfill-archival`

When a backfill operation replaces events in a time range, the system **MUST** archive (not delete) the replaced events. Archived events **MUST** remain queryable for audit purposes but **MUST** be clearly distinguishable from active records so that downstream consumers can exclude them from their processing.

**Rationale**: Permanent deletion of replaced events destroys audit trail and makes it impossible to investigate billing disputes or reconstruct historical state.
**Actors**: `cpt-cf-uc-actor-platform-operator`

#### Backfill Audit Trail

- [ ] `p2` - **ID**: `cpt-cf-uc-fr-backfill-audit`

Every backfill operation **MUST** produce an immutable audit record containing: operator identity, initiation timestamp, affected time range, affected tenant(s), number of events added/replaced/deprecated, reason or justification, and whether the operation affected an already-invoiced period.

**Rationale**: Backfill operations are high-risk changes to billing-critical data. Comprehensive audit records are essential for dispute resolution, compliance, and operational visibility.
**Actors**: `cpt-cf-uc-actor-platform-operator`

#### Ledger Metadata Exposure

- [ ] `p3` - **ID**: `cpt-cf-uc-fr-ledger-metadata`

The system **MUST** expose per-source and per-tenant metadata — including event counts, latest event timestamps (watermarks), and ingestion statistics — via API, enabling external reconciliation and observability systems to detect gaps and perform integrity checks.

**Rationale**: While reconciliation logic is out of scope for the usage ledger, exposing the raw metadata needed for gap detection enables external systems to build reconciliation workflows. This keeps the ledger focused while not blocking operational integrity monitoring.
**Actors**: `cpt-cf-uc-actor-platform-operator`, `cpt-cf-uc-actor-monitoring-system`

### 5.7 Type System

#### Usage Type Validation

- [ ] `p1` - **ID**: `cpt-cf-uc-fr-type-validation`

The system **MUST** validate all usage records against registered type schemas, rejecting invalid records with actionable error messages.

**Rationale**: Schema validation prevents corrupt or malformed data from entering the system.
**Actors**: `cpt-cf-uc-actor-usage-source`, `cpt-cf-uc-actor-types-registry`

#### Custom Unit Registration

- [ ] `p1` - **ID**: `cpt-cf-uc-fr-custom-units`

The system **MUST** allow registration of custom measuring units via API without code changes.

Primary use cases: AI/LLM token metering (input/output tokens, custom credit units), compute metering (vCPU-hours, memory-GB-hours, GPU-hours), API request metering (calls by tenant and endpoint), storage metering (GB-hours across tiers), network transfer (bytes ingress/egress).

When a custom measuring unit is registered, the platform operator **MUST** also register the source-to-usage-type binding in the Types Registry, declaring which sources are permitted to emit records of this type. The system **MUST NOT** accept usage records for a type that has no registered source bindings.

**Rationale**: New resource types (AI tokens, GPU-hours) must be meterable without service redeployment. Source-to-usage-type bindings ensure that only authorized sources can emit records for each unit.
**Actors**: `cpt-cf-uc-actor-platform-operator`, `cpt-cf-uc-actor-types-registry`

### 5.8 Rate Limiting

#### Per-Tenant Ingestion Rate Limiting

- [ ] `p2` - **ID**: `cpt-cf-uc-fr-tenant-rate-limit`

The system **MUST** enforce per-tenant ingestion rate limits with independently configurable sustained rate (events per second) and burst size parameters. Requests exceeding the rate limit **MUST** be rejected with an appropriate rate limit error.

**Rationale**: Without per-tenant rate limiting, a single misbehaving or high-volume tenant can exhaust ingestion capacity and degrade service for all other tenants. Burst tolerance is required because usage event emission is inherently bursty (e.g., a batch job completing and emitting thousands of records at once).
**Actors**: `cpt-cf-uc-actor-usage-source`, `cpt-cf-uc-actor-platform-operator`

#### Per-Source Rate Limiting

- [ ] `p2` - **ID**: `cpt-cf-uc-fr-source-rate-limit`

The system **MUST** enforce per-source rate limits within each tenant, preventing a single usage source (e.g., a misconfigured LLM Gateway) from consuming the tenant's entire ingestion quota. Per-source limits **MUST** be configurable independently of the tenant-level limit.

**Rationale**: Tenant-level rate limits alone do not prevent a single noisy source from starving other sources within the same tenant. Per-source limits provide fault isolation within a tenant's service portfolio.
**Actors**: `cpt-cf-uc-actor-usage-source`, `cpt-cf-uc-actor-platform-operator`

#### Multi-Dimensional Rate Limits

- [ ] `p3` - **ID**: `cpt-cf-uc-fr-multi-dimensional-limits`

The system **MUST** enforce rate limits across multiple dimensions simultaneously: events per second, bytes per second, maximum batch size (events per request), and maximum record size (bytes per event). All dimensions **MUST** pass for a request to be accepted.

**Rationale**: Single-dimension rate limits are insufficient; a tenant could comply with events/sec limits while submitting oversized payloads that exhaust network or storage bandwidth. Multi-dimensional limits protect all resource types (CPU for event processing, network for payload transfer, storage for persistence).
**Actors**: `cpt-cf-uc-actor-usage-source`, `cpt-cf-uc-actor-platform-operator`

#### Rate Limit Configuration and Overrides

- [ ] `p2` - **ID**: `cpt-cf-uc-fr-rate-limit-config`

The system **MUST** support rate limit configuration with system-wide defaults and per-tenant overrides. Per-tenant overrides **MUST** be hot-reloadable without service restart. Unspecified fields in overrides **MUST** inherit from the system defaults.

**Rationale**: Different tenants have different throughput needs based on their workload profile. Hot-reloadable overrides enable operators to respond to capacity issues or tenant requests without service disruption.
**Actors**: `cpt-cf-uc-actor-platform-operator`

#### Rate Limit Response Format

- [ ] `p2` - **ID**: `cpt-cf-uc-fr-rate-limit-response`

When rate limits are exceeded, the system **MUST** respond with a rate limit error and include metadata indicating when the client can retry. The system **MUST** provide rate limit status information on all API responses to enable clients to monitor their quota consumption. Response metadata **MUST** include:

- **`retry_after_ms`**: Integer milliseconds the client should wait before retrying (provided on rate limit errors).
- **`rate_limit_limit`**: Integer representing the total request allowance for the current rate limit window.
- **`rate_limit_remaining`**: Integer representing the number of requests remaining in the current rate limit window.
- **`rate_limit_reset`**: Unix epoch timestamp (seconds since 1970-01-01T00:00:00Z) in UTC indicating when the current rate limit window resets.

For HTTP transport specifically, these **MUST** be provided as response headers: `Retry-After` (in seconds), `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and `X-RateLimit-Reset`, with HTTP 429 status. For gRPC transport, these **MUST** be provided in response metadata. For Rust API, these **MUST** be provided in error/response structures.

All time-related values are in UTC. Clients **MUST** treat missing or unparseable values conservatively (i.e., assume the limit is exhausted and apply a default backoff).

**Rationale**: Rate limit metadata enables clients to track quota consumption and schedule retries, reducing wasted requests against an already-exhausted quota. Transport-specific encoding ensures the same information is available regardless of how clients integrate.
**Actors**: `cpt-cf-uc-actor-usage-source`, `cpt-cf-uc-actor-platform-developer`

#### SDK Retry and Buffering on Rate Limit

- [ ] `p2` - **ID**: `cpt-cf-uc-fr-sdk-retry`

The SDK **MUST** buffer usage events in a bounded in-memory queue and retry with exponential backoff and jitter on rate limit errors and backfill conflict errors, honoring the `retry_after_ms` field when present. Under buffer exhaustion or sustained overload, the SDK will drop oldest events and **MUST** emit loss metrics (count, rate, usage type, timestamp) and **MUST** trigger alerts when drop rate exceeds configurable thresholds. The SDK **MUST NOT** block the calling service due to rate limiting.

**Loss Conditions**: Data loss at the SDK level is acceptable under the following conditions:
- Buffer exhaustion: The in-memory queue reaches capacity and new events arrive before older events can be successfully retried
- Sustained overload: Rate limiting persists beyond the SDK's retry window and buffer capacity
- Process termination: The SDK process crashes or is terminated before buffered events are flushed

When loss occurs, the SDK **MUST** expose metrics including: `usage_events_dropped_total` (counter), `usage_events_dropped_rate` (gauge), `usage_buffer_utilization` (gauge), tagged by `usage_type`, `tenant_id`, and `drop_reason`. Operators **MUST** configure alerts on sustained drops (e.g., >1% drop rate over 5 minutes).

**Rationale**: Usage sources generate events regardless of collector availability. The SDK must absorb temporary rate limiting transparently, retrying without burdening the caller. Exponential backoff with jitter prevents synchronized retry bursts across sources. Non-blocking behavior is critical because usage emission must not degrade the source service's primary function. Explicit loss conditions with observability enable operators to detect and remediate issues (e.g., increase rate limits, scale collectors, reduce event volume) rather than silently losing data.
**Actors**: `cpt-cf-uc-actor-usage-source`, `cpt-cf-uc-actor-platform-developer`, `cpt-cf-uc-actor-platform-operator`

#### Priority-Based Load Shedding

- [ ] `p2` - **ID**: `cpt-cf-uc-fr-load-shedding`

The system **MUST** support priority classification of usage event types (e.g., billing-critical counters vs. analytics metrics) and, when operating under sustained overload beyond rate limits, **MUST** preferentially accept higher-priority events while shedding lower-priority ones. The priority classification **MUST** be configurable per usage type.

**Rationale**: Under extreme load, indiscriminate rejection causes billing-critical data loss. Priority-based load shedding ensures that the most business-critical usage data (which affects revenue accuracy) is preserved even when the system cannot accept all traffic.
**Actors**: `cpt-cf-uc-actor-platform-operator`, `cpt-cf-uc-actor-usage-source`

#### Rate Limit Observability

- [ ] `p2` - **ID**: `cpt-cf-uc-fr-rate-limit-observability`

The system **MUST** expose per-tenant and per-source rate limit consumption as metrics (current usage vs. limit, rejection counts, throttle duration) for operator dashboards. The system **MUST** emit alerts when tenants approach configured warning thresholds (e.g., 75%, 90% of capacity).

**Rationale**: Operators need visibility into rate limit utilization to proactively adjust limits before tenants experience rejections. Approaching-limit alerts enable capacity planning.
**Actors**: `cpt-cf-uc-actor-platform-operator`, `cpt-cf-uc-actor-monitoring-system`

## 6. Non-Functional Requirements

### 6.1 Module-Specific NFRs

#### High Availability

- [ ] `p1` - **ID**: `cpt-cf-uc-nfr-availability`

The system **MUST** maintain 99.95% monthly availability for usage collection endpoints.

**Threshold**: 99.95% uptime per calendar month
**Rationale**: Usage collection is on the critical path for all billable operations.

#### Ingestion Throughput

- [ ] `p1` - **ID**: `cpt-cf-uc-nfr-throughput`

The system **MUST** support sustained ingestion of at least 600,000 usage records per minute (10,000 events per second) under normal operation.

**Threshold**: >= 600,000 records/minute (10,000 events/sec) sustained
**Rationale**: High-volume services (LLM Gateway, API Gateway) generate significant event throughput.

#### Ingestion Latency

- [ ] `p1` - **ID**: `cpt-cf-uc-nfr-ingestion-latency`

The system **MUST** complete usage record ingestion within 200ms (p95) under normal load.

**Threshold**: p95 <= 200ms
**Rationale**: Low ingestion latency prevents blocking in usage source services.

#### Query/Retention Isolation

- [ ] `p2` - **ID**: `cpt-cf-uc-nfr-query-isolation`

The system **MUST** ensure that query operations and retention enforcement jobs do not degrade ingestion latency. Query and retention workloads **MUST** be isolated from the ingestion path such that heavy query load or retention job execution maintains ingestion p95 latency within the `cpt-cf-uc-nfr-ingestion-latency` threshold (200ms).

**Threshold**: Ingestion p95 latency remains ≤200ms during concurrent query/retention operations
**Rationale**: Query and retention are read-heavy and batch-processing workloads that can compete for storage resources with the latency-sensitive ingestion path. Without isolation, billing report generation or retention cleanup could degrade ingestion performance, blocking usage sources.

#### Query Latency

- [ ] `p1` - **ID**: `cpt-cf-uc-nfr-query-latency`

The system **MUST** complete usage queries for 30-day ranges within 500ms (p95) under normal load.

**Threshold**: p95 <= 500ms for 30-day range queries
**Rationale**: Billing and quota enforcement systems require fast query responses.

#### Exactly-Once Semantics

- [ ] `p1` - **ID**: `cpt-cf-uc-nfr-exactly-once`

The system **MUST** guarantee exactly-once processing for all usage records successfully ingested by the API; zero usage records lost or duplicated after ingestion under normal operation. A record is considered **successfully ingested** only once the collector has **durably persisted** the record to storage **and** returned an acknowledgment (HTTP 2xx or gRPC OK) to the SDK. If the collector acknowledges a batch before durable persistence completes and a crash occurs, those records are **NOT** covered by the exactly-once guarantee and must be treated as SDK-level failures per `cpt-cf-uc-fr-sdk-retry`. This guarantee applies to the service layer (collector, storage adapters, query path) but does **NOT** apply to SDK-level drops caused by buffer exhaustion or sustained overload as defined in `cpt-cf-uc-fr-sdk-retry`.

**Threshold**: Zero data loss or duplication after successful API ingestion under normal operation
**Rationale**: Duplicate or missing records directly impact billing accuracy. The exactly-once guarantee begins when the collector durably persists a record and returns success (HTTP 2xx/gRPC OK); acknowledged records are guaranteed to be queryable by downstream consumers (billing, quota enforcement). SDK-level drops before acknowledgment are observable via metrics and alerts, enabling operators to address capacity or configuration issues.

#### Audit Trail

- [ ] `p1` - **ID**: `cpt-cf-uc-nfr-audit-trail`

The system **MUST** preserve immutable audit records for all usage data including source, timestamps, and any corrections.

**Threshold**: 100% of usage operations audited
**Rationale**: Audit trails are required for billing disputes and compliance.

#### Authentication Required

- [ ] `p1` - **ID**: `cpt-cf-uc-nfr-authentication`

The system **MUST** require authentication (OAuth 2.0, mTLS, or API key) for all API operations.

**Threshold**: Zero unauthenticated API access
**Rationale**: Usage data is sensitive and must be protected.

#### Authorization Enforcement

- [ ] `p1` - **ID**: `cpt-cf-uc-nfr-authorization`

The system **MUST** enforce authorization for read/write operations based on tenant context and usage type permissions.

**Threshold**: Zero unauthorized data access
**Rationale**: Authorization prevents unauthorized usage data manipulation.

#### Horizontal Scalability

- [ ] `p2` - **ID**: `cpt-cf-uc-nfr-scalability`

The system **MUST** scale horizontally to handle increased load without architectural changes.

**Threshold**: Linear throughput scaling with added instances
**Rationale**: Usage volume grows with platform adoption.

#### Storage Fault Tolerance

- [ ] `p2` - **ID**: `cpt-cf-uc-nfr-fault-tolerance`

The system **MUST** buffer usage records during storage backend failures and recover without data loss. This guarantee applies to records successfully accepted by the ingestion API; SDK-level drops due to buffer exhaustion before ingestion are covered by `cpt-cf-uc-fr-sdk-retry`.

**Threshold**: Zero data loss during storage backend failures for records accepted by the collector
**Rationale**: Storage outages must not result in lost usage data after successful ingestion. The service layer must buffer and retry to ensure durability.

#### Configurable Retention

- [ ] `p1` - **ID**: `cpt-cf-uc-nfr-retention`

The system **MUST** support retention periods from 7 days to 7 years depending on usage type and compliance requirements.

**Threshold**: Configurable retention from 7 days to 7 years
**Rationale**: Different usage types have different compliance and operational retention needs.

#### Graceful Degradation

- [ ] `p1` - **ID**: `cpt-cf-uc-nfr-graceful-degradation`

The system **MUST** continue accepting usage records even if downstream consumers (billing, monitoring) are unavailable.

**Threshold**: Zero ingestion failures due to downstream consumer unavailability
**Rationale**: Usage collection must not be blocked by consumer outages.

## 7. Public Library Interfaces

### 7.1 Public API Surface

#### Usage Ingestion SDK

- [ ] `p1` - **ID**: `cpt-cf-uc-interface-sdk`

**Type**: Client library
**Stability**: stable
**Description**: Client-side SDK with automatic batching for high-throughput usage emission. Primary ingestion path for platform services. Provides Rust API (in-process) and gRPC transports.
**Breaking Change Policy**: Major version bump required

#### HTTP API

- [ ] `p1` - **ID**: `cpt-cf-uc-interface-http-api`

**Type**: HTTP/REST API
**Stability**: stable
**Description**: HTTP API for usage record ingestion, querying, and administration. Supports external integrations and simple client scenarios.
**Breaking Change Policy**: Major version bump required

### 7.2 External Integration Contracts

#### Storage Adapter Contract

- [ ] `p1` - **ID**: `cpt-cf-uc-contract-storage-adapter`

**Direction**: required from client
**Protocol/Format**: Rust trait / plugin interface
**Compatibility**: Backward-compatible within major version; adapters implement a defined trait for read/write operations.

#### Types Registry Contract

- [ ] `p1` - **ID**: `cpt-cf-uc-contract-types-registry`

**Direction**: required from client
**Protocol/Format**: Internal API
**Compatibility**: Schema validation contract; UC depends on Types Registry for unit and type definitions.

## 8. Use Cases

### UC: High-Volume Usage Emission via SDK

- [ ] `p1` - **ID**: `cpt-cf-uc-usecase-sdk-emission`

**Actor**: `cpt-cf-uc-actor-usage-source`, `cpt-cf-uc-actor-platform-developer`

**Preconditions**:
- Usage type registered in Types Registry

**Main Flow**:
1. Service calls SDK to record usage event
2. SDK queues event in memory
3. SDK batches events (by count or time threshold)
4. SDK sends batch to UC
5. UC validates each record against type schema
6. UC persists to configured storage backend
7. SDK receives acknowledgment

**Postconditions**:
- Usage records persisted with tenant/resource attribution; available for query by downstream consumers

### UC: Configure Custom Measuring Unit

- [ ] `p1` - **ID**: `cpt-cf-uc-usecase-custom-unit`

**Actor**: `cpt-cf-uc-actor-platform-operator`

**Preconditions**:
- Unit name is unique

**Main Flow**:
1. Operator defines unit schema (name, type: counter/gauge, base unit)
2. Operator submits via API
3. UC validates unit definition
4. UC registers unit with Types Registry
5. UC confirms registration

**Postconditions**:
- New unit immediately available for usage collection; sources can emit records with new unit type

### UC: Query Tenant Usage for Billing

- [ ] `p1` - **ID**: `cpt-cf-uc-usecase-billing-query`

**Actor**: `cpt-cf-uc-actor-billing-system`

**Preconditions**:
- Billing period defined by Billing System

**Main Flow**:
1. Billing System queries usage API for billing period (time range, tenant, optional subject filter, usage types)
2. UC retrieves raw usage records matching the filter criteria in stable order
3. UC returns first page with cursor token
4. Billing System processes page and requests next page using cursor
5. Steps 3-4 repeat until all pages retrieved
6. Billing System aggregates and processes complete record set for rating (with optional per-subject breakdown)

**Postconditions**:
- Billing System has accurate, deduplicated raw usage records for its own aggregation and invoice generation
- No records missed or duplicated due to concurrent insertions during pagination
- If subject filtering was applied, billing can generate per-subject chargeback reports

### UC: Add New Storage Backend

- [ ] `p1` - **ID**: `cpt-cf-uc-usecase-add-storage`

**Actor**: `cpt-cf-uc-actor-platform-operator`

**Preconditions**:
- Storage adapter plugin implemented
- Backend accessible

**Main Flow**:
1. Operator deploys adapter plugin
2. Operator configures adapter connection in UC
3. Operator defines routing rules (which usage types to which backend)
4. UC discovers and validates adapter
5. UC begins routing matching usage types to new backend

**Postconditions**:
- New storage backend active; usage routed per configuration; existing data unaffected

### UC: Backfill Usage After Outage

- [ ] `p2` - **ID**: `cpt-cf-uc-usecase-backfill-after-outage`

**Actor**: `cpt-cf-uc-actor-platform-operator`

**Preconditions**:
- Gap detected (by external reconciliation system or operator investigation)
- Replacement usage data available from secondary source (infrastructure metrics, API gateway logs, or source service replay)

**Main Flow**:
1. Operator identifies gap (via external reconciliation system alerts or manual investigation)
2. Operator prepares replacement events from secondary source
3. Operator submits backfill request specifying time range, tenant, and replacement events
4. UC validates time range is within backfill window
5. UC archives existing events in the time range (if any)
6. UC validates and persists replacement events with backfill idempotency namespace
7. UC creates audit record for the backfill operation

**Postconditions**:
- Gap filled with corrected data; archived events retained for audit; downstream consumers can query corrected raw records

### UC: Query Tenant Usage Data

- [ ] `p1` - **ID**: `cpt-cf-uc-usecase-usage-query`

**Actor**: `cpt-cf-uc-actor-tenant-admin`

**Main Flow**:
1. Administrator queries usage API for a time period with optional subject and resource filters
2. UC retrieves raw usage records scoped to the tenant only in stable order
3. UC returns paginated raw records with cursor tokens, filtered by type, subject, and resource
4. Administrator retrieves all pages using cursors
5. Administrator (or downstream reporting system) processes the complete data

**Postconditions**:
- Administrator receives only their tenant's raw usage records; no cross-tenant data exposure
- Paginated results are consistent without gaps or duplicates
- Administrator can analyze usage by specific subjects (users, service accounts) within their tenant

## 9. Acceptance Criteria

- [ ] All billable platform services emit usage through UC
- [ ] Downstream consumers (billing, monitoring) querying the same time range receive identical raw records
- [ ] Usage records can be attributed to specific subjects (users, service accounts) within a tenant and queried by subject_id and/or subject_type
- [ ] Custom unit registration completes in less than 5 minutes without code changes
- [ ] High-volume services can emit 10,000+ events per second without blocking
- [ ] 99.95%+ monthly availability maintained
- [ ] Zero data loss or duplication after successful ingestion (usage records accepted by the collector API are persisted exactly once)
- [ ] SDK buffer drops (pre-ingestion) are observable via metrics (`usage_events_dropped_total`, `usage_buffer_utilization`) and trigger alerts when drop rate exceeds configurable thresholds (as defined in `cpt-cf-uc-fr-sdk-retry`)
- [ ] Backfill API can restore missing usage data for any time range within the backfill window
- [ ] Zero data permanently deleted during backfill operations (archive-only)
- [ ] Ledger metadata (event counts, watermarks) is available via API for external reconciliation systems
- [ ] A single tenant exceeding its rate limit does not degrade ingestion latency for other tenants
- [ ] Rate limit configuration changes take effect without service restart
- [ ] Query results are returned in stable, deterministic order across all pagination requests
- [ ] Cursor-based pagination prevents record gaps or duplicates during concurrent insertions
- [ ] Cursors remain valid for at least 24 hours after issuance
- [ ] Usage records can carry optional extensible metadata (JSON object) that is persisted as-is and returned in query results
- [ ] Records with metadata exceeding the configured size limit (default 8 KB) are rejected with an actionable error

## 10. Dependencies

| Dependency | Description | Criticality |
|------------|-------------|-------------|
| Storage Backend (PostgreSQL or ClickHouse) | At least one storage backend available in the platform | p1 |
| Types Registry | Schema validation for usage types, custom measuring units, and source-to-usage-type authorization bindings | p1 |
| Platform Auth Infrastructure | Authentication and authorization infrastructure (OAuth 2.0, mTLS) | p1 |

## 11. Assumptions

- At least one storage backend (PostgreSQL or ClickHouse) is available in the platform
- Types Registry service is available for schema validation
- Platform authentication/authorization infrastructure exists
- Consumers (Billing, Quota Enforcement, Monitoring) integrate via query API

## 12. Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Storage backend unavailability | Usage records lost during outage | Buffering with retry and backoff; multi-backend routing |
| High ingestion volume exceeds capacity | Usage sources rejected; delayed billing data | Horizontal scaling; SDK-side batching and buffering; per-tenant rate limiting |
| Schema evolution breaks existing sources | Usage sources fail validation after type changes | Backward-compatible schema evolution; versioned type schemas |
| Cross-tenant data leakage | Security and compliance violation | Fail-closed authorization; tenant isolation at all layers |
| Insufficient ledger metadata | External reconciliation systems cannot detect gaps if UC does not expose adequate metadata | Expose per-source event counts, watermarks, and ingestion statistics via dedicated API |
| Backfill data quality | Incorrect replacement data worsens the gap instead of fixing it | Full schema validation on backfill events; archive-not-delete allows rollback; audit trail enables investigation |
| Noisy-neighbor ingestion | A single tenant or source exhausts ingestion capacity, degrading service for all tenants | Hierarchical rate limiting (global, per-tenant, per-source); priority-based load shedding |
| Rate limit misconfiguration | Limits set too low cause SDK buffer exhaustion and event drops (as defined in `cpt-cf-uc-fr-sdk-retry`); limits set too high provide no overload protection | System defaults with per-tenant hot-reloadable overrides; rate limit observability and approaching-limit alerts; SDK drop metrics and alerts enable operators to detect and remediate capacity issues |
| Retry storms after rate limiting | Synchronized client retries after a rate limit event amplify load | SDK enforces exponential backoff with jitter; Retry-After headers provide explicit retry guidance |

## 13. Open Questions

- Specific CloudEvents format adoption for usage record schema
- Retention policy enforcement mechanism (TTL vs. scheduled cleanup)
- Exact SDK batching defaults (count threshold, time threshold)
- Default rate limit values for system-wide defaults and per-source defaults
- Priority classification of existing usage types for load shedding (which types are billing-critical P0 vs. analytics P2)
- Specific metadata fields and API shape for ledger metadata exposure (to support external reconciliation)
- Cursor encoding format (opaque token vs. base64-encoded position metadata)
- Snapshot read implementation strategy (storage-native isolation vs. versioning metadata)
- Cursor and snapshot expiration handling (graceful error responses when expired)
- Federated-query merge strategy for operational/historical storage split (`cpt-cf-uc-principle-storage-separation`): how to guarantee stable ordering (`cpt-cf-uc-fr-stable-ordering`) when merging results from both tiers (e.g., merge by event timestamp + deterministic tiebreaker, or global sequence/watermark)
- Cursor semantics for federated queries: cursor token payload format to support pagination across operational and historical storage tiers (must include source shard identifiers, per-source offsets, and global watermark)
- Migration coordination for federated cursors: how cursors spanning an operational-to-historical migration are handled (e.g., migration watermark in cursor to consult both adapters, or invalidation error requiring cursor refresh)
