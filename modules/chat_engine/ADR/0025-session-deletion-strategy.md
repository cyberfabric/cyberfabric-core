# ADR-0025: Session Deletion Strategy (Soft Delete as Default)

**Status**: Accepted

**Date**: 2026-02-06

**Decision Makers**: Architecture Team

**Context**:

Chat Engine needs a deletion strategy that balances multiple competing concerns:

1. **User Safety**: Protect users from accidental data loss
2. **Storage Costs**: Minimize database storage for deleted data
3. **Compliance**: Support GDPR/CCPA right-to-erasure and data minimization
4. **Performance**: Maintain query performance as data grows
5. **Recovery**: Enable restoration of accidentally deleted data

**Decision**:

Implement **soft delete as the default deletion mechanism** with automatic hard deletion after a configurable retention period.

**Key Design Choices:**

1. **Dual Deletion Model**:
   - Soft delete (default): Recoverable, sets lifecycle_state=soft_deleted
   - Hard delete (explicit): Permanent, physically removes from database
   - SessionDeleteRequest accepts deletion_type parameter (default: soft)

2. **Retention Policies**:
   - Configurable per session type
   - Automatic hard delete after soft_delete_retention_days
   - Optional archival for inactive sessions
   - Background job enforces policies daily

3. **Lifecycle States** (4 states):
   - active → archived → soft_deleted → hard_deleted
   - Messages inherit state from session (cascade)
   - Webhook notifications for all transitions

4. **Recovery Window**:
   - Sessions restorable until scheduled_hard_delete_at
   - Explicit restore operation via POST /sessions/:id/restore
   - Clear error messages after grace period expires

**Rationale**:

**Why Soft Delete as Default:**
- User safety prioritized over storage costs
- Industry standard (Gmail, Slack, Google Drive use soft delete)
- Accidental deletion is common user error
- Recovery requests are frequent in production systems
- Storage cost of soft-deleted data is minimal compared to active data

**Why Not Immediate Hard Delete:**
- No recovery from accidental deletion
- Higher support burden (users requesting data recovery)
- Compliance issues (premature deletion before legal hold)
- More aggressive than necessary for most use cases

**Why Retention Policies:**
- Balances safety (grace period) with storage costs (eventual cleanup)
- Supports compliance (automatic data minimization)
- Flexible per session type (different retention for different use cases)
- Reduces manual maintenance burden

**Consequences**:

**Positive:**
- ✅ Users protected from accidental data loss
- ✅ Compliance-friendly (grace period + automatic cleanup)
- ✅ Flexible per session type
- ✅ Industry-standard behavior
- ✅ Auditability via webhook events

**Negative:**
- ❌ Requires background job for cleanup
- ❌ Slightly more storage than immediate deletion
- ❌ More complex implementation than simple DELETE
- ❌ Requires lifecycle state management

**Mitigation:**
- Background job runs during low-traffic periods
- Storage cost is minimal (<5% overhead for typical workloads)
- Complexity contained in session lifecycle module
- Lifecycle state indexed for query performance

**Alternatives Considered**:

**Alternative 1: Immediate Hard Delete (Rejected)**
- Simpler implementation, lower storage costs
- Rejected: Too dangerous, no recovery from accidental deletion
- Industry: This is uncommon pattern, users expect trash/recycle bin

**Alternative 2: External Archival System (Rejected)**
- Move deleted sessions to external archive service (S3, Glacier)
- Rejected: Added complexity, slower recovery, higher latency
- External dependency increases failure modes

**Alternative 3: Soft Delete Only (No Auto Cleanup) (Rejected)**
- Never hard delete, keep soft-deleted sessions indefinitely
- Rejected: Violates data minimization, unbounded storage growth
- Compliance issues (GDPR requires eventual deletion)

**Alternative 4: Dual-State Model (active/deleted) (Rejected)**
- Only 2 states instead of 4 (active/archived/soft_deleted/hard_deleted)
- Rejected: Cannot optimize archived sessions, no archival tier
- Lacks flexibility for different lifecycle stages

**Implementation Notes**:

- Schemas already implement full lifecycle (DeletionType, LifecycleState, RetentionPolicy)
- Session and Message entities have lifecycle_state, deleted_at, scheduled_hard_delete_at fields
- Webhook events: session.soft_deleted, session.hard_deleted, session.restored, session.lifecycle_changed
- Background cleanup job: cron-based, batch processing, idempotent

**References**:

- FR-014: Session Lifecycle Management
- FR-014a: Soft Delete Session
- FR-014b: Hard Delete Session
- FR-014c: Restore Soft-Deleted Session
- FR-014e: Retention Policy Configuration
- Schema: `/modules/chat_engine/schemas/common/LifecycleState.json`
- Schema: `/modules/chat_engine/schemas/common/RetentionPolicy.json`
