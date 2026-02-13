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
* Simple cancellation mechanism
* Backend cleanup (cancel backend request)
* Database persistence of partial responses

## Considered Options

* **Option 1: Close HTTP connection** - Abort HTTP request to cancel stream
* **Option 2: HTTP DELETE request** - Separate HTTP endpoint to cancel by message_id
* **Option 3: HTTP timeout** - Set aggressive timeout to limit long operations

## Decision Outcome

Chosen option: "Close HTTP connection", because it provides immediate cancellation by aborting the HTTP request (using AbortController in browsers or request cancellation in other clients), saves backend resources, preserves partial responses with is_complete=false flag, aligns with standard HTTP patterns, and requires no separate cancellation endpoint.

### Consequences

* Good, because standard HTTP cancellation pattern (AbortController)
* Good, because immediate resource cleanup (no lingering connections)
* Good, because simple client implementation (abort request)
* Good, because backend detects disconnection immediately
* Good, because partial response preserved with is_complete=false flag
* Good, because no separate cancellation endpoint needed
* Bad, because connection close terminates stream (by design)
* Bad, because no explicit acknowledgment (implicit via disconnection)
* Bad, because backend must handle connection close gracefully

## Related Design Elements

**Actors**:
* `fdd-chat-engine-actor-client` - Closes HTTP connection on user action
* `fdd-chat-engine-response-streaming` - Detects connection close, saves partial response
* `fdd-chat-engine-webhook-integration` - Cancels HTTP request to backend

**Requirements**:
* `fdd-chat-engine-fr-stop-streaming` - Cancel streaming, save partial response with incomplete flag
* `fdd-chat-engine-nfr-streaming` - Minimal latency for cancellation response

**Design Elements**:
* `fdd-chat-engine-entity-message` - is_complete field indicates cancelled messages
* HTTP connection close mechanism (Section 3.3.1 of DESIGN.md)
* Sequence diagram S11 (Stop Streaming Response)

**Related ADRs**:
* ADR-0003 (Streaming Architecture) - Depends on this for complete streaming lifecycle
* ADR-0007 (HTTP Client Protocol) - HTTP streaming client protocol with cancellation
* ADR-0006 (Webhook Protocol) - HTTP request cancellation to backend
