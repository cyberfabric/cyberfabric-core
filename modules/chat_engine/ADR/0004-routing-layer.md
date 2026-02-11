# ADR-0004: Zero Business Logic in Routing Layer

**Date**: 2026-02-04

**Status**: accepted

**ID**: `fdd-chat-engine-adr-routing-layer`

## Context and Problem Statement

Chat Engine sits between clients and webhook backends as a proxy service. Should Chat Engine inspect, analyze, or transform message content, or should it remain a pure routing infrastructure focused on session management and message persistence?

## Decision Drivers

* Enable rapid backend experimentation without infrastructure changes
* Keep Chat Engine focused on infrastructure concerns (routing, persistence, scaling)
* Avoid coupling Chat Engine to specific AI models or processing logic
* Support diverse backend types (LLMs, rule-based, human-in-the-loop)
* Simplify Chat Engine codebase and reduce maintenance burden
* Enable backends to evolve independently
* Minimize latency overhead from proxying

## Considered Options

* **Option 1: Zero business logic (pure routing)** - Chat Engine only routes, persists, and manages message trees
* **Option 2: Enrichment layer** - Chat Engine adds metadata, moderation, logging before routing
* **Option 3: Smart routing** - Chat Engine analyzes content to select appropriate backend or transform messages

## Decision Outcome

Chosen option: "Zero business logic (pure routing)", because it decouples infrastructure from processing logic, enables backends to change without Chat Engine updates, keeps routing latency minimal, allows diverse backend implementations, and simplifies Chat Engine codebase focusing on reliability and scaling.

### Consequences

* Good, because backends can change processing logic without Chat Engine deployment
* Good, because new backend types require zero Chat Engine code changes
* Good, because routing layer remains simple, testable, and maintainable
* Good, because latency overhead is minimal (no content inspection/transformation)
* Good, because Chat Engine can focus on reliability, scaling, and message tree management
* Good, because content moderation, language detection, etc. can be backend-specific
* Bad, because common processing (moderation, logging enrichment) must be implemented per backend
* Bad, because Chat Engine cannot provide value-added services (e.g., automatic translation)
* Bad, because debugging requires looking at backend logs (Chat Engine doesn't inspect content)

## Related Design Elements

**Actors**:
* `fdd-chat-engine-actor-webhook-backend` - Responsible for ALL message processing logic

**Design Elements**:
* `fdd-chat-engine-webhook-integration` - Chat Engine's HTTP proxy functionality with timeout/circuit breaker

**Requirements**:
* All functional requirements assume Chat Engine routes without processing
* `fdd-chat-engine-nfr-response-time` - Minimal overhead from routing (< 100ms)

* `fdd-chat-engine-principle-zero-business-logic` - Design principle codifying this decision
* `fdd-chat-engine-webhook-integration` - Chat Engine's HTTP client functionality for pure forwarding
* `fdd-chat-engine-design-context-webhook-integration` - Backend responsibility scope

**Related ADRs**:
* ADR-0002 (Capability Model) - Backends define capabilities, not Chat Engine
* ADR-0006 (Webhook Protocol) - Protocol passes content unchanged
* ADR-0008 (Webhook Event Schema with Typed Events) - Events carry full context without interpretation
