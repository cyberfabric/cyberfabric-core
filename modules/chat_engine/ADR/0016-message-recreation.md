# ADR-0016: Recreation Creates Variants, Branching Creates Children

**Date**: 2026-02-04

**Status**: accepted

**ID**: `fdd-chat-engine-adr-message-recreation`

## Context and Problem Statement

Users can both regenerate assistant responses and branch from historical messages. These operations seem similar (both create alternative conversation paths) but have different semantics. How should Chat Engine distinguish between recreation (trying again with same user message) and branching (new user message from historical point)?

## Decision Drivers

* Semantic difference: recreation = same input different output, branching = new input
* Variant navigation clarity (user expects variants to answer same question)
* Message tree structure consistency
* Context loading for webhook backend (history truncation)
* UI affordances (regenerate button vs branch action)
* Webhook event differentiation (message.recreate vs message.new)
* Conversation history integrity

## Considered Options

* **Option 1: Recreation = variant (sibling), Branch = child** - Recreation creates sibling with same parent, branching creates child
* **Option 2: Both create children** - Both operations create new children, no distinction
* **Option 3: Recreation = update** - Recreation replaces original message, branching creates child

## Decision Outcome

Chosen option: "Recreation = variant (sibling), Branch = child", because it preserves semantic distinction (same input vs new input), enables natural variant navigation (comparing different answers to same question), maintains conversation history integrity (branching preserves original path), and clearly differentiates webhook events (message.recreate vs message.new).

### Consequences

* Good, because semantic distinction clear (variants = same question, children = new question)
* Good, because variant navigation intuitive (compare alternative answers)
* Good, because branching preserves original conversation path
* Good, because webhook events distinguish intent (recreate vs new)
* Good, because UI can show appropriate affordances (regenerate button vs branch action)
* Good, because history truncation different (recreation uses same history, branching truncates)
* Bad, because two concepts for similar operations (user education needed)
* Bad, because implementation differs (variant_index calculation vs parent_message_id assignment)
* Bad, because switching between operations not obvious (user might want to convert)

## Related Design Elements

**Actors**:
* `fdd-chat-engine-actor-client` - Initiates recreation or branching operations
* `fdd-chat-engine-actor-webhook-backend` - Receives different events (message.recreate vs message.new)

**Requirements**:
* `fdd-chat-engine-fr-recreate-response` - Creates sibling with same parent_message_id
* `fdd-chat-engine-fr-branch-message` - Creates child with specified parent_message_id

**Design Elements**:
* `fdd-chat-engine-entity-message` - variant_index for variants, parent_message_id for tree
* Webhook event message.recreate vs message.new (Section 3.3.2 of DESIGN.md)
* Sequence diagrams S6 (Recreate) vs S7 (Branch)

**Related ADRs**:
* ADR-0001 (Message Tree Structure) - Tree structure enables both operations
* ADR-0014 (Message Variants with Index and Active Flag) - Recreation creates variants using variant_index
* ADR-0017 (Conversation Branching from Any Historical Message) - Branching creates children in tree
* ADR-0008 (Webhook Event Schema with Typed Events) - Different events for recreation vs branching
