# PRD

## 1. Overview

**Purpose**: Chat Engine is a proxy service that manages session lifecycle and message routing between clients and external webhook backends.

Chat Engine provides a unified interface for building conversational applications by abstracting session management, message history persistence, and flexible message processing. The system acts as an intermediary layer that handles the complexity of session state, message tree structures, and backend integration, allowing application developers to focus on building user experiences and webhook backend developers to focus on message processing logic.

The core value proposition is enabling flexible, stateful conversation management with support for advanced features like message regeneration and conversation branching. By decoupling the conversation infrastructure from processing logic, Chat Engine enables rapid experimentation with different AI models, processing backends, and conversation patterns without requiring changes to client applications.

The system supports various conversation patterns including traditional linear chat and branching conversations with variant exploration. This flexibility enables use cases ranging from AI-powered assistants to human-in-the-loop support systems.

**Target Users**:
- **Application Developers** - Build chat applications using Chat Engine as backend infrastructure for session and message management
- **Webhook Backend Developers** - Implement custom message processing logic (AI, rule-based, human-in-the-loop) that integrates with Chat Engine
- **End Users** (indirect) - Use applications built on Chat Engine, experiencing responsive conversational interfaces

**Key Problems Solved**:
- **Session Management Complexity**: Eliminates the need for each application to implement session lifecycle, message history persistence, and state management from scratch
- **Message Routing Flexibility**: Decouples message processing logic from infrastructure, enabling easy switching between different backends (AI models, custom logic, human operators)
- **Conversation Variants**: Provides built-in support for message regeneration and branching conversations, enabling users to explore alternative responses without losing conversation history
- **Multi-Backend Support**: Allows seamless switching between different message processing backends mid-conversation, enabling hybrid approaches like starting with AI and escalating to human support

**Success Criteria**:
- Message routing latency < 100ms (p95) excluding backend processing time
- 99.9% uptime for session management operations
- Support for 10,000 concurrent sessions per instance
- Zero message loss during backend failures
- First message response time < 200ms from session creation

**Capabilities**:
- Session lifecycle management (create, delete, retrieve)
- Message routing to webhook backends with real-time streaming
- Message variant preservation (regeneration, branching)
- File attachment references in messages
- Session type switching mid-conversation
- Session export (JSON, Markdown, TXT)
- Session sharing via links with read-only and branching access
- Message search within sessions and across sessions
- Message tree navigation and variant selection

### Glossary

| Term | Definition |
|------|------------|
| **Session** | A persistent conversation context with a unique ID, owned by a client and associated with a session type |
| **Session Type** | A configuration profile that maps a session to a webhook backend and defines enabled capabilities (file attachments, summarization, WebSocket, etc.) |
| **Webhook Backend** | An external HTTP service that receives session context and messages from Chat Engine and returns responses |
| **Message Tree** | A tree structure where each message references a parent message; sibling nodes with the same parent are variants |
| **Message Variant** | An alternative response at the same position in the conversation tree — created by regeneration or branching |
| **Capability** | A feature flag returned by the webhook backend at session creation time that enables optional Chat Engine functionality |
| **Streaming Response** | Real-time forwarding of response chunks from the webhook backend to the client as they are generated |
| **Lifecycle State** | One of four session states: `active`, `archived`, `soft_deleted`, `hard_deleted` |
| **is_hidden_from_user** | Message visibility flag that excludes the message from client-facing APIs |
| **is_hidden_from_llm** | Message visibility flag that excludes the message from the context sent to webhook backends |

## 2. Actors

### 2.1 Human Actors

#### Client Application Developer

**ID**: `cpt-chat-engine-actor-developer`

<!-- fdd-id-content -->
**Role**: Integrates Chat Engine into applications by configuring session types, implementing client-side UI for message display and navigation, and managing user authentication and file uploads.
<!-- fdd-id-content -->

#### End User

**ID**: `cpt-chat-engine-actor-end-user`

<!-- fdd-id-content -->
**Role**: Interacts with client applications built on Chat Engine, sending messages, receiving responses, and navigating conversation variants (indirect actor, does not directly interact with Chat Engine).
<!-- fdd-id-content -->

#### Webhook Backend Developer

**ID**: `cpt-chat-engine-actor-backend-developer`

<!-- fdd-id-content -->
**Role**: Implements webhook backends that receive session context and messages from Chat Engine, process them according to custom logic (AI, rules, human-in-the-loop), and return responses.
<!-- fdd-id-content -->

### 2.2 System Actors

#### Client Application

**ID**: `cpt-chat-engine-actor-client`

<!-- fdd-id-content -->
**Role**: Frontend application (web, mobile, desktop) that sends messages to Chat Engine, receives streaming responses, and renders conversation UI including message trees and variants.
<!-- fdd-id-content -->

#### Webhook Backend

**ID**: `cpt-chat-engine-actor-webhook-backend`

<!-- fdd-id-content -->
**Role**: External HTTP service that processes messages and returns responses. Receives full session context, message history, and capabilities from Chat Engine. Implements custom message processing logic.
<!-- fdd-id-content -->

#### File Storage Service

**ID**: `cpt-chat-engine-actor-file-storage`

<!-- fdd-id-content -->
**Role**: External file storage service (e.g., S3, GCS) that stores file attachments. Provides signed URL access for file upload and download. Client applications upload files directly to storage.
<!-- fdd-id-content -->

#### Database Service

**ID**: `cpt-chat-engine-actor-database`

<!-- fdd-id-content -->
**Role**: Persistent storage for sessions, messages, message tree structures, and metadata. Supports ACID transactions to ensure data integrity and consistency.
<!-- fdd-id-content -->

## 3. Functional Requirements

#### FR-001: Create Session

- [ ] `p1` - **ID**: `cpt-chat-engine-fr-create-session`

<!-- fdd-id-content -->
The system **MUST** create a new session with a specified session type and client ID. The system binds each session to the requesting user (`user_id`) and tenant (`tenant_id`), both extracted from the JWT bearer token — they are never accepted from the request body. The system notifies the webhook backend of the new session and receives available capabilities for that session type. The capabilities determine which features are enabled (file attachments, session switching, summarization, etc.).

**Actors**: `cpt-chat-engine-actor-client`, `cpt-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

#### FR-002: Send Message with Streaming Response

- [ ] `p1` - **ID**: `cpt-chat-engine-fr-send-message`

<!-- fdd-id-content -->
The system **MUST** forward user messages to webhook backend with full session context (session metadata, capabilities, message history) and stream responses back to client in real-time. The system persists the complete message exchange (user message and assistant response) after streaming completes.

**Actors**: `cpt-chat-engine-actor-client`, `cpt-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

#### FR-003: Attach Files to Messages

- [ ] `p1` - **ID**: `cpt-chat-engine-fr-attach-files`

<!-- fdd-id-content -->
The system **MUST** support file references in messages. Clients upload files to File Storage Service, obtain file UUIDs (stable identifiers), and include these UUIDs in message payloads. The system stores UUIDs in message records and forwards them to webhook backends as part of message context. File handling is enabled only if session capabilities allow it.

**File Upload Workflow:**
1. Client calls File Storage Service upload endpoint
2. File Storage returns UUID as file identifier
3. Client includes UUID in message send request (file_ids array, max 10 files)
4. Chat Engine stores UUIDs in message record
5. Webhook backends receive UUIDs and fetch files from File Storage as needed

**File Access Control:**
- UUIDs are stable identifiers that do not expire
- File Storage Service controls access via separate authentication
- Webhook backends must have credentials for File Storage API
- Clients retrieve files by requesting temporary signed URLs from File Storage

**Actors**: `cpt-chat-engine-actor-client`, `cpt-chat-engine-actor-file-storage`
<!-- fdd-id-content -->

#### FR-004: Switch Session Type

- [ ] `p2` - **ID**: `cpt-chat-engine-fr-switch-session-type`

<!-- fdd-id-content -->
The system **SHOULD** allow switching to a different session type mid-session. When switching occurs, the next message is routed to the new webhook backend with full message history. The new backend returns updated capabilities which apply for subsequent messages.

**Actors**: `cpt-chat-engine-actor-client`, `cpt-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

#### FR-005: Recreate Assistant Response

- [ ] `p1` - **ID**: `cpt-chat-engine-fr-recreate-response`

<!-- fdd-id-content -->
The system **MUST** allow regeneration of assistant responses. When recreation is requested, the old response is preserved as a variant in the message tree, and a new response is generated and stored as a sibling (same parent, different branch). Both variants remain accessible for navigation.

**Actors**: `cpt-chat-engine-actor-client`, `cpt-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

#### FR-006: Branch from Message

- [ ] `p2` - **ID**: `cpt-chat-engine-fr-branch-message`

<!-- fdd-id-content -->
The system **SHOULD** allow creating new messages from any point in conversation history, creating alternative conversation paths. When branching, the system loads context up to the specified parent message and forwards the new message to the backend with truncated history. Both conversation branches remain preserved.

**Actors**: `cpt-chat-engine-actor-client`, `cpt-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

#### FR-007: Navigate Message Variants

- [ ] `p2` - **ID**: `cpt-chat-engine-fr-navigate-variants`

<!-- fdd-id-content -->
The system **SHOULD** allow navigation between message variants (siblings with same parent message). When retrieving messages, the system provides variant position information (e.g., "2 of 3") and allows clients to request specific variants.

Webhook backends receive message history with file_ids (UUIDs). Backends must implement File Storage Service client to fetch file content when needed.

**Actors**: `cpt-chat-engine-actor-client`
<!-- fdd-id-content -->

#### FR-008: Stop Streaming Response

- [ ] `p1` - **ID**: `cpt-chat-engine-fr-stop-streaming`

<!-- fdd-id-content -->
The system **MUST** allow canceling streaming responses mid-generation. When cancellation occurs, the system stops forwarding data from webhook backend, closes the connection, and saves the partial response as an incomplete message with appropriate metadata.

**Actors**: `cpt-chat-engine-actor-client`
<!-- fdd-id-content -->

#### FR-009: Export Session

- [ ] `p3` - **ID**: `cpt-chat-engine-fr-export-session`

<!-- fdd-id-content -->
The system **MAY** export sessions in JSON, Markdown, or TXT format. Export can include only the active conversation path or all message variants. The system uploads the formatted export to file storage and returns a download URL.

**Actors**: `cpt-chat-engine-actor-client`
<!-- fdd-id-content -->

#### FR-010: Share Session

- [ ] `p3` - **ID**: `cpt-chat-engine-fr-share-session`

<!-- fdd-id-content -->
The system **MAY** generate shareable links for sessions. Recipients can view sessions in read-only mode and create branches from the last message in the session. Branches created by recipients do not affect the original session owner's conversation path.

**Actors**: `cpt-chat-engine-actor-client`, `cpt-chat-engine-actor-end-user`
<!-- fdd-id-content -->

#### FR-011: Session Summary

- [ ] `p2` - **ID**: `cpt-chat-engine-fr-session-summary`

<!-- fdd-id-content -->
The system **SHOULD** support session summarization if enabled by session type capabilities. Summary generation is triggered automatically or on demand and can be handled by the webhook backend or a dedicated summarization service. The summary is stored as session metadata.

**Actors**: `cpt-chat-engine-actor-client`, `cpt-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

#### FR-016: Conversation Memory Management Strategies

- [ ] `p2` - **ID**: `cpt-chat-engine-fr-conversation-memory`

<!-- fdd-id-content -->
The system **SHOULD** provide guidance and capabilities to support conversation memory management strategies for handling long-running sessions that exceed LLM context window limits. Webhook backends can implement various strategies to optimize token usage while preserving conversation context.

**Memory Management Strategies**:
1. **Full History** (default) - Send complete message history (suitable for short conversations)
2. **Sliding Window** - Keep last N messages or T tokens (predictable token usage)
3. **Summarization + Recent** - Summarize old messages, keep recent ones verbatim
4. **Importance Filtering** - Keep semantically important messages, filter filler
5. **Hierarchical Summarization** - Multi-level summaries for very long conversations
6. **Visibility Flags** - Use `is_hidden_from_llm` to exclude messages from context

**System Capabilities Supporting Strategies**:
- Session Summary (FR-011) - Webhook can request conversation summaries
- Message Visibility Flags - Mark messages as `is_hidden_from_llm=true` to exclude from context
- Branching (FR-006) - Create new conversation path with truncated history
- Message Tree Navigation - Backends can traverse history to implement custom strategies
- Session Metadata - Store strategy configuration and state (e.g., last summarization point)

**Backend Responsibilities**:
- Choose appropriate strategy based on session type and conversation length
- Implement token counting and context window management
- Handle summarization or filtering logic
- Store strategy state in session metadata if needed
- Monitor token usage and adjust strategy dynamically

**Strategy Selection Guidelines**:
- **<50 messages**: Full History (default)
- **50-200 messages**: Sliding Window or Visibility Flags
- **200-1000 messages**: Summarization + Recent Messages
- **1000+ messages**: Hierarchical Summarization or Importance Filtering
- **Context window limits**: Adjust strategy based on model (8K, 32K, 128K tokens)

**Trade-offs**:
- **Full History**: High fidelity but expensive for long conversations
- **Sliding Window**: Predictable costs but loses older context
- **Summarization**: Balanced approach but adds summarization overhead
- **Importance Filtering**: Optimal quality but complex to implement

**Actors**: `cpt-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

#### FR-017: Individual Message Deletion

- [ ] `p1` - **ID**: `cpt-chat-engine-fr-delete-message`

<!-- fdd-id-content -->
The system **MUST** support deletion of individual messages within a session. When a message is deleted, all associated reactions are cascade-deleted automatically to maintain referential integrity. The system validates ownership (authenticated user must own the message) before deletion and notifies the webhook backend of the deletion event.

**Deletion Behavior**:
- **Hard delete only**: Messages are permanently removed (no soft delete for individual messages)
- **Cascade delete reactions**: All reactions associated with the message are automatically deleted
- **Ownership validation**: Only the message owner (authenticated user) can delete their messages
- **Webhook notification**: Backend receives message_deleted event with message_id and timestamp
- **Error handling**: Returns 403 Forbidden if user doesn't own message, 404 if message not found

**Use Cases**:
- User wants to remove a message they regret sending
- User wants to clean up test messages or mistakes
- User wants to remove sensitive information accidentally shared

**Constraints**:
- Cannot delete messages that are parent to other messages (would break conversation tree)
- Cannot delete assistant messages (only user messages can be deleted by users)
- Deletion is permanent and cannot be undone

**Actors**: `cpt-chat-engine-actor-client`, `cpt-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

#### FR-018: Per-Message Feedback

- [ ] `p2` - **ID**: `cpt-chat-engine-fr-message-feedback`

<!-- fdd-id-content -->
The system **SHOULD** support per-message feedback in the form of like/dislike reactions and optional text comments. Feedback enables quality monitoring, model evaluation, and user satisfaction tracking. Each message can have at most one reaction per user, with reaction changes (like → dislike) replacing the previous reaction. The system stores feedback metadata and optionally forwards it to webhook backends for analytics.

**Reaction Types**:
- **like**: Positive feedback (thumbs up)
- **dislike**: Negative feedback (thumbs down)
- **none**: Remove existing reaction

**Behavioral Rules**:
- Each message can have at most one reaction per user
- Reaction changes (like → dislike) replace the previous reaction
- Optional text comment supported per reaction (bounded length)
- Reactions are tied to authenticated users (not anonymous)

**Webhook Integration**:
- Backends receive `message_feedback` events when reactions are added/changed
- Events include message_id, reaction_type, comment, user_id, timestamp
- Backends can use feedback for model fine-tuning, quality metrics, A/B testing

**Privacy & Data Handling**:
- Feedback is tied to authenticated user (not anonymous)
- Comments are stored encrypted if they contain sensitive information
- Feedback can be queried by client for display or exported with session data

**Capability Gating**: Enabled if session type supports feedback capability

**Actors**: `cpt-chat-engine-actor-client`, `cpt-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

#### FR-019: Context Overflow Strategies

- [ ] `p2` - **ID**: `cpt-chat-engine-fr-context-overflow`

<!-- fdd-id-content -->
The system **SHOULD** provide explicit support for handling context window overflow when message history exceeds LLM token limits. Chat Engine provides primitives and metadata to enable webhook backends to implement various overflow strategies. The system does not enforce a specific strategy but provides the mechanisms for backends to implement their chosen approach.

**Supported Strategy Primitives**:

1. **Sliding Window**: Keep only the most recent N messages to bound context size
2. **Hard Stop**: Reject new messages when the session exceeds a configured token threshold
3. **Drop-Middle**: Retain the beginning and end of the conversation, dropping the middle portion
4. **Summarization**: Use `cpt-chat-engine-fr-session-summary` to compress older messages into a summary that is included instead of verbatim history
5. **Message Visibility Flags**: Mark individual messages with `is_hidden_from_llm` to exclude them from context sent to backends

**System Support**:
- Session metadata exposes estimated message count and token usage for backend decision-making
- Session metadata stores strategy configuration and state between messages
- Message tree navigation supports arbitrary history traversal by backends
- `cpt-chat-engine-fr-session-summary` provides summarization capability

**Default Strategy**: Full History (send all messages until overflow, then error)

**Backend Selection Guidelines**:
- **Short sessions (<50 msgs)**: Full History
- **Medium sessions (50-200 msgs)**: Sliding Window
- **Long sessions (200-1000 msgs)**: Summarization + Recent
- **Very long sessions (1000+ msgs)**: Hierarchical Summarization or Drop-Middle

**Strategy Trade-offs**:
- **Full History**: Highest fidelity, expensive for long conversations
- **Sliding Window**: Predictable costs, loses context over time
- **Summarization**: Balanced approach, adds latency for summary generation
- **Drop-Middle**: Preserves key context, may lose important middle details
- **Hard Stop**: Simplest, worst UX (forces session restart)

**Capability Gating**: Strategy configuration exposed via session capabilities

**Actors**: `cpt-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

#### FR-020: Message Retention & Cleanup Policies

- [ ] `p2` - **ID**: `cpt-chat-engine-fr-message-retention`

<!-- fdd-id-content -->
The system **SHOULD** support message-level retention policies that automatically clean up old messages while preserving session structure. Unlike session deletion (FR-014), message retention policies allow selective message cleanup to optimize storage costs while keeping sessions accessible. Cleanup operations preserve message tree integrity and notify webhook backends.

**Message Retention Behavior**:
- **Age-based cleanup**: Delete messages older than N days
- **Count-based cleanup**: Keep only last N messages per session
- **Selective cleanup**: Remove non-active conversation branches (unused variants)
- **Tree-aware deletion**: Preserve parent messages required for tree structure
- **Webhook notification**: Backend receives `messages_cleaned` event with deleted message IDs

**Retention Policy Configuration** (per session type):
- `message_retention_days`: Auto-delete messages older than N days (default: null/unlimited)
- `max_messages_per_session`: Keep only last N messages (default: null/unlimited)
- `cleanup_inactive_branches`: Remove unused message variants (default: false)
- `preserve_favorited`: Keep messages marked with feedback reactions (default: true)

**Cleanup Execution**:
- Automated job runs daily to enforce retention policies
- Cleanup preserves active conversation path (marked by is_active=true)
- Parent messages required for tree structure are never deleted (even if old)
- Webhook backends notified asynchronously after cleanup completes

**Use Cases**:
- Reduce storage costs for long-running sessions with thousands of messages
- Comply with data minimization regulations (GDPR, CCPA)
- Clean up experimental branches that users never navigate to
- Archive old conversations while keeping recent context accessible

**Constraints**:
- Cannot delete messages that are parents to active messages (breaks tree)
- Cannot delete messages with pending operations or incomplete streaming
- Cleanup respects session lifecycle state (no cleanup on soft_deleted sessions)

**Integration with Session Retention**:
- Session retention (FR-014e) operates at session level (all-or-nothing)
- Message retention operates within active sessions (selective cleanup)
- When session is deleted, all messages are deleted (session takes precedence)

**Actors**: `cpt-chat-engine-actor-system`, `cpt-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

#### FR-012: Search Session History

- [ ] `p3` - **ID**: `cpt-chat-engine-fr-search-session`

<!-- fdd-id-content -->
The system **MAY** search within a single session's message history and return matching messages with surrounding context. Search supports text matching across all message roles (user and assistant).

**Actors**: `cpt-chat-engine-actor-client`
<!-- fdd-id-content -->

#### FR-013: Search Across Sessions

- [ ] `p3` - **ID**: `cpt-chat-engine-fr-search-sessions`

<!-- fdd-id-content -->
The system **MAY** search across all sessions belonging to a client and return ranked results with session metadata (session ID, title, timestamp, match context). Results are ordered by relevance.

**Actors**: `cpt-chat-engine-actor-client`
<!-- fdd-id-content -->

#### FR-014: Session Lifecycle Management

- [ ] `p1` - **ID**: `cpt-chat-engine-fr-delete-session`

<!-- fdd-id-content -->
The system **MUST** support session lifecycle management with four states: active, archived, soft_deleted, and hard_deleted. Sessions transition through these states based on user actions or retention policies. Each lifecycle transition notifies webhook backends to enable synchronized resource management.

**Lifecycle States:**
- **active** - Normal operational state (default)
- **archived** - Inactive sessions optimized for long-term storage
- **soft_deleted** - Deleted but recoverable within retention period
- **hard_deleted** - Permanently removed from database

**Operations:** Detailed in FR-014a (soft delete), FR-014b (hard delete), FR-014c (restore), FR-014d (archive), and FR-014e (retention policies).

**State Inheritance:** Messages inherit lifecycle_state from their session and transition together to maintain referential integrity.

**Actors**: `cpt-chat-engine-actor-client`, `cpt-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

#### FR-014a: Soft Delete Session (Recoverable)

- [ ] `p1` - **ID**: `cpt-chat-engine-fr-soft-delete-session`

<!-- fdd-id-content -->
The system **MUST** support soft deletion as the default deletion mechanism. Soft-deleted sessions are hidden from normal queries but remain in the system and can be restored within a retention period. The system notifies webhook backends of soft deletion, allowing them to cleanup or suspend associated resources. Sessions automatically transition to permanent deletion after the retention period expires unless restored.

**Actors**: `cpt-chat-engine-actor-client`, `cpt-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

#### FR-014b: Hard Delete Session (Permanent)

- [ ] `p1` - **ID**: `cpt-chat-engine-fr-hard-delete-session`

<!-- fdd-id-content -->
The system **MUST** support permanent hard deletion that irreversibly removes sessions and all associated messages. Hard deletion is triggered explicitly by user request or automatically when soft-deleted sessions reach their retention period expiry. The system notifies webhook backends of permanent deletion, requiring them to cleanup all external resources (files, analytics, indices). This supports data minimization requirements (GDPR, CCPA).

**Actors**: `cpt-chat-engine-actor-client`, `cpt-chat-engine-actor-webhook-backend`, `cpt-chat-engine-actor-system`
<!-- fdd-id-content -->

#### FR-014c: Restore Soft-Deleted Session

- [ ] `p2` - **ID**: `cpt-chat-engine-fr-restore-session`

<!-- fdd-id-content -->
The system **SHOULD** support restoring soft-deleted sessions back to active state. Restoration is only possible before the retention period expires. This enables recovery from accidental deletions. The system notifies webhook backends when sessions are restored, allowing them to reinstate any suspended resources. Hard-deleted sessions cannot be restored.

**Actors**: `cpt-chat-engine-actor-client`, `cpt-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

#### FR-014d: Archive Inactive Sessions

- [ ] `p3` - **ID**: `cpt-chat-engine-fr-archive-session`

<!-- fdd-id-content -->
The system **MAY** support archiving inactive sessions to optimize database performance. Archived sessions remain accessible and queryable but may have reduced query performance. Archival can be triggered manually or automatically based on inactivity period. The system notifies webhook backends of lifecycle state changes. Archived sessions can transition back to active state when new activity occurs or be deleted.

**Actors**: `cpt-chat-engine-actor-client`, `cpt-chat-engine-actor-webhook-backend`, `cpt-chat-engine-actor-system`
<!-- fdd-id-content -->

#### FR-014e: Retention Policy Configuration and Enforcement

- [ ] `p2` - **ID**: `cpt-chat-engine-fr-retention-policy`

<!-- fdd-id-content -->
The system **SHOULD** support configurable retention policies that automatically manage session lifecycle based on age and inactivity. Retention policies enable automated data lifecycle management while balancing storage costs and compliance requirements. Policies are configured per session type and control automatic archival of inactive sessions, automatic hard deletion of soft-deleted sessions after grace period, and optional immediate deletion for compliance scenarios. The system processes retention policies periodically and notifies webhook backends of all lifecycle transitions.

**Actors**: `cpt-chat-engine-actor-system`, Admin
<!-- fdd-id-content -->

#### FR-015: WebSocket Protocol Support

- [ ] `p2` - **ID**: `cpt-chat-engine-fr-websocket-protocol`

<!-- fdd-id-content -->
The system **SHOULD** support WebSocket protocol as an alternative to HTTP streaming for client-server communication. Clients can connect via WebSocket and perform all operations (session management, message sending, streaming responses) over a persistent connection instead of HTTP REST endpoints.

**Protocol Features**:
- Persistent bidirectional connection using WebSocket (RFC 6455)
- JSON message framing for commands and NDJSON for streaming chunks
- Connection lifecycle management (authenticate, heartbeat, reconnect)
- All HTTP REST operations available via WebSocket commands
- Graceful degradation to HTTP if WebSocket unavailable

**Use Cases**:
- Clients preferring WebSocket client libraries
- Lower latency for rapid message exchanges
- Future bidirectional features (typing indicators, presence, notifications)
- Mobile apps with persistent connections

**Trade-offs**:
- Requires sticky session configuration for load balancing
- Adds connection state management complexity
- WebSocket proxy configuration needed in deployment
- Not compatible with serverless architectures

**Actors**: `cpt-chat-engine-actor-client`, `cpt-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

## 4. Use Cases

#### UC-001: Create Session and Send First Message

**ID**: `cpt-chat-engine-usecase-create-session`

<!-- fdd-id-content -->
**Actor**: `cpt-chat-engine-actor-client`

**Preconditions**: Client has valid session type ID and client ID

**Flow**:
1. Client requests session creation with session type ID and client ID
2. System creates session record in database with unique session ID
3. System notifies webhook backend of session creation with session metadata
4. Backend processes creation notification and returns available capabilities (file attachments, session switching, summarization, etc.)
5. System stores capabilities in session record and returns session ID to client
6. Client sends first message with capabilities indicating which features are enabled
7. System validates capabilities against stored session capabilities
8. System forwards message to backend with full context (session metadata, capabilities, empty message history)
9. Backend processes message and streams response
10. System streams response chunks to client in real-time
11. System stores complete message exchange in database

**Postconditions**: Session created with unique ID, capabilities stored, first message exchanged and persisted

**Acceptance criteria**:
- Session ID returned to client within 200ms of creation request
- Capabilities list correctly stored and accessible for subsequent messages
- First message routed to correct webhook backend based on session type
- Streaming response delivered to client without data loss
- Complete message exchange persisted in database before acknowledgment
<!-- fdd-id-content -->

#### UC-002: Recreate Assistant Response

**ID**: `cpt-chat-engine-usecase-recreate-response`

<!-- fdd-id-content -->
**Actor**: `cpt-chat-engine-actor-client`

**Preconditions**: Session exists with at least one assistant message

**Flow**:
1. Client requests recreation of last assistant response, specifying message ID
2. System validates that the specified message exists and is an assistant message
3. System identifies the parent message of the assistant message to recreate
4. System loads message history up to and including the parent message
5. System sends recreation request to webhook backend with context (message history, session metadata, capabilities)
6. Backend generates new response based on context
7. System streams new response chunks to client in real-time
8. System stores new response as a sibling of the original response (same parent message ID)
9. System marks the new response as the active variant
10. System returns variant information to client (e.g., "variant 2 of 2")

**Postconditions**: New response variant created and stored, both variants preserved and navigable, new variant marked as active

**Acceptance criteria**:
- Old response remains unchanged in database
- New response has same parent message ID as old response
- Client receives variant position information
- Both variants can be retrieved and navigated
- Message tree integrity maintained (no orphaned messages)
<!-- fdd-id-content -->

#### UC-003: Branch from Historical Message

**ID**: `cpt-chat-engine-usecase-branch-message`

<!-- fdd-id-content -->
**Actor**: `cpt-chat-engine-actor-client`

**Preconditions**: Session exists with message history containing at least one message

**Flow**:
1. Client selects a message in history to branch from (parent message ID)
2. Client sends new message with specified parent message ID
3. System validates parent message exists in session
4. System loads message history from session start up to and including parent message
5. System forwards message to webhook backend with truncated context
6. Backend processes message with historical context (ignoring messages after parent)
7. System streams response chunks to client in real-time
8. System stores new message with parent reference
9. System stores assistant response with new message as parent
10. System marks new branch as active path
11. Client can navigate between original path and new branch

**Postconditions**: New conversation branch created starting from specified message, both paths preserved, new branch marked as active

**Acceptance criteria**:
- New message has correct parent message ID reference
- Context sent to backend includes only messages up to parent
- Both conversation branches preserved in database
- Both branches navigable by client
- No data loss in original conversation path
- Message tree structure maintains referential integrity
<!-- fdd-id-content -->

#### UC-004: Export Session

**ID**: `cpt-chat-engine-usecase-export-session`

<!-- fdd-id-content -->
**Actor**: `cpt-chat-engine-actor-client`

**Preconditions**: Session exists with at least one message

**Flow**:
1. Client requests export with specified format (JSON, Markdown, or TXT) and scope (active path only or all variants)
2. System validates session exists and client has access
3. System retrieves session messages according to scope:
   - Active path only: follows current active variant chain
   - All variants: retrieves entire message tree
4. System formats data according to requested format:
   - JSON: structured data with message tree relationships
   - Markdown: human-readable format with message roles and content
   - TXT: plain text format with minimal formatting
5. System generates formatted file content
6. System uploads formatted file to file storage service
7. File storage returns signed URL with expiration
8. System returns download URL to client

**Postconditions**: Session exported to requested format, file uploaded to storage, download URL provided

**Acceptance criteria**:
- Export completes within 5 seconds for sessions with <1000 messages
- All message variants included if "all variants" scope requested
- Active path only includes messages in current variant chain if "active path" scope requested
- Generated file is valid and parseable according to format
- Download URL is accessible and valid for at least 24 hours
- File content accurately represents session data without loss
<!-- fdd-id-content -->

#### UC-005: Share Session

**ID**: `cpt-chat-engine-usecase-share-session`

<!-- fdd-id-content -->
**Actor**: `cpt-chat-engine-actor-client`, `cpt-chat-engine-actor-end-user`

**Preconditions**: Session exists with at least one message

**Flow**:
1. Client requests shareable link for session
2. System generates unique share token and associates it with session ID
3. System returns shareable URL containing share token
4. Client shares URL with recipient
5. Recipient opens shared URL in client application
6. Client application sends share token to system
7. System validates share token and retrieves associated session ID
8. System returns session data in read-only mode to recipient
9. Recipient views session messages
10. Recipient sends new message in shared session
11. System creates new message branching from last message in session
12. System routes message to webhook backend with full history
13. Backend processes message and returns response
14. System stores new branch separately from original session path

**Postconditions**: Session shared via unique URL, recipient can view original messages and create branches, original session remains unchanged

**Acceptance criteria**:
- Share token is unique, secure, and not guessable
- Original session data cannot be modified by recipient
- Recipient's messages create new branch in message tree
- Recipient cannot modify or delete original messages
- Original session owner can still access and modify their conversation path
- Share link can be revoked by original owner
<!-- fdd-id-content -->

#### UC-006: Send Message and Receive Streaming Response

**ID**: `cpt-chat-engine-usecase-send-message`

<!-- fdd-id-content -->
**Actor**: `cpt-chat-engine-actor-client`

**Preconditions**: Session exists in active state; client has valid session ID

**Flow**:
1. Client sends message with content and optional file attachment UUIDs
2. System validates session is active and client owns or has access to the session
3. System validates file UUIDs against session capabilities (if file attachments enabled)
4. System persists user message to database and assigns message ID
5. System loads full message history for the session (respecting `is_hidden_from_llm` flags)
6. System forwards message to webhook backend with: session metadata, capabilities, message history, new message content
7. Backend begins processing and streams response chunks
8. System forwards each chunk to client in real-time
9. Upon stream completion, system persists assistant message to database
10. System acknowledges message exchange to client with message IDs

**Postconditions**: User and assistant messages persisted; client receives complete streaming response

**Alternative Flows**:
- **Client cancels mid-stream**: System stops forwarding, saves partial response with incomplete status (see `cpt-chat-engine-fr-stop-streaming`)
- **Webhook backend timeout**: System closes stream, saves error message with timeout metadata, returns 504 to client
- **Webhook backend returns error**: System saves error message, propagates structured error to client
<!-- fdd-id-content -->

#### UC-007: Delete Session

**ID**: `cpt-chat-engine-usecase-delete-session`

<!-- fdd-id-content -->
**Actor**: `cpt-chat-engine-actor-client`

**Preconditions**: Session exists; client owns the session

**Flow (Soft Delete)**:
1. Client requests session soft-deletion
2. System validates ownership (client ID matches session owner)
3. System transitions session to `soft_deleted` state
4. System hides session from normal queries
5. System notifies webhook backend of soft-deletion event
6. System returns success to client with retention period expiry timestamp

**Flow (Hard Delete)**:
1. Client requests permanent hard-deletion (or retention period expires)
2. System transitions session to `hard_deleted` state
3. System permanently removes all session messages and metadata from database
4. System notifies webhook backend with `session.hard_deleted` event (backend must clean up external resources)
5. System returns success to client

**Postconditions**: Session hidden (soft) or permanently removed (hard); webhook backend notified

**Alternative Flows**:
- **Client requests restore within retention period**: Session transitions back to `active` (see `cpt-chat-engine-fr-restore-session`)
- **Session not found**: System returns 404
- **Client does not own session**: System returns 403 Forbidden
<!-- fdd-id-content -->

#### UC-008: Backend Failure During Streaming

**ID**: `cpt-chat-engine-usecase-backend-failure`

<!-- fdd-id-content -->
**Actor**: `cpt-chat-engine-actor-client`, `cpt-chat-engine-actor-webhook-backend`

**Preconditions**: Session active; message forwarded to backend; streaming in progress

**Flow**:
1. Backend connection drops or returns an error mid-stream
2. System detects connection failure or error response from backend
3. System stops forwarding chunks to client
4. System saves partial response with `incomplete` status and error metadata
5. System sends structured error event to client indicating streaming failure
6. Session remains in `active` state — client can retry or branch

**Postconditions**: Partial assistant message saved with error metadata; client notified of failure; session remains operational

**Alternative Flows**:
- **Timeout before first byte**: System returns 504 to client; no assistant message saved
- **Backend returns 503/429 (rate limit)**: System logs backend health event; client receives retryable error with backoff hint
- **All retries exhausted** (if retry configured for session type): System marks session backend as degraded; client can still read history (degraded mode per `cpt-chat-engine-nfr-availability`)
<!-- fdd-id-content -->

## 5. Non-functional Requirements

#### NFR-001: Response Time

- [ ] `p1` - **ID**: `cpt-chat-engine-nfr-response-time`

<!-- fdd-id-content -->
Message routing latency must be less than 100ms at p95, measured from receiving client message to forwarding to webhook backend (excluding backend processing time). Session creation must complete within 200ms at p95, including database write and backend notification.
<!-- fdd-id-content -->

#### NFR-002: Availability

- [ ] `p1` - **ID**: `cpt-chat-engine-nfr-availability`

<!-- fdd-id-content -->
System must maintain 99.9% uptime for session management operations (create, retrieve, delete sessions). During webhook backend failures, the system must support degraded mode with read-only access to session history. Planned maintenance windows must be scheduled during low-traffic periods with advance notice.
<!-- fdd-id-content -->

#### NFR-003: Scalability

- [ ] `p1` - **ID**: `cpt-chat-engine-nfr-scalability`

<!-- fdd-id-content -->
System must support at least 10,000 concurrent active sessions per instance. Message throughput must support at least 1,000 messages per second per instance. System must support horizontal scaling by adding instances without shared state constraints.
<!-- fdd-id-content -->

#### NFR-004: Data Persistence

- [ ] `p1` - **ID**: `cpt-chat-engine-nfr-data-persistence`

<!-- fdd-id-content -->
All messages must be persisted to database before sending acknowledgment to client. Zero message loss is required during system failures, network interruptions, or backend failures. Database writes must use ACID transactions to ensure consistency.
<!-- fdd-id-content -->

#### NFR-005: Streaming Performance

- [ ] `p1` - **ID**: `cpt-chat-engine-nfr-streaming`

<!-- fdd-id-content -->
Streaming latency overhead (time between receiving chunk from backend and forwarding to client) must be less than 10ms at p95. First byte of streamed response must arrive at client within 200ms of backend starting to stream. Streaming must support backpressure to handle slow clients.
<!-- fdd-id-content -->

#### NFR-006: Authentication

- [ ] `p1` - **ID**: `cpt-chat-engine-nfr-authentication`

<!-- fdd-id-content -->
System must authenticate all client requests using JWT bearer tokens. Each token must carry `client_id`, `user_id`, and `tenant_id` claims, all extracted server-side and never accepted from request body. Session access must be restricted to the owning user (`user_id` match) within the owning tenant (`tenant_id` match), or to share token holders for read-only access. All data queries must be scoped by `tenant_id` to ensure tenant isolation.
<!-- fdd-id-content -->

#### NFR-007: Data Integrity

- [ ] `p1` - **ID**: `cpt-chat-engine-nfr-data-integrity`

<!-- fdd-id-content -->
Message tree structure must maintain referential integrity at all times. Orphaned messages (messages with non-existent parent) are not allowed. Parent-child relationships must be immutable once created. Database constraints must enforce tree structure integrity.
<!-- fdd-id-content -->

#### NFR-008: Backend Isolation

- [ ] `p1` - **ID**: `cpt-chat-engine-nfr-backend-isolation`

<!-- fdd-id-content -->
Webhook backend failures must not affect other sessions using different backends. Request timeout must be configurable per session type with a default of 30 seconds. Backend errors must be isolated and logged without cascading to other system components.
<!-- fdd-id-content -->

#### NFR-009: File Size Limits

- [ ] `p1` - **ID**: `cpt-chat-engine-nfr-file-size`

<!-- fdd-id-content -->
System must enforce file size limits with a default of 10MB per individual file. Total attachments per message must be limited to 50MB. File size validation occurs at client upload time (enforced by file storage service) and limits are configurable per session type.
<!-- fdd-id-content -->

#### NFR-010: Search Performance

- [ ] `p2` - **ID**: `cpt-chat-engine-nfr-search`

<!-- fdd-id-content -->
Session history search must return results within 1 second at p95 for sessions with up to 10,000 messages. Cross-session search must return results within 3 seconds at p95 for clients with up to 1,000 sessions. Search must support pagination for large result sets.
<!-- fdd-id-content -->

#### NFR-011: WebSocket Performance

- [ ] `p2` - **ID**: `cpt-chat-engine-nfr-websocket-performance`

<!-- fdd-id-content -->
When WebSocket is enabled, connection establishment must complete within 500ms at p95. Message routing latency over WebSocket must be less than 50ms at p95 (lower than HTTP's 100ms target). Heartbeat interval must be 30 seconds with automatic reconnection using exponential backoff (maximum 60 seconds). The system must support at least 5,000 concurrent WebSocket connections per instance.
<!-- fdd-id-content -->

#### NFR-012: WebSocket Reliability

- [ ] `p2` - **ID**: `cpt-chat-engine-nfr-websocket-reliability`

<!-- fdd-id-content -->
When WebSocket is enabled, connections must support automatic reconnection with state restoration after network interruptions. Message delivery guarantees must match HTTP protocol (at-least-once for operations, exactly-once for streaming). The system must handle graceful connection closure with pending operation completion or cancellation. Connection timeout must be 5 minutes for idle connections, configurable per deployment.
<!-- fdd-id-content -->

#### NFR-013: Message History Handling

- [ ] `p2` - **ID**: `cpt-chat-engine-nfr-message-history`

<!-- fdd-id-content -->
System must support sessions with up to 10,000 messages without performance degradation. Message history forwarding to webhook backends must complete within 2 seconds at p95 for sessions with 1,000 messages. Backends must implement conversation memory management strategies when approaching context window limits (typically 4,000-100,000 tokens depending on LLM model). System must provide message count and estimated token count in session metadata to help backends make memory management decisions.
<!-- fdd-id-content -->

#### NFR-014: Lifecycle Operation Performance

- [ ] `p2` - **ID**: `cpt-chat-engine-nfr-lifecycle-performance`

<!-- fdd-id-content -->
Lifecycle operations (soft delete, restore, archive) must complete within 500ms at p95 for sessions with up to 10,000 messages. Hard delete operations may take up to 5 seconds at p95 for large sessions. Restoration must preserve complete session state including message tree structure, metadata, and file references. Lifecycle state transitions must be atomic.
<!-- fdd-id-content -->

#### NFR-015: Retention Policy Enforcement SLA

- [ ] `p2` - **ID**: `cpt-chat-engine-nfr-retention-sla`

<!-- fdd-id-content -->
Automatic retention policy enforcement must run at least daily. Sessions must transition to permanent deletion within 24 hours of reaching their retention period expiry. Policy processing must handle at least 10,000 sessions per run without impacting production query performance (p95 latency increase <10%). Failed operations must retry and alert on repeated failures.
<!-- fdd-id-content -->

#### NFR-016: Recovery Requirements

- [ ] `p2` - **ID**: `cpt-chat-engine-nfr-recovery`

<!-- fdd-id-content -->
Recovery objectives for Chat Engine persistent data:

- **RPO (Recovery Point Objective)**: ≤ 5 minutes — maximum acceptable data loss window in the event of a catastrophic failure
- **RTO (Recovery Time Objective)**: ≤ 30 minutes — maximum acceptable downtime before service is restored to degraded mode; ≤ 2 hours for full recovery
- **Backup frequency**: Session and message data must be backed up at minimum every 5 minutes via continuous WAL shipping or equivalent
- **Backup retention**: Backups must be retained for at least 30 days
- **Point-in-time recovery**: Database must support point-in-time recovery to any point within the backup retention window
- **Atomic lifecycle transitions**: All session lifecycle state transitions must be ACID-compliant; partial transitions are not acceptable
<!-- fdd-id-content -->

#### NFR-017: Developer Experience

- [ ] `p2` - **ID**: `cpt-chat-engine-nfr-developer-experience`

<!-- fdd-id-content -->
Chat Engine's primary users are Application Developers and Webhook Backend Developers. Integration quality is a core product metric:

- **Time-to-first-message**: A developer familiar with REST APIs must be able to send a first message within ≤ 30 minutes of reading the API documentation, without prior Chat Engine knowledge
- **Error response quality**: All API errors must return structured responses with: machine-readable error code, human-readable message, and actionable remediation hint
- **API documentation**: A complete OpenAPI specification must be published and kept up-to-date with every API change
- **Webhook contract documentation**: Webhook backend developers must have a documented contract covering all event types, payload schemas, and expected response formats
- **Client SDK**: At minimum one reference client SDK must be provided (language TBD) demonstrating session creation, message exchange, and streaming
<!-- fdd-id-content -->

## 6. Additional Context

#### Integration with Webhook Backends

**ID**: `cpt-chat-engine-prd-context-webhook-integration`

<!-- fdd-id-content -->
Webhook backends are expected to be HTTP services that receive session context (session metadata, capabilities, message history) and return responses. Backends are responsible for all message processing logic, enabling flexible implementations including AI chat (LLMs), rule-based systems, human-in-the-loop support, or hybrid approaches. The webhook contract is designed to be backend-agnostic, allowing easy experimentation with different processing approaches.
<!-- fdd-id-content -->

#### Message Tree Structure

**ID**: `cpt-chat-engine-prd-context-message-tree`

<!-- fdd-id-content -->
Messages form a tree structure where each message (except the root) references a parent message. This tree structure enables conversation branching and message variant preservation. Multiple sibling messages with the same parent represent variants (alternative responses). The client application is responsible for rendering the tree structure in UI and providing navigation controls. The system maintains tree integrity but does not enforce a specific UI representation.
<!-- fdd-id-content -->

#### Message Visibility Control

**ID**: `cpt-chat-engine-prd-context-message-visibility`

<!-- fdd-id-content -->
Messages can be selectively hidden from users or LLMs using visibility flags:

- **`is_hidden_from_user`** (boolean): When true, the message is excluded from client-facing APIs and UI rendering. The message remains in the database and message tree but is not returned to clients. Use cases include system prompts, backend configuration messages, and internal tracking notes.

- **`is_hidden_from_llm`** (boolean): When true, the message is excluded from the context history sent to webhook backends during message processing. The message is still visible to users (unless also hidden via `is_hidden_from_user`) but does not influence LLM responses. Use cases include user feedback, debug messages, and messages that should not affect conversation context.

These flags enable flexible message handling patterns:
- **System prompts**: `is_hidden_from_user=true, is_hidden_from_llm=false` - Configure LLM behavior without showing configuration to users
- **Internal notes**: `is_hidden_from_user=true, is_hidden_from_llm=true` - Store metadata or debug information without affecting UI or LLM
- **User feedback**: `is_hidden_from_user=false, is_hidden_from_llm=true` - Show user messages in UI but exclude from LLM context (e.g., rating messages)
- **Normal messages**: `is_hidden_from_user=false, is_hidden_from_llm=false` - Standard visible messages that are part of conversation flow
<!-- fdd-id-content -->

#### Conversation Memory Management

**ID**: `cpt-chat-engine-prd-context-memory-management`

<!-- fdd-id-content -->
Chat Engine forwards complete message history to webhook backends by default, enabling backends to implement their own memory management strategies. For long conversations that exceed LLM context window limits, backends should implement strategies such as sliding windows, summarization, or importance filtering.

The system provides building blocks for memory management:
- **Session Summary (FR-011)**: Request conversation summaries at any point
- **Message Visibility Flags**: Mark messages to exclude from LLM context
- **Branching (FR-006)**: Create new conversation paths with truncated history
- **Session Metadata**: Store strategy state and configuration

Backends are responsible for:
- Monitoring conversation length and token usage
- Choosing appropriate strategy for session type
- Implementing token counting and context optimization
- Storing strategy state in session metadata

Common strategies include sending only recent messages (sliding window), summarizing older messages while keeping recent ones verbatim, or filtering messages by semantic importance.
<!-- fdd-id-content -->

#### Session Lifecycle State Flow

**ID**: `cpt-chat-engine-prd-context-lifecycle-flow`

<!-- fdd-id-content -->
Sessions and messages progress through four lifecycle states that control visibility, accessibility, and storage optimization:

**Lifecycle States:**

1. **active** (default) - Normal operational state. Sessions are visible in queries and fully accessible. Messages can be sent and received.

2. **archived** - Inactive sessions optimized for long-term storage. Sessions remain queryable but may have reduced performance.

3. **soft_deleted** - Deleted but recoverable. Sessions are hidden from normal queries but remain in the system. Can be restored before retention period expires.

4. **hard_deleted** - Permanently removed. Cannot be recovered.

**State Transition Flows:**

Common transitions:
- active → soft_deleted (user deletion) → hard_deleted (retention policy or explicit)
- active → archived (inactivity) → soft_deleted (deletion) → hard_deleted
- soft_deleted → active (restoration, before expiry)
- archived → active (new activity or manual restore)

**State Inheritance:**
Messages inherit lifecycle state from their session. When a session transitions, all its messages transition together to maintain referential integrity.

**Webhook Events:**
The system notifies webhook backends of all lifecycle transitions (`session.soft_deleted`, `session.hard_deleted`, `session.restored`, `session.lifecycle_changed`) to enable synchronized resource management.
<!-- fdd-id-content -->

#### Retention Policy Design Philosophy

**ID**: `cpt-chat-engine-prd-context-retention-philosophy`

<!-- fdd-id-content -->
Retention policies enable automated data lifecycle management while balancing user safety, storage costs, and compliance requirements. The design prioritizes safety and flexibility over aggressive data deletion.

**Design Principles:**

1. **Safety by Default**
   - Soft delete is the default deletion mechanism
   - Grace period before permanent deletion protects against accidental data loss
   - Hard delete requires explicit action or policy configuration

2. **Flexibility Over Rigidity**
   - Policies configured per session type (not global)
   - Policies can be disabled for manual lifecycle management
   - Different retention periods for different use cases

3. **Compliance Support**
   - Automatic hard delete supports data minimization (GDPR, CCPA)
   - Configurable retention periods meet regulatory requirements
   - Audit trail via webhook events for compliance reporting
   - Immediate deletion option for right-to-erasure requests

4. **Performance Optimization**
   - Archival separates active and inactive data
   - Automatic cleanup reduces storage growth over time
   - Lifecycle operations maintain system performance at scale

**Use Cases:**
- **Temporary chat**: Short inactivity threshold, moderate retention period
- **Support tickets**: Long inactivity threshold, extended retention for audit
- **Legal compliance**: Minimal retention, automatic cleanup enabled
- **User data (GDPR)**: Moderate thresholds, automatic cleanup for data minimization
<!-- fdd-id-content -->

#### Assumptions

**ID**: `cpt-chat-engine-prd-context-assumptions`

<!-- fdd-id-content -->
Key assumptions underlying this PRD:
- Webhook backends are always HTTP-accessible from Chat Engine instances
- Client applications handle all UI rendering of message trees and conversation visualization
- File storage service provides signed URL access with configurable expiration
- Database service supports ACID transactions and can handle write loads from concurrent sessions
- Network between Chat Engine and webhook backends is reliable (same region/VPC preferred)
- Client applications handle user authentication and pass validated client IDs to Chat Engine
- Webhook backends have reasonable response times (<30 seconds for most operations)
<!-- fdd-id-content -->

#### Out of Scope (Non-Goals)

**ID**: `cpt-chat-engine-prd-context-non-goals`

<!-- fdd-id-content -->
The following are explicitly out of scope for Chat Engine:
- Message content processing, analysis, or moderation (handled by webhook backends)
- User authentication and identity management (handled by client applications)
- File upload/download implementation (handled by external file storage service)
- UI rendering and conversation visualization (handled by client applications)
- Rate limiting per user or organization (handled by client applications or API gateway)
- Billing, usage tracking, and quota management (separate service)
- Real-time collaboration features (multiple users in same session)
- Message encryption at rest (delegated to database service)
- Content delivery network (CDN) integration for file serving
<!-- fdd-id-content -->

#### Risks

**ID**: `cpt-chat-engine-prd-context-risks`

<!-- fdd-id-content -->
Identified risks and mitigation strategies:
- **Webhook Backend Latency**: Slow backends directly impact user experience. Mitigation: configurable timeouts per session type, monitoring and alerting for slow backends, consider caching for idempotent operations.
- **Database Contention**: High message volume may cause database write contention and slow queries. Mitigation: read replicas for query operations, connection pooling, query optimization, consider sharding by client ID.
- **Message Tree Complexity**: Deep branching (many variants or deep trees) may impact query performance and UI rendering. Mitigation: implement depth limits, pagination for variant navigation, database indexing on parent relationships.
- **File Storage Costs**: Unrestricted file attachments may lead to high storage costs. Mitigation: enforce file size limits, implement retention policies, consider compression for certain file types.
- **Session Abandonment**: Large numbers of inactive sessions may consume database resources. Mitigation: implement session cleanup policies, archive old sessions, monitor active session metrics.
<!-- fdd-id-content -->

#### Privacy by Design

**ID**: `cpt-chat-engine-prd-context-privacy`

<!-- fdd-id-content -->
Chat Engine processes user messages and user identifiers on behalf of client applications. Privacy requirements are embedded by design:

**Personal Data Handled**:
- User identifiers (client IDs passed by client applications)
- Message content (text, file attachment UUIDs)
- Per-message feedback (reaction type, optional comment text)
- Session metadata (timestamps, session type, lifecycle state)

**Data Minimization**: Chat Engine collects only the data operationally required to route messages and maintain session state. No analytics, profiling, or secondary use of message content occurs within Chat Engine.

**Purpose Limitation**: Message content is forwarded to webhook backends for processing purposes only. Chat Engine does not analyse or index message content for any other purpose.

**Privacy by Default**: Optional data collection (feedback comments, session metadata fields) is disabled unless explicitly enabled by session type capabilities.

**Data Subject Rights Support**: Hard-delete (`cpt-chat-engine-fr-hard-delete-session`) supports the right to erasure (GDPR Art. 17). Client applications are responsible for accepting erasure requests from end users and forwarding them to Chat Engine.

**Responsibility Boundary**: Chat Engine acts as a **data processor** on behalf of client applications (the data controllers). Client applications are responsible for obtaining valid legal basis for processing user messages and for data subject consent where required.
<!-- fdd-id-content -->

#### Data Ownership

**ID**: `cpt-chat-engine-prd-context-data-ownership`

<!-- fdd-id-content -->
**Data Controller**: The client application that creates sessions and sends messages. The client application is responsible for obtaining user consent and establishing the legal basis for processing message content.

**Data Processor**: Chat Engine acts as a data processor on behalf of the client application. Chat Engine processes message data solely as instructed by the client application via the API.

**User-Generated Content**: Message content is owned by the end user who authored it, as represented through the client application. Chat Engine makes no claim to ownership of message content.

**Data Processing Agreement**: Client applications deploying Chat Engine in environments subject to GDPR or equivalent regulations must establish a Data Processing Agreement (DPA) governing Chat Engine's processing role.

**Third-Party Processors**: Webhook backends receive message content from Chat Engine. Client applications are responsible for ensuring their webhook backends also operate under appropriate data processing agreements.
<!-- fdd-id-content -->

## 7. Intentional Exclusions

The following checklist categories are **not applicable** to this PRD. Each is explicitly excluded with reasoning to distinguish intentional omission from oversight.

| Category | Status | Reason |
|----------|--------|--------|
| **Safety (SAFE-PRD-001/002)** | N/A | Chat Engine is a pure information API service with no physical interaction, no hardware control, and no potential for physical harm. ISO 25010:2023 Safety characteristic does not apply. |
| **Accessibility (UX-PRD-002)** | N/A | Chat Engine exposes a server-side REST/WebSocket API only — no user interface. Accessibility standards (WCAG) apply to client applications built on top of Chat Engine, not to Chat Engine itself. |
| **Internationalization (UX-PRD-003)** | N/A | Chat Engine is message-content-agnostic. It stores and forwards opaque text without interpreting language, encoding, or locale. I18n is the responsibility of client applications and webhook backends. |
| **Inclusivity (UX-PRD-005)** | N/A | Chat Engine has no user interface. Inclusivity concerns apply to client applications. |
| **Market Positioning (BIZ-PRD-002)** | N/A | Chat Engine is an internal platform module, not a market-facing product. Competitive analysis and market positioning are not applicable. |
| **Documentation Requirements (MAINT-PRD-001)** | Addressed in NFR-017 | Developer documentation, API spec, and webhook contract documentation are covered under `cpt-chat-engine-nfr-developer-experience`. |
| **Support Requirements (MAINT-PRD-002)** | Deferred | Support tier SLAs are defined at the Hyperspot platform level, not per-module. Chat Engine inherits platform-wide support policies. |
| **Deployment Requirements (OPS-PRD-001)** | Deferred | Deployment environment, release cadence, and rollback policies are defined in the Hyperspot platform-level PRD and infrastructure documentation. Chat Engine inherits these. |
| **Monitoring Requirements (OPS-PRD-002)** | Deferred | Alerting, dashboards, and log retention are governed by the Hyperspot platform observability standards. Chat Engine must emit standard structured logs and metrics — specifics defined in DESIGN. |
| **Industry Standards (COMPL-PRD-002)** | Partial | Applicable standards are referenced inline: GDPR (Art. 17, 25), CCPA, and ACID transaction guarantees. No formal certification (ISO 27001, SOC 2) is currently required. |
