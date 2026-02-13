# ADR-0023: PostgreSQL Full-Text Search with GIN Indexes

**Date**: 2026-02-04

**Status**: accepted

**ID**: `fdd-chat-engine-adr-search-strategy`

## Context and Problem Statement

Users need to search conversation history within sessions and across sessions to find relevant messages. How should Chat Engine implement full-text search to balance query performance, relevance ranking, and infrastructure simplicity?

## Decision Drivers

* Search within session (< 1 second for 10K messages)
* Search across sessions (< 3 seconds for 1K sessions)
* Relevance ranking (most relevant results first)
* Case-insensitive search with stemming
* Phrase search support ("exact match")
* Pagination for large result sets
* Infrastructure simplicity (avoid separate search engine)
* Leverage existing PostgreSQL database

## Considered Options

* **Option 1: PostgreSQL tsvector with GIN indexes** - Native full-text search with tsvector, GIN indexing, ts_rank_cd
* **Option 2: Elasticsearch** - Dedicated search engine with advanced features
* **Option 3: Simple LIKE queries** - Basic pattern matching with ILIKE

## Decision Outcome

Chosen option: "PostgreSQL tsvector with GIN indexes", because it provides built-in full-text search without additional infrastructure, GIN indexes enable fast queries meeting performance requirements, ts_rank_cd relevance ranking with document length normalization, case-insensitive search with English stemming, and cursor-based pagination for consistency.

### Consequences

* Good, because no additional infrastructure (search within PostgreSQL)
* Good, because GIN indexes provide fast full-text queries
* Good, because ts_rank_cd ranking considers document length
* Good, because case-insensitive with stemming ("running" matches "run")
* Good, because phrase search supported (to_tsquery with "running & fast")
* Good, because pagination with cursors (created_at + message_id)
* Good, because client_id partitioning prevents noisy neighbors
* Bad, because less feature-rich than Elasticsearch (no typo tolerance, advanced ranking)
* Bad, because GIN indexes consume storage (additional ~20% per index)
* Bad, because index updates add write latency (~5ms per message)
* Bad, because cross-language stemming limited (English default)

## Related Design Elements

**Actors**:
* `fdd-chat-engine-actor-client` - Submits search queries, receives ranked results
* `fdd-chat-engine-message-search` - Executes full-text queries

**Requirements**:
* `fdd-chat-engine-fr-search-session` - Session-scoped full-text search
* `fdd-chat-engine-fr-search-sessions` - Cross-session full-text search
* `fdd-chat-engine-nfr-search` - Performance requirements (1s session, 3s cross-session)

**Design Elements**:
* `fdd-chat-engine-db-table-messages` - Full-text index on content field
* `fdd-chat-engine-design-context-search` - Implementation details (tsvector, GIN, ts_rank_cd)
* HTTP POST /sessions/{id}/search and POST /search endpoints

**Related ADRs**:
* ADR-0010 (Stateless Horizontal Scaling with Database State) - PostgreSQL full-text search features
* ADR-0020 (Session Metadata JSONB for Extensibility) - Search includes metadata fields (title, tags)
