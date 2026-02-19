# PRD - Mini Chat

## 1. Overview

### 1.1 Purpose

Mini Chat is a multi-tenant AI chat module that provides users with a conversational interface backed by a large language model. Users can send messages, receive streamed responses in real time, upload documents, and ask questions about uploaded content. The module enforces strict tenant isolation, usage-based cost controls, and emits audit events.

Parent tenant / MSP administrators MUST NOT have access to chat content. Admin visibility is limited to aggregated usage and operational metrics.

### 1.2 Background / Problem Statement

The platform requires an integrated AI assistant that gives users the ability to have multi-turn conversations with an LLM and ground those conversations in their own documents. Without this capability, users must rely on external tools (ChatGPT, etc.), which creates data governance risks, lacks integration with platform access controls, and provides no visibility into aggregated usage and operational metrics for tenant administrators.

Current gaps: no native chat experience within the platform; no way to query uploaded documents via LLM; no per-user usage tracking or quota enforcement for AI features; no audit events emitted for AI interactions.

### 1.3 Goals (Business Outcomes)

- Provide a stable, production-ready AI chat with real-time streaming and persistent conversation history
- Enable document-aware conversations: users upload files and ask questions grounded in document content
- Guarantee tenant data isolation and enforce access control via `ai_chat` license feature
- Control operational costs through per-user quotas, token budgets, and tool-call limits
- Emit audit events to platform `audit_service` for completed chat turns and policy decisions (one structured event per turn; see `cpt-cf-mini-chat-fr-audit`)

### 1.4 Glossary

| Term | Definition |
|------|------------|
| Chat | A persistent conversation between a user and the AI assistant |
| Message | A single turn within a chat (user input or assistant response) |
| Attachment | A document file uploaded to a chat for question answering |
| Thread Summary | A compressed representation of older messages, used to keep long conversations within token limits |
| Vector Store | A provider-hosted index of document embeddings (OpenAI or Azure OpenAI), scoped per tenant, used for document search |
| File Search | An LLM tool call that retrieves relevant excerpts from uploaded documents |
| Token Budget | The maximum number of input/output tokens allowed per request |
| Temporary Chat | A chat marked for automatic deletion after 24 hours |
| OAGW | Outbound API Gateway - platform service that handles external API calls and credential injection |

## 2. Actors

### 2.1 Human Actors

#### Chat User

**ID**: `cpt-cf-mini-chat-actor-chat-user`

**Role**: End user who creates chats, sends messages, uploads documents, and receives AI responses. Belongs to a tenant and is subject to that tenant's license and quota policies.
**Needs**: Real-time conversational AI; ability to ask questions about uploaded documents; persistent chat history; clear feedback when quotas are exceeded.

### 2.2 System Actors

#### OpenAI / Azure OpenAI

**ID**: `cpt-cf-mini-chat-actor-openai`

**Role**: External LLM provider (OpenAI or Azure OpenAI). Processes chat completion requests, hosts uploaded files, maintains vector stores for document search. All communication routed through OAGW.

#### Cleanup Scheduler

**ID**: `cpt-cf-mini-chat-actor-cleanup-scheduler`

**Role**: Scheduled process that deletes expired temporary chats and purges associated external resources (files, vector store entries) after the retention period.

## 3. Operational Concept & Environment

No module-specific environment constraints beyond platform defaults.

## 4. Scope

This PRD uses **P0/P1/P2** to describe phased scope. The `p1`/`p2` tags on requirement checkboxes are internal priority markers and do not define release phase.

### 4.1 In Scope

- Chat CRUD (create, list, get, delete) API
- Real-time streamed AI responses (SSE)
- Persistent conversation history
- Document upload and document-aware question answering via file search
- Document summary on upload
- Thread summary compression for long conversations
- Temporary chats with 24h auto-deletion
- Per-user usage quotas (daily, monthly) with auto-downgrade to base model
- File search call limits per message and per user/day
- Token budget enforcement and context truncation
- License feature gate (`ai_chat`)
- Emit audit events to platform `audit_service` (append-only semantics owned by `audit_service`)
- Streaming cancellation when client disconnects
- Cleanup of external resources (provider files, vector store entries) on chat deletion

### 4.2 Out of Scope

- Projects or shared/collaborative chats
- Full-text search across chat history
- Non-OpenAI-compatible provider support (e.g., Anthropic, Google) - OpenAI and Azure OpenAI are supported at P0 via a shared API surface
- Complex retrieval policies beyond simple limits
- Per-workspace vector stores — P0 uses a single shared vector store per tenant (all users and chats in a tenant share one provider vector store). Per-workspace isolation is deferred.
- Image or non-document file support
- Custom audit storage (audit events are emitted to platform `audit_service`)
- Chat export or migration
- Web search tool support (P1+; schema provisions exist in `quota_usage.web_search_calls`)
- URL content extraction
- Admin configuration UI for AI policies, model selection, or provider settings (P0 uses deployment configuration; see DESIGN.md Section 2.2 constraints and emergency flags)
- Multiple quota/budget periods beyond daily and monthly (e.g. 4h, 12h rolling windows)
- Module-specific multi-lingual support (LLM handles languages natively; no module-level i18n)
- Per-feature dynamic feature flags beyond the `ai_chat` license gate and emergency kill switches (DESIGN.md lines 166-168)

### 4.3 Deferred (P2+)

#### Group Chats / Collaboration

- [ ] `p2` - **ID**: `cpt-cf-mini-chat-fr-group-chats`

Group chats and chat sharing (projects) are deferred to P2+ and are out of scope for P0/P1.

## 5. Functional Requirements

### 5.1 Core Chat

#### Chat CRUD

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-fr-chat-crud`

The system MUST allow authenticated users to create, list, retrieve, and delete chats. Each chat belongs to exactly one user within one tenant. Chat content (messages, attachments, summaries, citations) MUST be accessible only to the owning user within their tenant. Listing returns chats for the current user ordered by most recent activity. Retrieval returns chat metadata and the most recent messages. Deletion soft-deletes the chat and triggers cleanup of associated external resources.

**Rationale**: Users need to manage their conversations - create new ones, resume existing ones, and remove ones they no longer need.
**Actors**: `cpt-cf-mini-chat-actor-chat-user`

#### Streamed Chat Responses

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-fr-chat-streaming`

The system MUST deliver AI responses as a real-time SSE stream. The user sends a message and immediately begins receiving `delta` events as they are generated. The stream terminates with exactly one terminal `done` or `error` event. The terminal `done` event contains the message ID and token usage. The terminal `error` event contains an error `code` and `message` only.

The request body MAY include a client-generated `request_id` used as an idempotency key. If a `chat_turns` record with `state=running` exists for the same `(chat_id, request_id)`, the system MUST reject with `409 Conflict`. If a completed generation exists for the same `(chat_id, request_id)`, the system MUST replay the completed assistant response rather than starting a new provider request.

**Rationale**: Streaming provides perceived low latency and matches user expectations from consumer AI chat products.
**Actors**: `cpt-cf-mini-chat-actor-chat-user`

#### Conversation History

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-fr-conversation-history`

The system MUST persist all user and assistant messages. Conversation history access MUST be limited to the owning user within their tenant. On each new user message, the system MUST include relevant conversation history in the LLM context to maintain conversational coherence.

**Rationale**: Multi-turn conversations require the AI to remember prior context within the same chat.
**Actors**: `cpt-cf-mini-chat-actor-chat-user`

#### Streaming Cancellation

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-fr-streaming-cancellation`

The system MUST detect client disconnection during a streaming response and cancel the in-flight LLM request. Cancellation MUST propagate through the entire request chain to terminate the external API call.

When a stream is cancelled or disconnects before a terminal completion, the system MUST apply a bounded best-effort debit for quota enforcement so cancellation cannot be used to evade usage limits.

**Rationale**: Prevents wasted compute and cost when the user navigates away or closes the browser.
**Actors**: `cpt-cf-mini-chat-actor-chat-user`

### 5.2 Document Support

#### File Upload

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-fr-file-upload`

The system MUST allow users to upload document files (not images) to a chat. Uploaded files are processed and indexed for search. Attachment access MUST be limited to the owning user within their tenant. The system MUST return an attachment identifier and processing status.

**Rationale**: Users need to ground AI conversations in their own documents (contracts, policies, reports).
**Actors**: `cpt-cf-mini-chat-actor-chat-user`

#### Document Question Answering (File Search)

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-fr-file-search`

The system MUST support answering questions about uploaded documents by retrieving relevant excerpts during chat. File search MUST be scoped to the user's tenant. Retrieved excerpts and citations MUST be returned only to the owning user within their tenant. The system MUST enforce a configurable per-message file search call limit (default: 2 retrieval calls per message).

**Rationale**: The primary value of document upload is the ability to ask questions and get answers grounded in document content.
**Actors**: `cpt-cf-mini-chat-actor-chat-user`

#### Document Summary on Upload

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-fr-doc-summary`

The system MUST generate a brief summary of each uploaded document at upload time. The summary is stored and used in the conversation context to give the AI general awareness of attached documents without requiring a search call.

Document summary generation MUST run as a background/system task (`requester_type=system`) and MUST NOT be charged to an arbitrary end user.

**Rationale**: Improves AI response quality when the user asks general questions about attached documents.
**Actors**: `cpt-cf-mini-chat-actor-chat-user`

### 5.3 Conversation Management

#### Thread Summary Compression

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-fr-thread-summary`

The system MUST compress older conversation history into a summary when the conversation exceeds defined thresholds (message count, token count, or turn count). Thread summary access MUST be limited to the owning user within their tenant. The summary MUST preserve key facts, decisions, names, and document references. Summarized messages are retained in storage but replaced by the summary in the LLM context.

**Rationale**: Long conversations would exceed LLM context limits and increase costs without compression.
**Actors**: `cpt-cf-mini-chat-actor-chat-user`

#### Temporary Chats

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-fr-temporary-chat`

The system MUST allow users to mark a chat as temporary. Temporary chats MUST be automatically deleted (including all associated external resources) after 24 hours.

**Rationale**: Users need disposable conversations for quick questions without cluttering their chat list.
**Actors**: `cpt-cf-mini-chat-actor-chat-user`, `cpt-cf-mini-chat-actor-cleanup-scheduler`

### 5.4 Cost Control & Governance

#### Per-User Usage Quotas

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-fr-quota-enforcement`

The system MUST enforce per-user usage limits on a daily and monthly basis. Tracked metrics: input tokens, output tokens, file search calls, premium model calls. When a user exceeds their premium model quota, the system MUST auto-downgrade to a base model. When all quotas are exhausted, the system MUST reject requests with a clear error.

Quota counting MUST use two phases: Preflight (reserve) before the provider call, and commit actual usage after completion.

**Rationale**: Prevents runaway costs from individual users and ensures fair resource distribution across a tenant.
**Actors**: `cpt-cf-mini-chat-actor-chat-user`

#### Token Budget Enforcement

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-fr-token-budget`

The system MUST enforce a maximum input token budget per request. When the assembled context exceeds the budget, the system MUST truncate lower-priority content (old messages, document summaries, retrieval excerpts) while preserving the system prompt and thread summary. A reserve for output tokens MUST always be maintained.

**Rationale**: Prevents requests from exceeding provider context limits and controls per-request cost.
**Actors**: `cpt-cf-mini-chat-actor-chat-user`

#### License Gate

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-fr-license-gate`

The system MUST verify that the user's tenant has the `ai_chat` feature enabled via the platform's `license_manager`. Requests from tenants without this feature MUST be rejected with HTTP 403.

**Rationale**: AI chat is a premium feature gated by the tenant's license agreement. License verification is delegated to the platform `license_manager`.
**Actors**: `cpt-cf-mini-chat-actor-chat-user`

#### Audit Events

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-fr-audit`

The system MUST emit structured audit events to the platform's `audit_service` for every AI interaction. Each event MUST include: tenant, user, chat reference, event type, model used, token counts, latency metrics, and policy decisions (quota checks, license gate results). Mini Chat does not store audit data locally.

Before emitting events, `chat_service` MUST redact obvious secret patterns from any included content. P0 redaction rules MUST include at least:

- Replace any `Authorization: Bearer <...>` header value with `Authorization: Bearer [REDACTED]`
- Replace any `api_key`, `x-api-key`, `client_secret`, `access_token`, `refresh_token` values with `[REDACTED]` when they appear in `key=value` or JSON string field form
- Replace any `api-key: <...>` or `Ocp-Apim-Subscription-Key: <...>` header value with `[REDACTED_AZURE_KEY]`
- Replace OpenAI-style API keys with prefix `sk-` with `[REDACTED_OPENAI_KEY]`
- Replace AWS access key IDs (for example values matching `AKIA...`) with `[REDACTED_AWS_ACCESS_KEY_ID]`
- Replace JWT-like tokens (`header.payload.signature`) with `[REDACTED_JWT]`
- Replace any `password` values with `[REDACTED]` when they appear in `key=value` or JSON string field form
- Replace PEM private key blocks (lines between `-----BEGIN` and `-----END` containing `PRIVATE KEY`) with `[REDACTED_PRIVATE_KEY]`

Audit events MUST NOT include raw attachment file bytes. Audit events MAY include attachment metadata and document summaries. Any included string content MUST be truncated after redaction to a configurable maximum per field (default: 8 KiB, append `…[TRUNCATED]`). The total audit event payload MUST NOT exceed the `audit_service` event size limit.

Audit payload retention and deletion semantics are owned by platform `audit_service`.

- `audit_service` is the system of record for audit TTL and deletion semantics.
- For P0, `audit_service` MUST retain Mini Chat audit payloads for at least 90 days by default (configurable).
- Mini Chat MUST NOT attempt to delete or mutate audit records after emission.

**Rationale**: Compliance and security incident response require a record of AI usage with policy decisions. Audit storage and append-only semantics are the platform `audit_service` responsibility. Cost analytics and billing attribution are driven by internal usage records and Prometheus metrics (see `cpt-cf-mini-chat-fr-cost-metrics`), not by audit events.
**Actors**: `cpt-cf-mini-chat-actor-chat-user`

#### Cost Metrics

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-fr-cost-metrics`

The system MUST log the following metrics for every LLM request: model, input tokens, output tokens, file search call count, time to first token, total latency. Tenant and user attribution MUST be available via audit events and internal usage records; Prometheus labels MUST NOT include `tenant_id` or `user_id`.

**Rationale**: Enables cost monitoring, budget alerts, and billing attribution per tenant/user.
**Actors**: `cpt-cf-mini-chat-actor-chat-user`

### 5.5 Data Lifecycle

#### Chat Deletion with Resource Cleanup

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-fr-chat-deletion-cleanup`

When a chat is deleted, the system MUST mark attachments for asynchronous cleanup and return without blocking on external provider operations. A cleanup worker MUST perform idempotent retries to remove files from the tenant's vector store and then delete the provider file. Local data MUST be soft-deleted or anonymized per the retention policy and hard-purged by a periodic cleanup job after a configurable grace period.

For temporary chats, “deleted within 25 hours” means:

- The chat is soft-deleted and no longer appears in chat list or history APIs within 25 hours of creation.
- Provider cleanup (vector store removal + provider file deletion) has a target completion time of 1 hour under normal conditions and is eventual with retry/backoff.

**Rationale**: Prevents orphaned external resources and ensures data governance compliance on deletion.
**Actors**: `cpt-cf-mini-chat-actor-chat-user`, `cpt-cf-mini-chat-actor-cleanup-scheduler`

## 6. Non-Functional Requirements

### 6.1 Module-Specific NFRs

#### Tenant Isolation

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-nfr-tenant-isolation`

Tenant data MUST never be accessible to users from another tenant. All data queries, file operations, and vector store searches MUST be scoped by tenant. The API MUST NOT accept raw external resource identifiers (file IDs, vector store IDs) from clients.

Parent tenant / MSP administrators MUST NOT have access to chat content. Admin visibility is limited to aggregated usage and operational metrics.

Authorization follows the platform PDP/PEP fail-closed rules (including 404 masking for denied requests with a concrete resource ID); see DESIGN.md (Authorization / Fail-Closed Behavior).

**Threshold**: Zero cross-tenant data leaks
**Rationale**: Multi-tenant SaaS with sensitive documents requires strict data boundaries.
**Architecture Allocation**: See DESIGN.md section 2.1 (Tenant-Scoped Everything principle)

#### Authorization Alignment

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-nfr-authz-alignment`

Authorization MUST follow the platform PDP/PEP model, including query-level constraints compiled to SQL by the PEP and fail-closed behavior on PDP errors or unreachability.

**Threshold**: Zero unauthorized reads/writes; fail-closed on 100% of PDP failures
**Rationale**: Chat content is sensitive and access must be enforced consistently at the query layer.
**Architecture Allocation**: See DESIGN.md section 3.8 (Authorization (PEP)) and Authorization Design (platform)

#### Cost Predictability

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-nfr-cost-control`

Per-user LLM costs MUST be bounded by configurable daily and monthly quotas. File search costs MUST be bounded by per-message and per-day call limits. The system MUST track actual costs with tenant aggregation and per-user attribution for quota enforcement. Administrator visibility is limited to aggregated usage and operational metrics.

**Threshold**: No user exceeds configured quota; estimated cost available for 100% of requests
**Rationale**: Unbounded LLM usage can generate unexpected costs; tenants need cost predictability.
**Architecture Allocation**: See DESIGN.md section 3.2 (quota_service component)

#### Streaming Latency

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-nfr-streaming-latency`

The system MUST minimize platform overhead beyond provider latency. Define `mini_chat_ttft_overhead_ms = t_first_token_ui - t_first_byte_from_provider`. Streaming events MUST be relayed without buffering.

**Threshold**: `mini_chat_ttft_overhead_ms` p99 < 50 ms (platform overhead excluding provider latency)
**Rationale**: Users expect near-instant response start in a chat interface.
**Architecture Allocation**: See DESIGN.md section 2.1 (Streaming-First principle)

#### Data Retention Compliance

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-nfr-data-retention`

Temporary chats MUST be deleted within 25 hours of creation. Target: deleted chat resources (files, vector store entries) at the external provider removed within 1 hour under normal conditions; eventually consistent with retry/backoff on provider errors.

**Threshold**: 100% of temporary chats cleaned up within SLA; target external resource cleanup within 1 hour under normal conditions, with retry/backoff on provider errors
**Rationale**: Regulatory and customer contractual requirements for data lifecycle management.
**Architecture Allocation**: See DESIGN.md section 4 (Cleanup on Chat Deletion)

### 6.2 Observability and Supportability (P0)

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-nfr-observability-supportability`

Mini Chat MUST provide an explicit operational contract to support on-call, SRE, and cost governance. This includes:

#### Required support signals (P0)

- Every chat turn MUST have a stable `request_id` (client idempotency key) and a persisted turn state (`running|completed|failed|cancelled`) that is exposed via the Turn Status API as (`running|done|error|cancelled`).
- Every completed provider request MUST be correlated via `provider_response_id` and MUST be persisted and searchable by operators.
- Support tooling MUST be able to determine turn state using server-side state (not inferred from client retry behavior).

#### Prometheus metrics contract (P0)

The service MUST expose Prometheus metrics with the following series names (types and label sets as specified in DESIGN.md):

Prometheus labels MUST NOT include high-cardinality identifiers such as `tenant_id`, `user_id`, `chat_id`, `request_id`, or `provider_response_id`.

##### Streaming and UX health

- `mini_chat_stream_started_total{provider,model}`
- `mini_chat_stream_completed_total{provider,model}`
- `mini_chat_stream_failed_total{provider,model,error_code}`
- `mini_chat_stream_disconnected_total{stage}`
- `mini_chat_stream_replay_total{reason}`
- `mini_chat_active_streams{instance}`
- `mini_chat_ttft_provider_ms{provider,model}`
- `mini_chat_ttft_overhead_ms{provider,model}`
- `mini_chat_stream_total_latency_ms{provider,model}`

##### Cancellation

- `mini_chat_cancel_requested_total{trigger}`
- `mini_chat_cancel_effective_total{trigger}`
- `mini_chat_tokens_after_cancel{trigger}`
- `mini_chat_time_to_abort_ms{trigger}`
- `mini_chat_time_from_ui_disconnect_to_cancel_ms{trigger}`
- `mini_chat_cancel_orphan_total`

##### Quota and cost control

- `mini_chat_quota_preflight_total{decision,model}`
- `mini_chat_quota_reserve_total{period}`
- `mini_chat_quota_commit_total{period}`
- `mini_chat_quota_overshoot_total{period}`
- `mini_chat_quota_negative_total{period}`
- `mini_chat_quota_estimated_tokens`
- `mini_chat_quota_actual_tokens`
- `mini_chat_quota_overshoot_tokens`
- `mini_chat_quota_reserved_tokens{period}`

##### Tools and retrieval

- `mini_chat_tool_calls_total{tool,phase}`
- `mini_chat_tool_call_limited_total{tool}`
- `mini_chat_file_search_latency_ms{provider,model}`
- `mini_chat_citations_count`

##### Summarization health

- `mini_chat_summary_regen_total{reason}`
- `mini_chat_summary_fallback_total`

##### Provider / OAGW interaction

- `mini_chat_provider_requests_total{provider,endpoint}`
- `mini_chat_provider_errors_total{provider,status}`
- `mini_chat_oagw_retries_total{provider,reason}`
- `mini_chat_oagw_circuit_open_total{provider}`
- `mini_chat_provider_latency_ms{provider,endpoint}`
- `mini_chat_oagw_upstream_latency_ms{provider,endpoint}`

##### Upload and attachments

- `mini_chat_attachment_upload_total{result}`
- `mini_chat_attachment_index_total{result}`
- `mini_chat_attachment_summary_total{result}`
- `mini_chat_attachments_pending{instance}`
- `mini_chat_attachments_failed{instance}`
- `mini_chat_attachment_upload_bytes`
- `mini_chat_attachment_index_latency_ms`

##### Cleanup and drift

- `mini_chat_cleanup_job_runs_total{kind}`
- `mini_chat_cleanup_attempts_total{op,result}`
- `mini_chat_cleanup_orphan_found_total{kind}`
- `mini_chat_cleanup_orphan_fixed_total{kind}`
- `mini_chat_cleanup_backlog{state}`
- `mini_chat_cleanup_latency_ms{op}`

##### Audit emission health

- `mini_chat_audit_emit_total{result}`
- `mini_chat_audit_redaction_hits_total{pattern}`
- `mini_chat_audit_emit_latency_ms`

##### DB health (chat_store)

- `mini_chat_db_query_latency_ms{query}`
- `mini_chat_db_errors_total{query,code}`

#### SLOs / thresholds (P0)

- `mini_chat_ttft_overhead_ms` p99 < 50 ms
- `mini_chat_time_to_abort_ms` p99 < 200 ms
- Temporary chat soft-delete within 25 hours
- Provider cleanup target completion within 1 hour under normal conditions (eventual with retry)

### 6.3 UX Recovery Contract (P0)

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-fr-ux-recovery`

The UI experience MUST be resilient to SSE disconnects and idempotency conflicts.

#### Disconnect before terminal event

- If the SSE stream disconnects before `done`/`error`, the UI MUST treat the send as indeterminate and MUST NOT auto-retry `POST /messages:stream` with the same `request_id`.
- After disconnect, the UI MAY call `GET /v1/chats/{chat_id}/turns/{request_id}` to determine whether the turn completed.
- The UI MUST show a user-visible banner with the exact text: `Connection lost. Message delivery is uncertain. You can resend.`
- If the user chooses to resend, the UI MUST generate a new `request_id`.

#### 409 Conflict (active generation)

- On `409 Conflict` for `(chat_id, request_id)`, the UI MUST show a user-visible banner with the exact text: `A response is already in progress for this message. Please wait.`

#### Completed replay (idempotent replay)

- If the server replays a completed generation for an existing `(chat_id, request_id)`, the UI MUST render the response without duplicating the message in the timeline.
- The UI MUST show a non-blocking banner with the exact text: `Recovered a previously completed response.`

## 7. Public Library Interfaces

### 7.1 Public API Surface

#### Chat REST API

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-interface-rest-api`

**Type**: REST API
**Stability**: stable
**Description**: Public HTTP API for chat management, message streaming, and file upload. All endpoints require authentication and tenant license verification.
**Breaking Change Policy**: Versioned via URL prefix (`/v1/`). Breaking changes require new version.

#### Turn Status (read-only) API (P0 optional, recommended)

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-interface-turn-status`

Support and UX recovery flows SHOULD be able to query authoritative turn state backed by `chat_turns`.

**Endpoint**: `GET /v1/chats/{chat_id}/turns/{request_id}`

**Response**:

- `chat_id`
- `request_id`
- `state`: `running|done|error|cancelled`
- `updated_at`

### 7.2 External Integration Contracts

#### SSE Streaming Contract

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-contract-sse-streaming`

**Direction**: provided by library
**Protocol/Format**: Server-Sent Events (SSE) over HTTP
**Compatibility**: Event types (`delta`, `tool`, `citations`, `done`, `error`, `ping`) and their payload schemas are stable within a major API version.

**Ordering**: `ping* (delta|tool|citations)* (done|error)`.

Provider identifiers (e.g., `file_id` in `citations`) are display-only metadata; clients MUST NOT send them back to any API.

## 8. Use Cases

### UC-001: Send Message and Receive Streamed Response

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-usecase-send-message`

**Actor**: `cpt-cf-mini-chat-actor-chat-user`

**Preconditions**:
- User is authenticated and tenant has `ai_chat` license
- Chat exists and belongs to the user

**Main Flow**:
1. User sends a message to an existing chat
2. System checks and reserves user quota (Preflight (reserve))
3. System assembles conversation context (summary, recent messages, document summaries)
4. System streams AI response SSE events back to the user in real-time
5. System persists both user message and assistant response
6. System emits audit events with usage metrics

**Postconditions**:
- Message and response persisted in chat history
- Usage counters updated
- Audit events emitted to platform `audit_service`

**Alternative Flows**:
- **Quota exceeded**: System rejects request with `quota_exceeded` error; no LLM call made
- **Client disconnects**: System cancels in-flight LLM request; partial response may be persisted. Delivery is indeterminate; the UI SHOULD first query `GET /v1/chats/{chat_id}/turns/{request_id}` to determine whether the turn completed. If the user resends, resend MUST use a new `request_id`.

#### UC-006: Reconnect After Network Loss (Turn Status Check)

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-usecase-reconnect-turn-status`

**Actor**: `cpt-cf-mini-chat-actor-chat-user`

**Preconditions**:

- The UI previously started a streaming send with a `request_id`.
- The SSE stream disconnected before terminal `done`/`error`.

**Main Flow**:

1. The UI calls `GET /v1/chats/{chat_id}/turns/{request_id}`.
2. If `state=done`, the UI renders the previously completed response and shows `Recovered a previously completed response.`
3. If `state=running`, the UI informs the user that a response is still in progress and does not resend.
4. If `state=error|cancelled`, the UI allows the user to resend using a new `request_id`.

#### UC-002: Send Message with Document Search

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-usecase-doc-search`

**Actor**: `cpt-cf-mini-chat-actor-chat-user`

**Preconditions**:
- Same as UC-001
- At least one document is attached to the chat and has `ready` status

**Main Flow**:
1. User sends a message that references document content
2. System detects that file search is needed
3. System retrieves relevant excerpts from the tenant's document index
4. System includes excerpts in the LLM context alongside conversation history
5. System streams AI response grounded in document content

**Postconditions**:
- Response incorporates information from uploaded documents
- File search call counted against user quota

**Alternative Flows**:
- **File search limit reached**: System proceeds without retrieval; response based on conversation context and document summaries only

#### UC-003: Upload Document

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-usecase-upload-document`

**Actor**: `cpt-cf-mini-chat-actor-chat-user`

**Preconditions**:
- User is authenticated and tenant has `ai_chat` license
- Chat exists and belongs to the user
- File is a supported document type and within size limits

**Main Flow**:
1. User uploads a document file to a chat
2. System stores the file with the external provider
3. System indexes the file in the tenant's document search index
4. System enqueues a brief summary generation of the document (background, `requester_type=system`)
5. System returns attachment ID and `ready` status

**Postconditions**:
- Document is searchable in subsequent chat messages
- Document summary available for context assembly

**Alternative Flows**:
- **Unsupported file type**: System rejects with `unsupported_file_type` error
- **File too large**: System rejects with `file_too_large` error
- **Processing failure**: Attachment status set to `failed`; user informed

#### UC-004: Delete Chat

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-usecase-delete-chat`

**Actor**: `cpt-cf-mini-chat-actor-chat-user`

**Preconditions**:
- Chat exists and belongs to the user

**Main Flow**:
1. User requests chat deletion
2. System soft-deletes the chat
3. System marks attachments for cleanup and returns
4. Cleanup worker removes file from tenant vector store and deletes the provider file (idempotent retries)
5. System emits audit events

**Postconditions**:
- Chat no longer appears in user's chat list
- External resources cleaned up
- Audit events emitted to platform `audit_service`

#### UC-005: Temporary Chat Auto-Deletion

- [ ] `p1` - **ID**: `cpt-cf-mini-chat-usecase-temporary-chat-cleanup`

**Actor**: `cpt-cf-mini-chat-actor-cleanup-scheduler`

**Preconditions**:
- Temporary chat exists with creation time > 24 hours ago

**Main Flow**:
1. Scheduler identifies expired temporary chats
2. System executes the same deletion flow as UC-004 for each expired chat

**Postconditions**:
- All expired temporary chats and their external resources are removed

## 9. Acceptance Criteria

- [ ] User can create a chat, send messages, and receive streamed AI responses with `mini_chat_ttft_overhead_ms` p99 < 50 ms platform overhead (excluding provider latency)
- [ ] Cancellation propagation meets design thresholds: `mini_chat_time_to_abort_ms` p99 < 200 ms and `mini_chat_tokens_after_cancel` p99 < 50 tokens
- [ ] User can upload a document and ask questions that are answered using document content
- [ ] Users from different tenants cannot access each other's chats, documents, or search results
- [ ] User exceeding daily quota receives a clear error message and is auto-downgraded to a base model
- [ ] Temporary chats are automatically deleted within 25 hours
- [ ] Deleted chat resources are removed from the external provider (target: within 1 hour under normal conditions; eventual with retry/backoff)
- [ ] Every AI interaction emits audit events to platform `audit_service` with usage metrics
- [ ] Long conversations (50+ turns) remain functional via thread summary compression

## 10. Dependencies

| Dependency | Description | Criticality |
|------------|-------------|-------------|
| Platform API Gateway | HTTP routing, SSE transport | `p1` |
| Platform AuthN | User authentication, tenant resolution | `p1` |
| Outbound API Gateway (OAGW) | External API egress, credential injection | `p1` |
| OpenAI-compatible Responses API (OpenAI / Azure OpenAI) | LLM chat completion (streaming and non-streaming) | `p1` |
| OpenAI-compatible Files API (OpenAI / Azure OpenAI) | Document upload and storage | `p1` |
| OpenAI-compatible Vector Stores / File Search (OpenAI / Azure OpenAI) | Document indexing and retrieval | `p1` |
| PostgreSQL | Primary data storage | `p1` |
| Platform license_manager | Tenant feature flag resolution (`ai_chat`) | `p1` |
| Platform audit_service | Audit event ingestion (prompts, responses, usage, policy decisions) | `p1` |

## 11. Assumptions

- OpenAI-compatible Responses API, Files API, and File Search remain stable and available (OpenAI or Azure OpenAI)
- OAGW supports streaming SSE relay and credential injection for OpenAI and Azure OpenAI endpoints
- OAGW owns Azure OpenAI endpoint details including required `api-version` parameters and path variants
- Platform AuthN provides `user_id` and `tenant_id` in the security context for every request
- Platform `license_manager` can resolve the `ai_chat` feature flag synchronously
- Platform `audit_service` is available to receive audit events
- One provider vector store per tenant is sufficient for P0 document volumes
- Thread summary quality is adequate for maintaining conversational coherence over long chats

## 12. Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| OpenAI-compatible provider API changes or deprecation (OpenAI / Azure OpenAI) | Feature breakage; requires rework | Pin API versions; monitor deprecation notices; design for eventual multi-provider |
| Provider outage or degraded performance (OpenAI / Azure OpenAI) | Chat unavailable or slow | Circuit breaking via OAGW; clear error messaging to users; eventual fallback provider (P2+) |
| Cost overruns from unexpected usage patterns | Budget exceeded at tenant level | Per-user quotas; file search call limits; token budgets; cost monitoring and alerts |
| Thread summary loses critical context | Degraded conversation quality over long chats | Include explicit instructions to preserve decisions, facts, names, document refs; allow users to start new chats |
| Vector store data consistency on deletion | Orphaned files at provider | Idempotent cleanup with retry; reconciliation job for detecting orphans |
| Large document volumes per tenant exceeding vector store limits | Search quality degrades; upload failures | Monitor per-tenant file counts; enforce upload limits; plan per-workspace stores (P2) |

## 13. Open Questions

- What document file types are supported in P0 beyond `pdf`, `docx`, and plain text?
- What is the exact UX when `state=running` is returned from Turn Status API (poll cadence, max wait, and banner text)?
- Thread summary trigger thresholds are defined in DESIGN.md (msg count > 20 OR tokens > budget OR every 15 user turns)
- Is the system prompt configurable per tenant, or fixed platform-wide?

### 13.1 P0 Defaults (configurable)

These defaults are used for P0 planning and MUST be configurable per tenant/operator:

- Default chat model: `gpt-5.2`
- Auto-downgrade/base model: `gpt-5-mini`
- Default quota targets (tokens): daily `50_000`, monthly `1_000_000`
- Upload size limit: 16 MiB (deployment config example: `uploaded_file_max_size_kb: 16384`)
- Temporary chat retention window: 24 hours (deployment config example: `temporary_chat_retention_hours: 24`)

## 14. Traceability

- **Design**: [DESIGN.md](./DESIGN.md)
- **ADRs**: [ADR/](./ADR/)
- **Features**: [features/](./features/) (planned)
