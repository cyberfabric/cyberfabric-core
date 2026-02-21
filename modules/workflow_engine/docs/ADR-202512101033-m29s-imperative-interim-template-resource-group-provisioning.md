# Architecture Decision Record

## Decision Summary
- **Problem**: VHP platform needs to provision resource groups atomically with complex logic (conditions, dependencies, rollback). The platform requires a high-level decision on how resource groups are processed and provisioned.
- **Decision**: Use an imperative interim template format that RMS generates from Resource Group templates and passes to the Workflow Engine for execution via core workflows that orchestrate specialized workers.
- **Rationale**: Centralizes provisioning orchestration logic (order, dependencies, rollback) in RMS rather than workflow, enables early policy validation, maintains clear separation between orchestration (RMS) and execution (workflow), and supports proper reconciliation semantics.
- **Status**: Proposed
- **Date**: 2025-12-10

## Context

### Background
The VHP platform must support provisioning entire resource groups atomically with complex provisioning logic including:
- Conditional resource creation
- Resource dependencies
- Rollback semantics on failures
- Multi-step provisioning workflows

The specific format of the Resource Group template is out of scope for this ADR. This decision focuses on the high-level architecture of how resource groups are processed and provisioned by the platform.

### Drivers / Constraints
- **Multi-tenancy**: Resource provisioning must respect tenant isolation and policies
- **Policy Enforcement**: Policy checks must occur before workflow execution begins (fail-fast)
- **Reconciliation**: Platform must support configuration drift detection and reconciliation
- **Modularity**: Platform is highly modular with specialized workers/workloads via Infrastructure Adapter mechanism
- **Rollback**: Failed provisioning operations must support atomic rollback
- **Observability**: Provisioning workflows must emit audit events and support debugging
- **Workflow Engine**: Platform uses Temporal as workflow engine foundation (see ADR-202512091406-4pd2)

### Assumptions
- RMS (Resource Management Service) receives Resource Group template requests
- Workflow Engine (Temporal-based) is available and operational
- Infrastructure Adapters provide specialized workers/workflows for specific resource types
- Policy decisions can be made synchronously before workflow execution
- Reconciliation can be handled separately from workflow execution

### Out of Scope
- Specific format/schema of Resource Group template
- Specific format/schema of imperative interim template
- Implementation details of specialized workers/workflows
- Infrastructure Adapter mechanism details
- Policy evaluation engine details
- Reconciliation mechanism details

## Options Considered

### Option A — Imperative Interim Template Format (Chosen)
**Description**: RMS parses Resource Group template and transforms it to an imperative interim format that explicitly describes provisioning steps (e.g., "provision disk, then IP address, then container using disk and IP data; if anything fails, rollback in this order"). **RMS decides the provisioning order and rollback logic** during transformation. This imperative format is passed to Workflow Engine for execution by core workflows that execute the steps and orchestrate specialized workers.

**Pros:**
- **Early Policy Validation**: Policy checks occur in RMS before workflow execution, enabling fail-fast behavior
- **Reconciliation Support**: RMS can track desired state from Resource Group template for reconciliation purposes
- **Clear Separation**: RMS handles transformation, orchestration logic (order/rollback), and policy; Workflow Engine executes steps; specialized workers handle resource provisioning
- **Explicit Control Flow**: Imperative format makes dependencies, order, and rollback logic explicit (decided by RMS)
- **Single Source of Truth**: RMS maintains the authoritative desired state from Resource Group template
- **Centralized Control**: RMS has centralized control over provisioning strategy (order, rollback)

**Cons:**
- **Transformation Complexity**: RMS must implement transformation logic from Resource Group template to imperative format
- **Two Formats**: Platform must maintain understanding of both Resource template and imperative interim format

### Option B — Declarative Interim Format
**Description**: RMS transforms Resource Group template to a declarative interim format (e.g., "provision disk1, ip1, container1; container1 depends on disk1 and ip1 and uses their properties"). This declarative format is passed to core workflow. The core workflow determines provisioning order, dependencies, and rollback logic based on the declarative specification.

**Key Difference from Option A**: In Option A, RMS decides the provisioning order and rollback logic (imperative format). In Option B, the core workflow decides provisioning order and rollback logic (declarative format).

**Pros:**
- **Workflow Flexibility**: Core workflow can optimize provisioning order and rollback strategy
- **Separation of Concerns**: RMS focuses on transformation; workflow focuses on orchestration logic
- **Optimization Opportunities**: Workflow can reorder steps for efficiency

**Cons:**
- **Split Logic**: Logic split between RMS (transformation) and core workflow (orchestration), making it harder to reason about end-to-end behavior
- **Late Policy Checks**: Policy validation happens after workflow starts (unless RMS validates before transformation), wasting resources on invalid requests
- **Complex Workflow**: Core workflow must implement complex dependency resolution and rollback logic
- **Reconciliation Complexity**: Harder to reconcile declarative state against actual infrastructure state (workflow must track desired state)
- **Less Explicit**: Declarative format hides explicit control flow, making debugging more difficult

### Option C — Direct Resource Group Template to Workflow
**Description**: Transfer Resource Group template directly to core workflow. RMS does not handle transformation; all logic (parsing, dependency resolution, orchestration) happens inside core workflow.

**Pros:**
- **Single Location**: All logic in one place (core workflow)
- **No Transformation**: No intermediate format needed

**Cons:**
- **Reconciliation Hard**: Workflows are ephemeral; reconciliation requires persistent desired state tracking
- **Late Policy Checks**: Policy validation happens after workflow starts
- **Workflow Complexity**: Core workflow must handle template parsing, policy evaluation, dependency resolution, and orchestration
- **Tight Coupling**: Workflow becomes tightly coupled to Resource Group template format

### Comparison Matrix

| Criteria                | Option A (Imperative Interim) | Option B (Declarative Interim) | Option C (Direct) |
|-------------------------|-------------------------------|-------------------------------|------------------------|
| Policy Check Timing     | ✅ Early (RMS)                | ⚠️ Late (Workflow) or Early (RMS) | ⚠️ Late (Workflow)     |
| Reconciliation Support  | ✅ RMS tracks desired state   | ⚠️ Complex (workflow tracks)  | ❌ Hard (ephemeral)    |
| Logic Separation        | ✅ Clear boundaries           | ⚠️ Split logic (RMS transforms, workflow orchestrates) | ❌ All in workflow     |
| Workflow Complexity     | ✅ Simple (executes steps)    | ⚠️ Complex (resolves dependencies/rollback) | ❌ Very complex        |
| Transformation Overhead | ⚠️ RMS transforms             | ⚠️ RMS transforms             | ✅ None                |
| Explicit Control Flow   | ✅ Imperative steps           | ⚠️ Implicit (workflow decides) | ⚠️ Implicit            |
| Fail-Fast Capability    | ✅ Policy checks early        | ⚠️ After workflow start (unless RMS validates) | ⚠️ After workflow start|
| Who Decides Order/Rollback | ✅ RMS (explicit)            | ⚠️ Core Workflow (implicit)   | ⚠️ Core Workflow       |

## Decision Details

### Chosen Option
**Option A — Imperative Interim Template Format** is selected.

**Why now:**
- VHP platform requires resource group provisioning with complex logic
- Policy enforcement must occur early (before resource consumption)
- Reconciliation requires persistent desired state tracking (RMS responsibility)
- RMS should decide provisioning order and rollback logic (not workflow), enabling centralized control and easier debugging
- Imperative format makes control flow explicit and easier to debug
- Clear separation: RMS handles transformation and orchestration logic; workflow executes steps

### Invariants (MUST)
- **Policy Enforcement**: RMS MUST perform policy checks before workflow execution; fail-closed on policy violations
- **Idempotency**: Imperative interim format MUST support idempotent execution (same input → same result)
- **Atomicity**: Resource group provisioning MUST be atomic (all-or-nothing) with proper rollback
- **Audit Trail**: RMS MUST emit CloudEvents-compatible audit events for template receipt, transformation, and workflow initiation
- **Tenant Isolation**: Imperative format MUST include tenant context; workflows MUST enforce tenant isolation
- **Deterministic Execution**: Imperative format MUST be deterministic (same template → same interim format)
- **Versioning**: Imperative interim format MUST be versioned; support backward compatibility
- **Observability**: All provisioning steps MUST emit metrics, logs, and traces with correlation IDs

### Interfaces / Protocols
- **Resource Group Template**: Input format (JSON/etc.) received by RMS API
- **Imperative Interim Format**: Internal format passed to Workflow Engine (format TBD in design phase)
- **Workflow Protocol**: Temporal workflow execution (see ADR-202512091406-4pd2)
- **Worker Protocol**: Communication between core workflow and specialized workers (TBD in design phase)
- **Events**: CloudEvents-compatible events for provisioning lifecycle

### Impact Radius
- **Services**: RMS (transformation logic), Workflow Engine (core workflows), Infrastructure Adapters (specialized workers)
- **APIs**: RMS API for Resource Group template submission
- **Workflows**: Core provisioning workflow, specialized resource provisioning workflows
- **Policies**: Policy evaluation in RMS before workflow execution
- **Events**: Provisioning lifecycle events (template received, transformation, workflow started/completed/failed)
- **Observability**: Metrics, logs, traces for template transformation and workflow execution
- **Runbooks**: Operational procedures for troubleshooting provisioning failures

## Consequences

### Positive
- **Early Validation**: Policy checks occur before workflow execution, reducing wasted resources
- **Reconciliation Support**: RMS maintains desired state for reconciliation purposes
- **Clear Separation**: Clear boundaries between RMS (transformation, orchestration logic/order/rollback, policy), Workflow Engine (step execution), and workers (resource provisioning)
- **Explicit Control Flow**: Imperative format makes dependencies, order, and rollback explicit
- **Debugging**: Easier to debug provisioning issues with explicit step-by-step format
- **Testability**: Each component (RMS transformation, workflow execution, worker execution) can be tested independently

### Negative / Risks

**Risk 1: Transformation Complexity**
- **Impact**: Medium — RMS must implement transformation logic from Resource Group template to imperative format
- **Likelihood**: High — Transformation logic will be non-trivial
- **Mitigation**: 
  - Design imperative interim format to be simple and explicit
  - Implement transformation incrementally (start with simple templates)
  - Comprehensive test coverage for transformation logic
  - Consider using existing Resource Group template parsers/libraries if available

**Risk 2: Format Maintenance**
- **Impact**: Medium — Platform must maintain understanding of both Resource Group template and imperative interim format
- **Likelihood**: High — Two formats to maintain
- **Mitigation**: 
  - Version imperative interim format explicitly
  - Document format evolution and migration path
  - Consider format validation schemas

**Risk 3: Performance Overhead**
- **Impact**: Low-Medium — Transformation adds latency to provisioning request
- **Likelihood**: Medium — Transformation is synchronous operation
- **Mitigation**: 
  - Optimize transformation logic (caching, parallel processing where possible)
  - Consider async transformation for large templates if needed
  - Monitor transformation latency (SLO: p95 < 100ms)

**Risk 4: Format Evolution**
- **Impact**: Medium — Changes to Resource Group template format or imperative format require updates
- **Likelihood**: Medium — Formats will evolve over time
- **Mitigation**: 
  - Version both formats explicitly
  - Support multiple format versions simultaneously
  - Clear deprecation and migration path

### Neutral / Trade-offs
- **Transformation vs. Direct**: Transformation adds complexity but enables early policy checks and reconciliation support
- **Imperative vs. Declarative**: Imperative format is more explicit but requires more detailed specification. Key difference: In Option A (imperative), RMS decides provisioning order and rollback logic. In Option B (declarative), core workflow decides provisioning order and rollback logic.
- **RMS Responsibility**: RMS takes on transformation and orchestration logic responsibility (deciding order/rollback), but gains centralized control over policy, reconciliation, and provisioning strategy
- **Workflow Complexity**: Imperative format simplifies workflow (just executes steps), while declarative format requires workflow to implement dependency resolution and rollback logic

## Security, Privacy & Compliance

### Threats Addressed
- **Unauthorized Provisioning**: Policy checks in RMS prevent unauthorized resource provisioning before workflow execution
- **Tenant Isolation**: Imperative format includes tenant context; workflows enforce tenant isolation
- **Template Tampering**: RMS validates Resource Group template format and content before transformation
- **Privilege Escalation**: Policy evaluation prevents privilege escalation attempts
- **Audit Trail**: All template receipts, transformations, and workflow initiations are audited

### Mitigations
- **Externalized Authorization**: RMS calls PDP for authorization decisions before workflow execution; fail-closed on errors
- **Input Validation**: RMS validates Resource Group template format, schema, and content before transformation
- **Structured Audit**: All provisioning operations emit CloudEvents-compatible audit events
- **Least Privilege**: Workflows and workers execute with least privilege credentials
- **Encryption**: Resource Group templates and imperative interim format encrypted in transit and at rest

### Data Classification
- **Resource Group Templates**: May contain sensitive resource configurations; encrypted at rest and in transit
- **Imperative Interim Format**: Contains resource provisioning instructions; encrypted at rest and in transit
- **Audit Events**: Contain provisioning metadata; PII minimized where possible

### SOC 2 Mapping
- **CC6.x (Access Control)**: 
  - Policy checks in RMS enforce access control before provisioning
  - Tenant isolation prevents unauthorized cross-tenant access
  - PDP integration ensures proper authorization
- **CC7.x (Change/Operations)**:
  - Audit trail records all template receipts, transformations, and workflow executions
  - Versioning tracks changes to template formats
  - Operational procedures documented in runbooks

## Impact on Interfaces, Events & Data

### APIs
- **RMS Template Submission API**: REST API for submitting Resource Group templates
  - Create OpenAPI stub based on `architecture-workspace/templates/api/openapi.yaml`
  - Endpoints: `POST /api/v1/resource-groups`, `GET /api/v1/resource-groups/{id}`, `DELETE /api/v1/resource-groups/{id}`
- **RMS Template Status API**: REST API for querying provisioning status
  - Endpoints: `GET /api/v1/resource-groups/{id}/status`, `GET /api/v1/resource-groups/{id}/workflow`

### Events
- **Template Lifecycle Events**: CloudEvents-compatible events emitted for template operations
  - Create event catalog + JSON Schemas based on `architecture-workspace/templates/events/`
  - Event types: `resource-group.template.received`, `resource-group.template.transformed`, `resource-group.workflow.started`, `resource-group.workflow.completed`, `resource-group.workflow.failed`, `resource-group.provisioning.completed`, `resource-group.provisioning.failed`
- **Provisioning Audit Events**: Events for audit trail compliance
  - Event types: `resource-group.provisioning.started`, `resource-group.provisioning.rolled-back`

### Compatibility
- **Backwards Compatibility**: Version imperative interim format; support multiple format versions
- **Versioning**: Semantic versioning for Resource Group template format and imperative interim format
- **Migration/Backfill**: Old templates continue to work with transformation logic; new templates use latest format
- **Deprecation Plan**: Deprecated format versions marked; migration path documented

## Ops & NFR Impact

### SLIs/SLOs
- **Template Transformation Latency p95**: ≤ 100ms for Resource Group template to imperative format transformation
- **Workflow Initiation Latency p95**: ≤ 50ms for workflow start after transformation
- **Provisioning Reliability**: ≥ 99.9% successful provisioning rate (excluding policy violations and invalid templates)
- **Policy Check Latency p95**: ≤ 50ms for policy evaluation

### Observability
- **Metrics**: 
  - Template transformation rate, latency, error rate
  - Workflow initiation rate, completion rate, failure rate
  - Policy check latency, rejection rate
  - Resource provisioning latency by resource type
- **Logs**: 
  - Structured logs with correlation IDs for template transformation
  - Workflow execution logs with step-by-step progress
  - Policy evaluation logs (policy decisions, rejections)
- **Traces**: 
  - Distributed tracing across RMS transformation → Workflow Engine → Workers
  - Trace correlation between template submission and provisioning completion

### Rollout / Backout
- **Format Versioning**: Support multiple format versions simultaneously during rollout
  - Deploy new transformation logic alongside existing logic
  - Route new templates to new format; old templates continue with old format
  - Gradual migration: route percentage of templates to new format, monitor, then migrate remaining
  - Rollback: revert to previous format version if issues detected
- **Workflow Deployment**: Deploy core workflows and specialized workers using standard Kubernetes deployment strategies (rolling updates, canary)
- **RMS Deployment**: Blue/green deployment for RMS transformation logic with feature flags

### Capacity & Costs
- **Expected Throughput**: 1,000 resource group provisioning requests/day initially, scaling to 10,000/day
- **Storage**: Template storage in RMS (retention policy: 90 days), workflow history in Temporal (retention: 90 days)
- **Compute**: RMS transformation logic (CPU-bound), workflow workers (I/O-bound)
- **Network**: gRPC traffic between RMS and Workflow Engine; estimated 100MB/day initially
