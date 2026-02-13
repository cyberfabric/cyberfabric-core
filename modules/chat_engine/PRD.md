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

## 2. Actors

### 2.1 Human Actors

#### Client Application Developer

**ID**: `fdd-chat-engine-actor-developer`

<!-- fdd-id-content -->
**Role**: Integrates Chat Engine into applications by configuring session types, implementing client-side UI for message display and navigation, and managing user authentication and file uploads.
<!-- fdd-id-content -->

#### End User

**ID**: `fdd-chat-engine-actor-end-user`

<!-- fdd-id-content -->
**Role**: Interacts with client applications built on Chat Engine, sending messages, receiving responses, and navigating conversation variants (indirect actor, does not directly interact with Chat Engine).
<!-- fdd-id-content -->

#### Webhook Backend Developer

**ID**: `fdd-chat-engine-actor-backend-developer`

<!-- fdd-id-content -->
**Role**: Implements webhook backends that receive session context and messages from Chat Engine, process them according to custom logic (AI, rules, human-in-the-loop), and return responses.
<!-- fdd-id-content -->

### 2.2 System Actors

#### Client Application

**ID**: `fdd-chat-engine-actor-client`

<!-- fdd-id-content -->
**Role**: Frontend application (web, mobile, desktop) that sends messages to Chat Engine, receives streaming responses, and renders conversation UI including message trees and variants.
<!-- fdd-id-content -->

#### Webhook Backend

**ID**: `fdd-chat-engine-actor-webhook-backend`

<!-- fdd-id-content -->
**Role**: External HTTP service that processes messages and returns responses. Receives full session context, message history, and capabilities from Chat Engine. Implements custom message processing logic.
<!-- fdd-id-content -->

#### File Storage Service

**ID**: `fdd-chat-engine-actor-file-storage`

<!-- fdd-id-content -->
**Role**: External file storage service (e.g., S3, GCS) that stores file attachments. Provides signed URL access for file upload and download. Client applications upload files directly to storage.
<!-- fdd-id-content -->

#### Database Service

**ID**: `fdd-chat-engine-actor-database`

<!-- fdd-id-content -->
**Role**: Persistent storage for sessions, messages, message tree structures, and metadata. Supports ACID transactions to ensure data integrity and consistency.
<!-- fdd-id-content -->

## 3. Functional Requirements

#### FR-001: Create Session

- [ ] `p1` - **ID**: `fdd-chat-engine-fr-create-session`

<!-- fdd-id-content -->
The system **MUST** create a new session with a specified session type and client ID. The system notifies the webhook backend of the new session and receives available capabilities for that session type. The capabilities determine which features are enabled (file attachments, session switching, summarization, etc.).

**Actors**: `fdd-chat-engine-actor-client`, `fdd-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

#### FR-002: Send Message with Streaming Response

- [ ] `p1` - **ID**: `fdd-chat-engine-fr-send-message`

<!-- fdd-id-content -->
The system **MUST** forward user messages to webhook backend with full session context (session metadata, capabilities, message history) and stream responses back to client in real-time. The system persists the complete message exchange (user message and assistant response) after streaming completes.

**Actors**: `fdd-chat-engine-actor-client`, `fdd-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

#### FR-003: Attach Files to Messages

- [ ] `p1` - **ID**: `fdd-chat-engine-fr-attach-files`

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

**Actors**: `fdd-chat-engine-actor-client`, `fdd-chat-engine-actor-file-storage`
<!-- fdd-id-content -->

#### FR-004: Switch Session Type

- [ ] `p2` - **ID**: `fdd-chat-engine-fr-switch-session-type`

<!-- fdd-id-content -->
The system **SHOULD** allow switching to a different session type mid-session. When switching occurs, the next message is routed to the new webhook backend with full message history. The new backend returns updated capabilities which apply for subsequent messages.

**Actors**: `fdd-chat-engine-actor-client`, `fdd-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

#### FR-005: Recreate Assistant Response

- [ ] `p1` - **ID**: `fdd-chat-engine-fr-recreate-response`

<!-- fdd-id-content -->
The system **MUST** allow regeneration of assistant responses. When recreation is requested, the old response is preserved as a variant in the message tree, and a new response is generated and stored as a sibling (same parent, different branch). Both variants remain accessible for navigation.

**Actors**: `fdd-chat-engine-actor-client`, `fdd-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

#### FR-006: Branch from Message

- [ ] `p2` - **ID**: `fdd-chat-engine-fr-branch-message`

<!-- fdd-id-content -->
The system **SHOULD** allow creating new messages from any point in conversation history, creating alternative conversation paths. When branching, the system loads context up to the specified parent message and forwards the new message to the backend with truncated history. Both conversation branches remain preserved.

**Actors**: `fdd-chat-engine-actor-client`, `fdd-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

#### FR-007: Navigate Message Variants

- [ ] `p2` - **ID**: `fdd-chat-engine-fr-navigate-variants`

<!-- fdd-id-content -->
The system **SHOULD** allow navigation between message variants (siblings with same parent message). When retrieving messages, the system provides variant position information (e.g., "2 of 3") and allows clients to request specific variants.

Webhook backends receive message history with file_ids (UUIDs). Backends must implement File Storage Service client to fetch file content when needed.

**Actors**: `fdd-chat-engine-actor-client`
<!-- fdd-id-content -->

#### FR-008: Stop Streaming Response

- [ ] `p1` - **ID**: `fdd-chat-engine-fr-stop-streaming`

<!-- fdd-id-content -->
The system **MUST** allow canceling streaming responses mid-generation. When cancellation occurs, the system stops forwarding data from webhook backend, closes the connection, and saves the partial response as an incomplete message with appropriate metadata.

**Actors**: `fdd-chat-engine-actor-client`
<!-- fdd-id-content -->

#### FR-009: Export Session

- [ ] `p3` - **ID**: `fdd-chat-engine-fr-export-session`

<!-- fdd-id-content -->
The system **MAY** export sessions in JSON, Markdown, or TXT format. Export can include only the active conversation path or all message variants. The system uploads the formatted export to file storage and returns a download URL.

**Actors**: `fdd-chat-engine-actor-client`
<!-- fdd-id-content -->

#### FR-010: Share Session

- [ ] `p3` - **ID**: `fdd-chat-engine-fr-share-session`

<!-- fdd-id-content -->
The system **MAY** generate shareable links for sessions. Recipients can view sessions in read-only mode and create branches from the last message in the session. Branches created by recipients do not affect the original session owner's conversation path.

**Actors**: `fdd-chat-engine-actor-client`, `fdd-chat-engine-actor-end-user`
<!-- fdd-id-content -->

#### FR-011: Session Summary

- [ ] `p2` - **ID**: `fdd-chat-engine-fr-session-summary`

<!-- fdd-id-content -->
The system **SHOULD** support session summarization if enabled by session type capabilities. Summary generation is triggered automatically or on demand and can be handled by the webhook backend or a dedicated summarization service. The summary is stored as session metadata.

**Actors**: `fdd-chat-engine-actor-client`, `fdd-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

#### FR-012: Search Session History

- [ ] `p3` - **ID**: `fdd-chat-engine-fr-search-session`

<!-- fdd-id-content -->
The system **MAY** search within a single session's message history and return matching messages with surrounding context. Search supports text matching across all message roles (user and assistant).

**Actors**: `fdd-chat-engine-actor-client`
<!-- fdd-id-content -->

#### FR-013: Search Across Sessions

- [ ] `p3` - **ID**: `fdd-chat-engine-fr-search-sessions`

<!-- fdd-id-content -->
The system **MAY** search across all sessions belonging to a client and return ranked results with session metadata (session ID, title, timestamp, match context). Results are ordered by relevance.

**Actors**: `fdd-chat-engine-actor-client`
<!-- fdd-id-content -->

#### FR-014: Delete Session

- [ ] `p1` - **ID**: `fdd-chat-engine-fr-delete-session`

<!-- fdd-id-content -->
The system **MUST** delete sessions and all associated messages. Before deletion, the system notifies the webhook backend to allow cleanup of backend-specific resources. Deletion is permanent and cannot be undone.

**Actors**: `fdd-chat-engine-actor-client`, `fdd-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

#### FR-015: WebSocket Protocol Support

- [ ] `p2` - **ID**: `fdd-chat-engine-fr-websocket-protocol`

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

**Actors**: `fdd-chat-engine-actor-client`, `fdd-chat-engine-actor-webhook-backend`
<!-- fdd-id-content -->

## 4. Use Cases

#### UC-001: Create Session and Send First Message

**ID**: `fdd-chat-engine-usecase-create-session`

<!-- fdd-id-content -->
**Actor**: `fdd-chat-engine-actor-client`

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

**ID**: `fdd-chat-engine-usecase-recreate-response`

<!-- fdd-id-content -->
**Actor**: `fdd-chat-engine-actor-client`

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

**ID**: `fdd-chat-engine-usecase-branch-message`

<!-- fdd-id-content -->
**Actor**: `fdd-chat-engine-actor-client`

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

**ID**: `fdd-chat-engine-usecase-export-session`

<!-- fdd-id-content -->
**Actor**: `fdd-chat-engine-actor-client`

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

**ID**: `fdd-chat-engine-usecase-share-session`

<!-- fdd-id-content -->
**Actor**: `fdd-chat-engine-actor-client`, `fdd-chat-engine-actor-end-user`

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

## 5. Non-functional Requirements

#### NFR-001: Response Time

**ID**: `fdd-chat-engine-nfr-response-time`

<!-- fdd-id-content -->
Message routing latency must be less than 100ms at p95, measured from receiving client message to forwarding to webhook backend (excluding backend processing time). Session creation must complete within 200ms at p95, including database write and backend notification.
<!-- fdd-id-content -->

#### NFR-002: Availability

**ID**: `fdd-chat-engine-nfr-availability`

<!-- fdd-id-content -->
System must maintain 99.9% uptime for session management operations (create, retrieve, delete sessions). During webhook backend failures, the system must support degraded mode with read-only access to session history. Planned maintenance windows must be scheduled during low-traffic periods with advance notice.
<!-- fdd-id-content -->

#### NFR-003: Scalability

**ID**: `fdd-chat-engine-nfr-scalability`

<!-- fdd-id-content -->
System must support at least 10,000 concurrent active sessions per instance. Message throughput must support at least 1,000 messages per second per instance. System must support horizontal scaling by adding instances without shared state constraints.
<!-- fdd-id-content -->

#### NFR-004: Data Persistence

**ID**: `fdd-chat-engine-nfr-data-persistence`

<!-- fdd-id-content -->
All messages must be persisted to database before sending acknowledgment to client. Zero message loss is required during system failures, network interruptions, or backend failures. Database writes must use ACID transactions to ensure consistency.
<!-- fdd-id-content -->

#### NFR-005: Streaming Performance

**ID**: `fdd-chat-engine-nfr-streaming`

<!-- fdd-id-content -->
Streaming latency overhead (time between receiving chunk from backend and forwarding to client) must be less than 10ms at p95. First byte of streamed response must arrive at client within 200ms of backend starting to stream. Streaming must support backpressure to handle slow clients.
<!-- fdd-id-content -->

#### NFR-006: Authentication

**ID**: `fdd-chat-engine-nfr-authentication`

<!-- fdd-id-content -->
System must authenticate all client requests using secure authentication mechanisms. Session access must be restricted to authorized clients (session owner or share token holders). Client IDs must be validated on every request.
<!-- fdd-id-content -->

#### NFR-007: Data Integrity

**ID**: `fdd-chat-engine-nfr-data-integrity`

<!-- fdd-id-content -->
Message tree structure must maintain referential integrity at all times. Orphaned messages (messages with non-existent parent) are not allowed. Parent-child relationships must be immutable once created. Database constraints must enforce tree structure integrity.
<!-- fdd-id-content -->

#### NFR-008: Backend Isolation

**ID**: `fdd-chat-engine-nfr-backend-isolation`

<!-- fdd-id-content -->
Webhook backend failures must not affect other sessions using different backends. Request timeout must be configurable per session type with a default of 30 seconds. Backend errors must be isolated and logged without cascading to other system components.
<!-- fdd-id-content -->

#### NFR-009: File Size Limits

**ID**: `fdd-chat-engine-nfr-file-size`

<!-- fdd-id-content -->
System must enforce file size limits with a default of 10MB per individual file. Total attachments per message must be limited to 50MB. File size validation occurs at client upload time (enforced by file storage service) and limits are configurable per session type.
<!-- fdd-id-content -->

#### NFR-010: Search Performance

**ID**: `fdd-chat-engine-nfr-search`

<!-- fdd-id-content -->
Session history search must return results within 1 second at p95 for sessions with up to 10,000 messages. Cross-session search must return results within 3 seconds at p95 for clients with up to 1,000 sessions. Search must support pagination for large result sets.
<!-- fdd-id-content -->

#### NFR-011: WebSocket Performance

**ID**: `fdd-chat-engine-nfr-websocket-performance`

<!-- fdd-id-content -->
WebSocket connection establishment must complete within 500ms at p95. Message routing latency over WebSocket must be less than 50ms at p95 (lower than HTTP's 100ms target). Heartbeat interval must be 30 seconds with automatic reconnection using exponential backoff (maximum 60 seconds). System must support at least 5,000 concurrent WebSocket connections per instance.
<!-- fdd-id-content -->

#### NFR-012: WebSocket Reliability

**ID**: `fdd-chat-engine-nfr-websocket-reliability`

<!-- fdd-id-content -->
WebSocket connections must support automatic reconnection with state restoration after network interruptions. Message delivery guarantees must match HTTP protocol (at-least-once for operations, exactly-once for streaming). System must handle graceful connection closure with pending operation completion or cancellation. Connection timeout must be 5 minutes for idle connections, configurable per deployment.
<!-- fdd-id-content -->

## 6. Additional Context

#### Integration with Webhook Backends

**ID**: `fdd-chat-engine-prd-context-webhook-integration`

<!-- fdd-id-content -->
Webhook backends are expected to be HTTP services that receive session context (session metadata, capabilities, message history) and return responses. Backends are responsible for all message processing logic, enabling flexible implementations including AI chat (LLMs), rule-based systems, human-in-the-loop support, or hybrid approaches. The webhook contract is designed to be backend-agnostic, allowing easy experimentation with different processing approaches.
<!-- fdd-id-content -->

#### Message Tree Structure

**ID**: `fdd-chat-engine-prd-context-message-tree`

<!-- fdd-id-content -->
Messages form a tree structure where each message (except the root) references a parent message. This tree structure enables conversation branching and message variant preservation. Multiple sibling messages with the same parent represent variants (alternative responses). The client application is responsible for rendering the tree structure in UI and providing navigation controls. The system maintains tree integrity but does not enforce a specific UI representation.
<!-- fdd-id-content -->

#### Message Visibility Control

**ID**: `fdd-chat-engine-prd-context-message-visibility`

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

#### Assumptions

**ID**: `fdd-chat-engine-prd-context-assumptions`

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

**ID**: `fdd-chat-engine-prd-context-non-goals`

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

**ID**: `fdd-chat-engine-prd-context-risks`

<!-- fdd-id-content -->
Identified risks and mitigation strategies:
- **Webhook Backend Latency**: Slow backends directly impact user experience. Mitigation: configurable timeouts per session type, monitoring and alerting for slow backends, consider caching for idempotent operations.
- **Database Contention**: High message volume may cause database write contention and slow queries. Mitigation: read replicas for query operations, connection pooling, query optimization, consider sharding by client ID.
- **Message Tree Complexity**: Deep branching (many variants or deep trees) may impact query performance and UI rendering. Mitigation: implement depth limits, pagination for variant navigation, database indexing on parent relationships.
- **File Storage Costs**: Unrestricted file attachments may lead to high storage costs. Mitigation: enforce file size limits, implement retention policies, consider compression for certain file types.
- **Session Abandonment**: Large numbers of inactive sessions may consume database resources. Mitigation: implement session cleanup policies, archive old sessions, monitor active session metrics.
<!-- fdd-id-content -->
