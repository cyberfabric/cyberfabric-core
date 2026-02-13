# ADR-0022: Per-Request Capability Filtering

**Date**: 2026-02-04

**Status**: accepted

**ID**: `fdd-chat-engine-adr-capability-filtering`

## Context and Problem Statement

Sessions have available_capabilities returned by backends (e.g., web_search, code_execution, image_generation). Users may want to selectively enable/disable capabilities per message rather than using all available capabilities. How should clients control which capabilities are active for specific messages?

## Decision Drivers

* User control over expensive features (disable web_search to save costs)
* Backend receives explicit capability intent per message
* Capabilities available at session level, enabled at message level
* Client validates capabilities against available set
* Backend can optimize based on enabled capabilities
* Support for capability subsets (enable only web_search, not code_execution)
* Future-proof for new capability types
* Clear error messaging for unsupported capabilities

## Considered Options

* **Option 1: enabled_capabilities array per message** - Client sends array of capability names with each message
* **Option 2: Session-level toggle** - Update session to enable/disable capabilities globally
* **Option 3: Implicit capabilities** - Backend infers from message content

## Decision Outcome

Chosen option: "enabled_capabilities array per message", because it provides per-message granularity for capability control, enables user cost optimization, gives backends explicit capability intent, supports capability subsets, maintains session available_capabilities as authoritative, and allows future capability types without protocol changes.

### Consequences

* Good, because users disable expensive capabilities per message (cost optimization)
* Good, because backend receives explicit intent (no capability inference needed)
* Good, because supports capability subsets (enable some, disable others)
* Good, because future capabilities work without protocol changes
* Good, because available_capabilities remain authoritative (session-level)
* Good, because client can validate before sending (check against available_capabilities)
* Bad, because client must send capability array with every message
* Bad, because enabled_capabilities validation adds overhead
* Bad, because invalid capabilities rejected (error handling complexity)
* Bad, because capability defaults not enforced (client must specify)

## Related Design Elements

**Actors**:
* `fdd-chat-engine-actor-client` - Specifies enabled_capabilities per message
* `fdd-chat-engine-actor-webhook-backend` - Receives enabled_capabilities, optimizes accordingly

**Design Elements**:
* Chat Engine validates capabilities against available_capabilities

**Requirements**:
* `fdd-chat-engine-fr-send-message` - Message includes enabled_capabilities array
* `fdd-chat-engine-fr-create-session` - Session stores available_capabilities

**Design Elements**:
* `fdd-chat-engine-entity-session` - available_capabilities (authoritative)
* HTTP POST /messages/send with enabled_capabilities field
* Webhook message.new event with enabled_capabilities array

**Related ADRs**:
* ADR-0002 (Capability Model) - Backend defines available_capabilities
* ADR-0006 (Webhook Protocol) - enabled_capabilities forwarded in webhook events
* ADR-0018 (Session Type Switching with Capability Updates) - Capabilities update when switching backends
