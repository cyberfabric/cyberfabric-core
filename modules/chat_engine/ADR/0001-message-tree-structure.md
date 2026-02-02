# ADR-0001: Message Tree with Immutable Parents

**Date**: 2026-02-04

**Status**: accepted

**ID**: `fdd-chat-engine-adr-message-tree-structure`

## Context and Problem Statement

Chat Engine needs to support conversation branching, message regeneration, and variant exploration while maintaining referential integrity and enabling safe concurrent message creation. How should messages be structured to enable these capabilities without introducing data inconsistencies or race conditions?

## Decision Drivers

* Support for conversation branching from any historical message
* Ability to regenerate assistant responses creating variants
* Referential integrity must be enforced at database level
* Safe concurrent message creation across multiple sessions
* Natural representation of conversation alternatives
* Support for history navigation and variant exploration
* Immutable conversation history for audit and debugging

## Considered Options

* **Option 1: Immutable tree with parent_message_id** - Messages form tree structure where each message references immutable parent via parent_message_id
* **Option 2: Mutable linked list** - Messages form doubly-linked list with next/previous pointers that can be updated
* **Option 3: Graph structure with edge table** - Separate table stores relationships between messages allowing arbitrary connections

## Decision Outcome

Chosen option: "Immutable tree with parent_message_id", because it provides natural representation of conversation structure, enables database-enforced referential integrity via foreign keys, supports safe concurrent writes without conflicts, and makes branching explicit through shared parent relationships.

### Consequences

* Good, because database foreign key constraints enforce tree integrity automatically
* Good, because immutability prevents accidental corruption of conversation history
* Good, because concurrent message creation is safe (different parents = no conflicts)
* Good, because variants are naturally represented as siblings (same parent)
* Good, because tree structure maps directly to conversation branching semantics
* Bad, because traversal queries require recursive CTEs for deep trees
* Bad, because calculating active path requires following is_active flags
* Bad, because re-parenting messages is impossible (by design, ensuring immutability)

## Related Design Elements

**Actors**:
* `fdd-chat-engine-actor-client` - Navigates message tree and requests branching operations
* `fdd-chat-engine-actor-webhook-backend` - Receives truncated history based on tree traversal

**Requirements**:
* `fdd-chat-engine-fr-recreate-response` - Variants created as siblings with same parent_message_id
* `fdd-chat-engine-fr-branch-message` - New messages reference historical message as parent
* `fdd-chat-engine-fr-navigate-variants` - Query siblings by parent_message_id for variant navigation
* `fdd-chat-engine-nfr-data-integrity` - Database constraints enforce tree structure integrity

**Design Elements**:
* `fdd-chat-engine-entity-message` - Core entity implementing tree structure
* `fdd-chat-engine-principle-immutable-tree` - Design principle mandating immutability
* `fdd-chat-engine-design-context-tree-traversal` - Implementation details for traversal queries

**Related ADRs**:
* ADR-0007 (Variant Management) - Depends on this tree structure
* ADR-0007 (Recreation vs New Branch) - Uses parent_message_id to create variants
* ADR-0007 (Conversation Branching) - Leverages tree structure for branching
