# ADR-0018: Session Type Switching with Capability Updates

**Date**: 2026-02-04

**Status**: accepted

**ID**: `fdd-chat-engine-adr-session-switching`

## Context and Problem Statement

Chat Engine supports multiple session types (different webhook backends like GPT-4, Claude, human support). Users may want to switch backends mid-conversation (e.g., escalate from AI to human). How should Chat Engine handle session type switching while preserving conversation history and updating capabilities?

## Decision Drivers

* Preserve full conversation history when switching
* Update capabilities to reflect new backend features
* No message loss or data corruption
* Backend receives complete history (not just current type's messages)
* Simple client API for switching
* Session metadata remains consistent
* Support switching to/from any backend type
* Enable use cases like AI → human escalation

## Considered Options

* **Option 1: Update session_type_id, route next message to new backend** - Mutable session_type_id field, routing changes immediately
* **Option 2: Create new session, copy history** - New session record with duplicated messages
* **Option 3: Message-level backend tracking** - Each message stores backend used, no session-level type

## Decision Outcome

Chosen option: "Update session_type_id, route next message to new backend", because it preserves conversation history in single session, updates capabilities from new backend, enables simple client API (single field update), maintains referential integrity, and supports all switching use cases (AI ↔ AI, AI ↔ human).

### Consequences

* Good, because single session retains full conversation history
* Good, because new backend receives complete history (all messages)
* Good, because client API simple (session.switch_type event)
* Good, because no message duplication or data migration
* Good, because capabilities updated from new backend
* Good, because session metadata (title, timestamps) preserved
* Bad, because history mixing backends may confuse some backend implementations
* Bad, because old capabilities become stale (stored but inactive)
* Bad, because cannot easily revert to previous backend (no capability restoration)
* Bad, because backend type history not tracked per message

## Related Design Elements

**Actors**:
* `fdd-chat-engine-actor-client` - Initiates session type switching
* `fdd-chat-engine-actor-webhook-backend` - New backend receives full history
* `fdd-chat-engine-session-management` - Updates session_type_id

**Requirements**:
* `fdd-chat-engine-fr-switch-session-type` - Switch to different backend mid-conversation
* `fdd-chat-engine-fr-send-message` - Routing uses current session_type_id

**Design Elements**:
* `fdd-chat-engine-entity-session` - session_type_id field (mutable)
* `fdd-chat-engine-entity-session-type` - Defines webhook_url per backend
* Sequence diagram S4 (Switch Session Type Mid-Conversation)

**Related ADRs**:
* ADR-0002 (Capability Model) - New backend returns updated capabilities
* ADR-0006 (Webhook Protocol) - New backend receives message.new with full history
* ADR-0007 (Per-Request Capabilities) - Client can enable/disable capabilities per message
