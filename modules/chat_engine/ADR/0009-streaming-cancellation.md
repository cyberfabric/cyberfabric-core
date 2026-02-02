# ADR-0009: Client-Initiated Streaming Cancellation

**Date**: 2026-02-04

**Status**: accepted

**ID**: `fdd-chat-engine-adr-streaming-cancellation`

## Context and Problem Statement

Users may want to stop assistant responses mid-generation (too slow, wrong direction, changing question). How should clients cancel ongoing streaming responses to save compute resources and provide responsive "stop" button UX?

## Decision Drivers

* User control over generation (stop button in UI)
* Compute resource conservation (cancel backend processing)
* Partial response preservation (save incomplete response)
* Responsive cancellation (immediate UI feedback)
* WebSocket connection remains open (don't disconnect)
* Multiple concurrent streams (cancel specific stream)
* Backend cleanup (cancel backend request)
* Database persistence of partial responses

## Considered Options

* **Option 1: WebSocket message with request_id** - Client sends message.stop event identifying stream
* **Option 2: Close WebSocket connection** - Disconnect to cancel all active streams
* **Option 3: HTTP DELETE request** - Separate HTTP endpoint to cancel by message_id

## Decision Outcome

Chosen option: "WebSocket message with request_id", because it allows canceling specific streams without closing connection, enables immediate UI feedback and backend request cancellation, preserves partial responses as incomplete messages, maintains WebSocket connection for subsequent operations, and supports multiple concurrent streams independently.

### Consequences

* Good, because specific stream cancellation (not all streams on connection)
* Good, because WebSocket connection remains open for subsequent messages
* Good, because backend HTTP request cancelled immediately (resource conservation)
* Good, because partial response saved with is_complete=false flag
* Good, because client gets immediate acknowledgment (response.success)
* Good, because UI can show "stopped" state with partial content
* Bad, because backend must handle request cancellation gracefully
* Bad, because partial responses require special UI rendering
* Bad, because race condition if cancellation arrives after completion (idempotent handling needed)

## Related Design Elements

**Actors**:
* `fdd-chat-engine-actor-client` - Sends message.stop event on user action
* `fdd-chat-engine-response-streaming` - Stops forwarding chunks, saves partial response
* `fdd-chat-engine-webhook-integration` - Cancels HTTP request to backend

**Requirements**:
* `fdd-chat-engine-fr-stop-streaming` - Cancel streaming, save partial response with incomplete flag
* `fdd-chat-engine-nfr-streaming` - Minimal latency for cancellation response

**Design Elements**:
* `fdd-chat-engine-entity-message` - is_complete field indicates cancelled messages
* WebSocket message.stop event (Section 3.3.1 of DESIGN.md)
* Sequence diagram S11 (Stop Streaming Response)

**Related ADRs**:
* ADR-0003 (Streaming Architecture) - Depends on this for complete streaming lifecycle
* ADR-0007 (WebSocket Client Protocol) - WebSocket enables mid-stream cancellation
* ADR-0006 (Webhook Protocol) - HTTP request cancellation to backend
