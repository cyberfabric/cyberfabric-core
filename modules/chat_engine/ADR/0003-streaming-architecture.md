# ADR-0003: Streaming-First with HTTP/WebSocket Dual Protocol

**Date**: 2026-02-04

**Status**: accepted

**ID**: `fdd-chat-engine-adr-streaming-architecture`

## Context and Problem Statement

Chat Engine must minimize time-to-first-byte for assistant responses to provide responsive user experience. Responses from backends (especially LLM-based) can take seconds to complete. How should Chat Engine handle response delivery to maximize perceived responsiveness?

## Decision Drivers

* Minimize time-to-first-byte for user-perceived responsiveness
* Support backends that stream (LLMs) and backends that don't (rule-based)
* Enable client to display partial responses as they arrive
* Allow cancellation of slow responses to save resources
* WebSocket connection for client API (bidirectional)
* HTTP for webhook backend communication (simple integration)
* Backpressure handling for slow clients
* Minimal latency overhead from proxying

## Considered Options

* **Option 1: Streaming-first with HTTP/WebSocket** - All webhook responses stream via HTTP, proxied over WebSocket to clients
* **Option 2: Buffered responses** - Wait for complete response from backend, then send to client
* **Option 3: Optional streaming** - Backends declare if they stream, Chat Engine adapts behavior per backend

## Decision Outcome

Chosen option: "Streaming-first with HTTP/WebSocket", because it minimizes time-to-first-byte (< 200ms requirement), enables responsive UX for slow backends, supports cancellation saving compute resources, and keeps webhook protocol simple (always HTTP streaming) while client protocol uses persistent WebSocket connections.

### Consequences

* Good, because first response chunk arrives at client within 200ms of backend streaming
* Good, because perceived latency is much lower than buffered approach
* Good, because clients can cancel slow responses (stop button)
* Good, because non-streaming backends work transparently (wrapped in stream adapter)
* Good, because webhook protocol remains simple HTTP (no WebSocket complexity for backend devs)
* Good, because WebSocket enables multiple concurrent streams over single connection
* Bad, because streaming overhead adds ~10ms latency per chunk forwarding
* Bad, because partial responses require special handling if connection drops
* Bad, because backpressure management adds complexity (buffer limits, flow control)

## Related Design Elements

**Actors**:
* `fdd-chat-engine-actor-client` - Receives WebSocket frames with streaming message chunks
* `fdd-chat-engine-actor-webhook-backend` - Streams HTTP responses (chunked transfer encoding)

**Requirements**:
* `fdd-chat-engine-fr-send-message` - Streaming response from backend to client
* `fdd-chat-engine-fr-stop-streaming` - Cancel streaming mid-generation
* `fdd-chat-engine-nfr-streaming` - Latency < 10ms overhead, first byte < 200ms
* `fdd-chat-engine-nfr-response-time` - Overall routing latency < 100ms

**Design Elements**:
* `fdd-chat-engine-response-streaming` - Chat Engine's WebSocket streaming and backpressure functionality
* `fdd-chat-engine-principle-streaming` - Design principle mandating streaming-first
* `fdd-chat-engine-design-context-backpressure` - Implementation details for flow control

**Related ADRs**:
* ADR-0006 (Webhook Protocol) - HTTP streaming from backends via chunked encoding
* ADR-0007 (WebSocket Client Protocol) - WebSocket for client-side streaming
* ADR-0009 (Client-Initiated Streaming Cancellation) - Client cancellation mechanism
* ADR-0012 (Streaming Backpressure with Buffer Limits) - Buffer management and flow control strategy
