# ADR-0006: Synchronous HTTP Webhooks with Streaming

**Date**: 2026-02-04

**Status**: accepted

**ID**: `fdd-chat-engine-adr-webhook-protocol`

## Context and Problem Statement

Chat Engine needs to invoke webhook backends for message processing, passing session context and receiving responses. What protocol should be used for Chat Engine to communicate with external webhook backends to balance simplicity, streaming support, and backend flexibility?

## Decision Drivers

* Streaming responses from backends (especially LLMs)
* Simple backend integration (standard HTTP)
* Synchronous semantics for simpler error handling
* Keep client connection open during processing
* Backend developers familiar with HTTP
* Timeout management per backend
* No complex message broker infrastructure
* Direct feedback to clients (no polling)

## Considered Options

* **Option 1: HTTP POST with chunked streaming** - Synchronous HTTP requests with chunked transfer encoding for streaming
* **Option 2: WebSocket bidirectional** - Persistent WebSocket connection between Chat Engine and backends
* **Option 3: Message queue (async)** - Async message queue with callback URLs for responses

## Decision Outcome

Chosen option: "HTTP POST with chunked streaming", because it provides simple integration for backend developers (standard HTTP), supports streaming via chunked transfer encoding with newline-delimited JSON (NDJSON), maintains synchronous semantics simplifying error handling, requires no persistent connections or message broker infrastructure, and keeps client connections open for real-time streaming.

### Consequences

* Good, because backends use standard HTTP (no WebSocket or message queue complexity)
* Good, because streaming is simple (chunked transfer encoding is standard HTTP feature)
* Good, because synchronous calls simplify error handling (HTTP status codes)
* Good, because timeout management is straightforward (HTTP request timeout)
* Good, because backend developers can test with curl/Postman easily
* Good, because no persistent connections or connection management
* Bad, because no async callback support (backends must respond synchronously)
* Bad, because long-running operations (>30s) require timeout configuration
* Bad, because backends cannot push updates after response completes
* Bad, because network interruptions terminate request (no automatic retry)

## Related Design Elements

**Actors**:
* `fdd-chat-engine-actor-webhook-backend` - Receives HTTP POST, responds with streaming

**Requirements**:
* `fdd-chat-engine-fr-send-message` - Forward message to webhook with streaming response
* `fdd-chat-engine-fr-create-session` - session.created event to webhook
* `fdd-chat-engine-nfr-backend-isolation` - Timeout and circuit breaker per backend
* `fdd-chat-engine-nfr-streaming` - Streaming performance requirements

**Design Elements**:
* `fdd-chat-engine-webhook-integration` - Chat Engine's HTTP client functionality for invoking webhooks
* `fdd-chat-engine-constraint-sync-webhooks` - Design constraint mandating synchronous protocol
* `fdd-chat-engine-entity-session-type` - Stores webhook_url and timeout per backend
* `fdd-chat-engine-design-context-circuit-breaker` - Circuit breaker implementation per backend

**Related ADRs**:
* ADR-0003 (Streaming Architecture) - Depends on HTTP streaming from backends
* ADR-0008 (Webhook Event Schema with Typed Events) - Defines event schemas sent via this protocol
* ADR-0011 (Circuit Breaker per Webhook Backend) - Resilience pattern for webhook failures
* ADR-0013 (Per-Session-Type Timeout Configuration) - Per-backend timeout management
