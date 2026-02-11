# ADR-0017: Conversation Branching from Any Historical Message

**Date**: 2026-02-04

**Status**: accepted

**ID**: `fdd-chat-engine-adr-branching-strategy`

## Context and Problem Statement

Users may want to explore alternative conversation paths by sending different messages from historical points in conversation. How should Chat Engine enable branching from any message while preserving the original conversation path and maintaining consistent backend context?

## Decision Drivers

* Branch from any message (not just latest)
* Preserve original conversation path (non-destructive)
* Consistent backend context (history up to branch point)
* Multiple branches from same message (unlimited branching)
* Navigation between branches
* Active path tracking (which branch is currently selected)
* Database integrity (no orphaned messages)
* UI visualization of branches

## Considered Options

* **Option 1: Parent reference with history truncation** - Client specifies parent_message_id, backend receives truncated history
* **Option 2: Copy-on-write** - Duplicate conversation up to branch point, then diverge
* **Option 3: Diff-based branches** - Store only differences from original path

## Decision Outcome

Chosen option: "Parent reference with history truncation", because it preserves original path unchanged, enables unlimited branching via parent_message_id references, provides consistent backend context (history up to parent), maintains database integrity via foreign keys, and naturally represents branches in message tree structure.

### Consequences

* Good, because original conversation path completely preserved
* Good, because branching from any message (just specify parent_message_id)
* Good, because unlimited branches from same message (multiple children)
* Good, because backend receives correct context (messages up to branch point)
* Good, because navigation clear (follow different parent_message_id paths)
* Good, because database foreign keys enforce integrity
* Good, because branches visible in message tree structure
* Bad, because history loading requires recursive query (up to branch point)
* Bad, because UI must render tree structure (more complex than linear)
* Bad, because active path calculation requires following is_active chain
* Bad, because deep branching creates complex tree (performance considerations)

## Related Design Elements

**Actors**:
* `fdd-chat-engine-actor-client` - Specifies parent_message_id for branching
* `fdd-chat-engine-actor-webhook-backend` - Receives truncated history for branch
* `fdd-chat-engine-message-processing` - Loads context up to parent, validates references

**Requirements**:
* `fdd-chat-engine-fr-branch-message` - Client specifies parent, creates new branch
* `fdd-chat-engine-nfr-data-integrity` - Foreign key constraint on parent_message_id
* `fdd-chat-engine-usecase-branch-message` - Full use case for branching

**Design Elements**:
* `fdd-chat-engine-entity-message` - parent_message_id enables branching
* `fdd-chat-engine-design-context-tree-traversal` - Recursive CTE for history loading
* Sequence diagram S7 (Branch from Historical Message)

**Related ADRs**:
* ADR-0001 (Message Tree Structure) - Tree structure enables branching
* ADR-0014 (Message Variants with Index and Active Flag) - Variants vs branches distinction
* ADR-0016 (Recreation Creates Variants, Branching Creates Children) - Branching creates children (not variants)
