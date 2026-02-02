# ADR-0002: Webhook Backend Authority for Capabilities

**Date**: 2026-02-04

**Status**: accepted

**ID**: `fdd-chat-engine-adr-capability-model`

## Context and Problem Statement

Chat Engine needs to support different session types with varying capabilities (file attachments, session switching, summarization). Who should define which capabilities are available for each session type, and how should this be managed as backends evolve?

## Decision Drivers

* Backends should control their own feature sets without Chat Engine code changes
* Capability semantics should be opaque to Chat Engine (no hardcoded validation)
* New capabilities should be addable without infrastructure changes
* Session types should be independently evolvable
* Backend-specific features should not require routing layer modifications
* Capability configuration should survive backend updates

## Considered Options

* **Option 1: Backend returns capabilities (webhook authority)** - Webhook backend returns available_capabilities on session.created event
* **Option 2: Capabilities configured in Chat Engine** - Admin configures capabilities via Chat Engine UI/API per session type
* **Option 3: Capability registry service** - Separate service manages capability definitions and assignments

## Decision Outcome

Chosen option: "Backend returns capabilities (webhook authority)", because it gives backends full control over their feature sets, eliminates need for coordination between backend updates and Chat Engine configuration, enables backends to customize capabilities per session (e.g., based on client tier), and keeps Chat Engine agnostic to capability semantics.

### Consequences

* Good, because backends can introduce new capabilities without Chat Engine changes
* Good, because Chat Engine doesn't need to understand capability semantics (just stores and forwards)
* Good, because capability evolution is decoupled from infrastructure evolution
* Good, because backends can customize capabilities per session based on client context
* Good, because testing backends independently doesn't require Chat Engine reconfiguration
* Bad, because Chat Engine cannot validate capability correctness (trusts backend)
* Bad, because clients cannot discover available capabilities without creating session
* Bad, because capability schema is not enforced (backends can return arbitrary JSON)

## Related Design Elements

**Actors**:
* `fdd-chat-engine-actor-webhook-backend` - Authoritative source for capabilities
* `fdd-chat-engine-actor-client` - Consumes capabilities to enable/disable features in UI
* `fdd-chat-engine-actor-developer` - Configures session types and webhook URLs

**Requirements**:
* `fdd-chat-engine-fr-create-session` - Backend returns available_capabilities on creation
* `fdd-chat-engine-fr-switch-session-type` - New backend returns updated capabilities

**Design Elements**:
* `fdd-chat-engine-entity-session` - Stores available_capabilities as immutable JSONB
* `fdd-chat-engine-entity-session-type` - Links to webhook_url but not capability definitions
* `fdd-chat-engine-principle-webhook-authority` - Design principle codifying backend control

**Related ADRs**:
* ADR-0006 (Webhook Protocol) - Defines session.created event returning capabilities
* ADR-0007 (Session Type Switching) - Capability updates when switching backends
* ADR-0007 (Per-Request Capabilities) - Client specifies enabled_capabilities per message
