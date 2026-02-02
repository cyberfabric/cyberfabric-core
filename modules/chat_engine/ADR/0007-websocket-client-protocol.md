# ADR-0007: HTTP REST + WebSocket Dual Protocol for Client Communication

**Date**: 2026-02-05 (Updated from 2026-02-04)

**Status**: accepted (supersedes original WebSocket-only decision)

**ID**: `fdd-chat-engine-adr-http-websocket-split`

## Context and Problem Statement

Chat Engine needs to support both simple CRUD operations (session management, message retrieval, search) and real-time streaming operations (message streaming, server push notifications). What protocol architecture should be used between client applications and Chat Engine to optimize for both use cases?

## Decision Drivers

**For CRUD Operations**:
* Standard RESTful patterns and HTTP semantics
* Easy testing with standard tools (curl, Postman)
* HTTP caching and CDN support
* Standard authentication (Bearer tokens)
* No persistent connection overhead for simple operations

**For Streaming Operations**:
* Real-time streaming of assistant responses (time-to-first-byte < 200ms)
* Multiple concurrent streams over single connection
* Server-initiated events (session updates, push notifications)
* Efficient connection management (avoid HTTP polling overhead)
* Request/response matching for concurrent operations
* Connection keep-alive and automatic reconnection

## Considered Options

* **Option 1: WebSocket only** - All operations over WebSocket (original design)
* **Option 2: HTTP REST + WebSocket split** - HTTP for CRUD, WebSocket for streaming
* **Option 3: HTTP/2 Server-Sent Events (SSE)** - HTTP/2 for requests, SSE for streaming
* **Option 4: gRPC split** - gRPC unary for CRUD, bidirectional streaming for messages

## Decision Outcome

Chosen option: "HTTP REST + WebSocket split", because it provides optimal protocol for each operation type, reduces complexity for simple CRUD operations, follows industry patterns (Slack, Discord, GitHub), enables standard HTTP features (caching, CDN, load balancing), simplifies testing and debugging, while preserving WebSocket benefits for streaming and push notifications.

### Consequences

**HTTP REST API Benefits**:
* Good, because standard RESTful patterns familiar to all developers
* Good, because easy testing with curl, Postman, standard HTTP clients
* Good, because standard load balancing, caching, CDN support
* Good, because stateless operations simplify horizontal scaling
* Good, because HTTP status codes provide clear semantics (200, 404, 500)
* Good, because mature debugging tools (browser DevTools, network analyzers)

**WebSocket API Benefits**:
* Good, because persistent connection eliminates handshake overhead for streaming
* Good, because bidirectional allows server push (session.updated, message.created)
* Good, because multiple concurrent streams share connection (parallel requests)
* Good, because WebSocket libraries handle reconnection, keep-alive automatically
* Good, because JSON framing simple for debugging (human-readable)

**Dual Protocol Benefits**:
* Good, because each protocol optimized for its use case
* Good, because follows industry patterns (Slack, Discord, GitHub)
* Good, because clients can use HTTP-only for simple apps (no WebSocket needed)
* Good, because simpler client implementation (HTTP for CRUD, WebSocket when needed)

**Dual Protocol Drawbacks**:
* Bad, because clients must implement two protocols instead of one
* Bad, because requires coordination between HTTP auth and WebSocket auth
* Bad, because potential for inconsistent state if protocols used incorrectly
* Bad, because two separate protocol specifications to maintain
* Bad, because WebSocket still requires stateful connection management (sticky sessions)

**Implementation Impact**:
* Requires dual API implementation (HTTP server + WebSocket server)
* Requires clear documentation on which protocol to use for each operation
* Requires client libraries to support both protocols
* Enables phased migration (HTTP first, add WebSocket later)

## Protocol Distribution

**HTTP REST API (14 operations)**:
* Session Management: create, get, delete, switch type, export, share, access shared
* Message Operations: list, get, stop, get variants, send multi
* Search Operations: search in session, search across sessions

**WebSocket API (5 operations)**:
* Client→Server (3): message.send, message.recreate, session.summarize
* Server→Client (2 push): session.updated, message.created

## Related Design Elements

**Actors**:
* `fdd-chat-engine-actor-client` - Web/mobile/desktop apps using HTTP REST and WebSocket
* Chat Engine instances - Each instance includes HTTP server + WebSocket server

**Requirements**:
* CRUD operations use HTTP REST for simplicity and standard patterns
* Streaming operations use WebSocket for real-time delivery
* `fdd-chat-engine-nfr-streaming` - First byte < 200ms, overhead < 10ms per chunk
* `fdd-chat-engine-nfr-response-time` - HTTP routing < 50ms, WebSocket routing < 100ms
* `fdd-chat-engine-fr-stop-streaming` - Cancellation via HTTP POST (not WebSocket)

**Design Elements**:
* HTTP REST server - Handles CRUD operations, authentication via Bearer tokens
* WebSocket server - Handles streaming operations, authentication via JWT
* `fdd-chat-engine-response-streaming` - Manages WebSocket frame streaming
* HTTP REST API specification (Section 3.3.1 of DESIGN.md)
* WebSocket API specification (Section 3.3.2 of DESIGN.md)
* Webhook API specification (Section 3.3.3 of DESIGN.md)

**Related ADRs**:
* ADR-0003 (Streaming Architecture) - WebSocket used for client-side streaming delivery
* ADR-0006 (Webhook Protocol) - Backend protocol remains HTTP (not WebSocket)
* ADR-0009 (Streaming Cancellation) - Cancellation now via HTTP POST /messages/{id}/stop
* ADR-0010 (Stateless Scaling) - HTTP enables stateless scaling; WebSocket requires session affinity

## Migration Path

For existing WebSocket-only clients:
1. Phase 1: Implement HTTP REST for CRUD operations (optional backward compatibility)
2. Phase 2: Migrate read operations to HTTP (GET session, GET messages, search)
3. Phase 3: Migrate write operations to HTTP (POST session, DELETE, PATCH)
4. Phase 4: Keep only streaming operations on WebSocket
5. Phase 5: Remove backward compatibility for old WebSocket CRUD operations
