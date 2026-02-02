# ADR-0015: Variant Index for Sequential Navigation

**Date**: 2026-02-04

**Status**: accepted

**ID**: `fdd-chat-engine-adr-variant-indexing`

## Context and Problem Statement

Users need to navigate between message variants (alternative responses to same question). How should variants be ordered and identified to enable intuitive sequential navigation (previous/next variant) and clear position indicators?

## Decision Drivers

* Intuitive navigation (variant 1 of 3, previous/next buttons)
* Deterministic ordering (not random or timestamp-based)
* Efficient queries (find next/previous variant)
* Position calculation simple (current index + total count)
* Support for unlimited variants (no fixed array size)
* Stable ordering (doesn't change as variants added)
* Database indexing efficient
* UI affordances clear (2 of 5)

## Considered Options

* **Option 1: 0-based variant_index** - Integer field starting at 0, incremented per variant
* **Option 2: UUID-based ordering** - UUIDs sorted lexicographically
* **Option 3: Timestamp-based ordering** - created_at determines order

## Decision Outcome

Chosen option: "0-based variant_index", because it provides intuitive sequential ordering (0, 1, 2, ...), enables simple position calculation (index + 1 of total), supports efficient next/previous queries (WHERE variant_index = current ± 1), maintains stable ordering independent of creation time, and maps naturally to UI navigation (variant 2 of 5).

### Consequences

* Good, because intuitive numbering (variant 1, 2, 3 for users)
* Good, because simple position calculation (SELECT COUNT(*) for total)
* Good, because efficient next/previous queries (variant_index ± 1)
* Good, because stable ordering (independent of creation time)
* Good, because database indexing straightforward (INTEGER index)
* Good, because UI naturally shows "2 of 5" from index and count
* Bad, because variant_index calculation requires MAX query (find highest index)
* Bad, because gaps possible if variants deleted (index 0, 1, 3 after delete)
* Bad, because no semantic meaning (index 0 not necessarily "best")
* Bad, because reordering requires UPDATE (change all indices)

## Related Design Elements

**Actors**:
* `fdd-chat-engine-actor-client` - Requests next/previous variant, displays position
* `fdd-chat-engine-message-processing` - Calculates variant_index for new variants

**Requirements**:
* `fdd-chat-engine-fr-navigate-variants` - Query API returns position metadata
* `fdd-chat-engine-fr-recreate-response` - New variant gets incremented index

**Design Elements**:
* `fdd-chat-engine-entity-message` - variant_index field (INTEGER, 0-based)
* `fdd-chat-engine-db-table-messages` - Unique constraint (session_id, parent_message_id, variant_index)

**Related ADRs**:
* ADR-0007 (Variant Management) - variant_index is core field for variants
* ADR-0007 (Recreation vs New Branch) - Recreation increments variant_index
* ADR-0007 (Parallel Multi-Backend) - Multiple backends create consecutive variant_index values
