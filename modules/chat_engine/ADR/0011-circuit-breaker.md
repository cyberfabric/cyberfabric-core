# ADR-0011: Circuit Breaker per Webhook Backend

**Date**: 2026-02-04

**Status**: accepted

**ID**: `fdd-chat-engine-adr-circuit-breaker`

## Context and Problem Statement

Webhook backends can fail or become slow, potentially causing cascading failures as Chat Engine continues sending requests to unhealthy backends. How should Chat Engine protect itself and other backends from failures in individual webhook backends?

## Decision Drivers

* Prevent cascade failures from slow/failing backends
* Isolate backend failures (don't affect other backends)
* Automatic failure detection (no manual intervention)
* Automatic recovery testing (half-open state)
* Fast-fail for known-bad backends (fail immediately)
* Per-backend isolation (backend A failure doesn't affect backend B)
* Configurable thresholds and timeouts
* Observable circuit state for monitoring

## Considered Options

* **Option 1: Circuit breaker per session_type_id** - Independent circuit breaker tracking each backend's health
* **Option 2: Global circuit breaker** - Single circuit breaker for all backends
* **Option 3: No circuit breaker (simple timeouts)** - Just timeout and retry on every request

## Decision Outcome

Chosen option: "Circuit breaker per session_type_id", because it isolates backend failures preventing cascade effects, enables fast-fail for known-bad backends saving resources, provides automatic recovery testing via half-open state, maintains per-backend failure tracking, and protects Chat Engine and other backends from problematic webhook implementations.

### Consequences

* Good, because backend A failures don't affect backend B routing
* Good, because circuit opens after N consecutive failures (fast-fail, no wasted requests)
* Good, because half-open state tests recovery automatically (single probe request)
* Good, because configurable per session type (failure threshold, open duration)
* Good, because observable state for monitoring and alerting
* Good, because prevents resource exhaustion from repeatedly calling failed backend
* Bad, because adds latency check overhead (~1ms per request)
* Bad, because false positives possible (temporary network issue opens circuit)
* Bad, because circuit state not shared across Chat Engine instances (per-instance circuit)
* Bad, because configuration complexity (threshold, timeout, recovery time tuning)

## Related Design Elements

**Actors**:
* `fdd-chat-engine-webhook-integration` - Implements circuit breaker logic
* `fdd-chat-engine-actor-webhook-backend` - Health monitored by circuit breaker

**Requirements**:
* `fdd-chat-engine-nfr-backend-isolation` - Backend failures must not cascade
* `fdd-chat-engine-nfr-availability` - Chat Engine maintains availability despite backend failures

**Design Elements**:
* `fdd-chat-engine-entity-session-type` - Timeout configuration per backend
* `fdd-chat-engine-design-context-circuit-breaker` - Implementation details (5 failures, 30s open)

**Related ADRs**:
* ADR-0006 (Webhook Protocol) - HTTP protocol for backend communication
* ADR-0010 (Stateless Horizontal Scaling with Database State) - Circuit state per instance (not shared)
* ADR-0013 (Per-Session-Type Timeout Configuration) - Timeout settings per session type
