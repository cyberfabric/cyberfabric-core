# ADR-0014: Message Variants with Index and Active Flag

**Date**: 2026-02-04

**Status**: accepted

**ID**: `fdd-chat-engine-adr-message-variants`

## Context and Problem Statement

Chat Engine supports message regeneration, creating multiple assistant responses for the same user message. How should these variant messages be stored, identified, and navigated to enable users to explore alternatives while maintaining a clear active path?

## Decision Drivers

* Natural representation of variants (siblings in message tree)
* Deterministic ordering (variants numbered 0, 1, 2, ...)
* Active path tracking (which variant is currently selected)
* Unique identification (prevent duplicate variants)
* Navigation metadata (variant position: "2 of 3")
* Database constraints enforce variant integrity
* Support for unlimited variants per parent
* Efficient variant querying

## Considered Options

* **Option 1: variant_index + is_active flags** - Each message has 0-based index and active boolean
* **Option 2: Separate variants table** - Message variants stored in separate table linking to original
* **Option 3: Version field with timestamps** - Timestamp-based versioning for variants

## Decision Outcome

Chosen option: "variant_index + is_active flags", because it provides deterministic ordering via 0-based index, enables unique constraint (session_id, parent_message_id, variant_index), supports active path tracking via is_active flag, keeps variants in message table (no joins needed), and enables efficient sibling queries.

### Consequences

* Good, because variants naturally represented as siblings (same parent_message_id)
* Good, because deterministic ordering (variant_index 0, 1, 2, ...)
* Good, because unique constraint prevents duplicate variants
* Good, because is_active flag marks current variant in UI
* Good, because variant position calculation simple (index + total count)
* Good, because no separate table or joins needed for variant queries
* Bad, because variant_index must be calculated (MAX(variant_index) + 1)
* Bad, because changing active variant requires UPDATE (set old to false, new to true)
* Bad, because deleting variants leaves gaps in variant_index sequence
* Bad, because is_active is session-level concept but stored per message

## Related Design Elements

**Actors**:
* `fdd-chat-engine-actor-client` - Navigates variants, requests position metadata
* `fdd-chat-engine-message-processing` - Assigns variant_index, manages is_active

**Requirements**:
* `fdd-chat-engine-fr-recreate-response` - Creates new variant with incremented variant_index
* `fdd-chat-engine-fr-navigate-variants` - Query siblings, return position metadata
* `fdd-chat-engine-nfr-data-integrity` - Unique constraint on (session_id, parent_message_id, variant_index)

**Design Elements**:
* `fdd-chat-engine-entity-message` - variant_index and is_active fields
* `fdd-chat-engine-db-table-messages` - Unique constraint enforcing variant integrity

**Related ADRs**:
* ADR-0001 (Message Tree Structure) - Variants are siblings in tree
* ADR-0015 (Variant Index for Sequential Navigation) - UI navigation using variant_index
* ADR-0016 (Recreation Creates Variants, Branching Creates Children) - Recreation creates variant (same parent)
