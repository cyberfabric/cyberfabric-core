# ADR-0012: Streaming Backpressure with Buffer Limits

**Date**: 2026-02-04

**Status**: accepted

**ID**: `fdd-chat-engine-adr-backpressure-handling`

## Context and Problem Statement

Webhook backends may stream responses faster than clients can consume (slow network, slow device rendering). How should Chat Engine handle backpressure to prevent memory exhaustion while maintaining streaming responsiveness?

## Decision Drivers

* Prevent memory exhaustion from unbounded buffering
* Support slow clients without blocking fast backends entirely
* Graceful handling when client cannot keep up
* HTTP/2 flow control for backend requests
* Per-stream buffer limits (not global)
* Client disconnect cancels backend request
* Minimal latency when client is fast
* Observable buffer metrics for monitoring

## Considered Options

* **Option 1: Per-stream buffer with limit and pause** - Buffer up to 10MB, pause backend via HTTP/2 flow control
* **Option 2: Unbounded buffering** - Buffer all chunks until client catches up
* **Option 3: Drop chunks** - Discard chunks when buffer full

## Decision Outcome

Chosen option: "Per-stream buffer with limit and pause", because it prevents memory exhaustion via 10MB buffer limit, uses HTTP/2 flow control to pause backend when buffer fills, supports slow clients within buffer limit, enables client disconnect to immediately cancel backend request, and maintains low latency for fast clients.

### Consequences

* Good, because memory usage bounded (10MB max per stream)
* Good, because backend paused via HTTP/2 flow control (not cancelled)
* Good, because slow clients supported within buffer limit
* Good, because client disconnect immediately cancels backend (saves resources)
* Good, because fast clients see minimal latency (no buffering)
* Good, because per-stream limits prevent one slow client affecting others
* Bad, because extremely slow clients may exhaust buffer (stream cancellation)
* Bad, because HTTP/2 flow control complexity (not all backends support)
* Bad, because buffer management adds overhead (~5% CPU)
* Bad, because no prioritization (all streams treated equally)

## Related Design Elements

**Actors**:
* `fdd-chat-engine-response-streaming` - Implements buffer and backpressure logic
* `fdd-chat-engine-actor-webhook-backend` - Paused via HTTP/2 flow control
* `fdd-chat-engine-actor-client` - Slow consumption triggers backpressure

**Requirements**:
* `fdd-chat-engine-nfr-streaming` - Backpressure handling requirement
* `fdd-chat-engine-fr-stop-streaming` - Client disconnect cancels backend

**Design Elements**:
* `fdd-chat-engine-design-context-backpressure` - Implementation details (10MB limit, HTTP/2 flow control)
* `fdd-chat-engine-response-streaming` - Buffer management per stream

**Related ADRs**:
* ADR-0003 (Streaming Architecture) - Streaming design depends on backpressure handling
* ADR-0006 (Webhook Protocol) - HTTP/2 flow control for backend pause
* ADR-0009 (Client-Initiated Streaming Cancellation) - Client cancellation releases buffer
