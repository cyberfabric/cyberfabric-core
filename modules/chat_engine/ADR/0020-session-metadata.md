# ADR-0020: Session Metadata JSONB for Extensibility

**Date**: 2026-02-04

**Status**: accepted

**ID**: `fdd-chat-engine-adr-session-metadata`

## Context and Problem Statement

Sessions need additional metadata beyond core fields (session_id, client_id, session_type_id). Examples include user-defined titles, tags, custom fields, summaries, or application-specific data. How should Chat Engine store extensible metadata without frequent schema changes?

## Decision Drivers

* Extensibility without schema migrations (add new metadata fields easily)
* Support user-defined titles and tags for organization
* Store session summaries for quick previews
* Enable application-specific custom fields
* Query capabilities for common metadata (title, tags)
* JSON schema flexibility for evolving requirements
* Efficient storage for sparse data
* Index support for frequently queried fields

## Considered Options

* **Option 1: JSONB metadata column** - Single JSONB field storing arbitrary key-value pairs
* **Option 2: Fixed columns** - Add columns for title, tags, summary, etc.
* **Option 3: Metadata table** - Separate key-value table with FK to sessions

## Decision Outcome

Chosen option: "JSONB metadata column", because it enables schema-free extensibility (add metadata without migrations), supports PostgreSQL JSONB indexing (GIN index for tags), provides flexible storage for evolving needs, efficiently handles sparse data, and maintains simple session table schema.

### Consequences

* Good, because add new metadata fields without schema migrations
* Good, because JSONB supports flexible structure (title, tags, summary, custom)
* Good, because PostgreSQL GIN indexes enable efficient metadata queries
* Good, because sparse data efficient (only store present fields)
* Good, because JSON operators for querying (->>, @>, ? for tag search)
* Good, because schema evolution simple (clients add new fields)
* Bad, because no schema enforcement (typos possible: "titel" vs "title")
* Bad, because metadata structure not self-documenting (need external docs)
* Bad, because complex queries less efficient than normalized columns
* Bad, because type validation at application level (not database level)

## Related Design Elements

**Actors**:
* `fdd-chat-engine-actor-client` - Sets session metadata (title, tags, custom fields)
* `fdd-chat-engine-session-management` - Manages metadata updates

**Requirements**:
* `fdd-chat-engine-fr-search-sessions` - Search includes session metadata (title, tags)
* `fdd-chat-engine-fr-session-summary` - Summary stored in metadata

**Design Elements**:
* `fdd-chat-engine-entity-session` - metadata field (JSONB)
* `fdd-chat-engine-db-table-sessions` - metadata column with GIN index
* WebSocket session.get returns metadata

**Related ADRs**:
* ADR-0007 (Database Architecture) - PostgreSQL JSONB support
* ADR-0007 (Search Strategy) - Full-text search includes metadata fields
