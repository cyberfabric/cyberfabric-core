# Architecture Decision Record

## Decision Summary
- **Problem**: Need to select a workflow orchestration engine foundation that supports long-running operations, durable execution, retries, and saga patterns for multi-tenant VHP platform services.
- **Decision**: Adopt Temporal as the workflow engine foundation for orchestrating complex, long-running business processes across VHP services.
- **Rationale**: Temporal provides battle-tested durability, built-in saga patterns, strong observability, and is already used by sister companies (Constructor, Acronis), enabling knowledge sharing and operational consistency.
- **Status**: Proposed
- **Date**: 2025-12-09

## Context

### Background
VHP platform requires a workflow orchestration engine to handle complex, long-running business processes that span multiple services. These workflows need to support:
- Durable execution across service boundaries
- Long-running operations (days/months/years)
- Retry and error handling with configurable policies
- Saga patterns for distributed transactions
- Multi-tenant isolation
- Strong observability and debugging capabilities

Currently, VHP platform has no workflow orchestration engine. This ADR establishes the foundation for introducing workflow orchestration capabilities as a new feature.

### Drivers / Constraints
- **Multi-tenancy**: Strict tenant isolation required; separate namespaces/task queues per tenant
- **Durability**: Workflows must survive service restarts, network partitions, and infrastructure failures
- **Observability**: Rich visibility into workflow state, history, and debugging capabilities essential for operations
- **Language Support**: Rust is the primary language requirement (VHP is Rust-first); support for Go, Java, Python, TypeScript, .NET is optional
- **Operational Maturity**: Prefer battle-tested solutions with proven production track records
- **Knowledge Sharing**: Leverage existing expertise from sister companies (Constructor, Acronis)
- **SOC 2 Compliance**: Workflow engine must support audit trails, secure multi-tenant isolation, and encryption at rest

### Assumptions
- Temporal cluster can be deployed and operated within VHP infrastructure
- Workflow definitions will be versioned and managed alongside service code
- Configuration drift/reconciliation will be handled separately (Temporal focuses on task execution, not infrastructure reconciliation)
- Temporal Rust SDK alpha/beta versions are acceptable for initial adoption; official Rust SDK is actively being developed (see references)

### Out of Scope
- Low-code/no-code workflow authoring tools (addressed separately if needed)
- Infrastructure as Code (IaC) reconciliation (Crossplane functions address this separately)
- Real-time event streaming (handled by event bus infrastructure)
- Short-lived request/response patterns (handled by API Gateway and service mesh)

### References
- Temporal documentation: https://temporal.io
- Temporal Rust SDK proposal: https://github.com/temporalio/proposals/pull/102
- Temporal Rust SDK community discussion: https://community.temporal.io/t/rust-sdk-for-temporal/1334/26
- Workflow Executors investigation: https://virtuozzo.atlassian.net/wiki/spaces/VHP/pages/4035280900/Workflow+Executors
- Related ADRs: TBD (to be linked when created)

## Options Considered

### Option A — Temporal
**Pros:**
- Mature, battle-tested (Uber origin), large community
- Native support for long-running workflows (days/months/years)
- Built-in saga pattern, compensation logic, automatic rollback
- Rich observability: Web UI, workflow timeline, step-by-step replay, visibility APIs
- Built-in idempotency keys, deterministic replay
- Configurable retry policies: exponential backoff, max attempts, non-retryable errors, timeouts
- Workflow versioning API, deterministic replay for backward compatibility
- Used by sister companies (Constructor, Acronis) — enables knowledge sharing
- Multi-tenant isolation via separate namespaces and task queues per tenant
- Strong security: mTLS, authentication plugins (OIDC, LDAP), encryption at rest, namespace authorization
- Declarative workflow support via Temporal DSL (YAML/JSON), or imperative with SDK
- Horizontal worker scaling, partitioned workflows, multi-cluster support
- SDK test frameworks, workflow mocking, activity stubs, replay tests

**Cons:**
- Does not cover configuration drift/reconciliation (focuses on task execution)
- Rust SDK is in alpha/beta phase (official Rust SDK Phase 1 proposal approved and actively being developed)
- No low-code/no-code solution (workflow definitions in code)
- Temporal DSL still in progress (project recently renamed)
- Learning curve: Medium (workflow concepts, SDK patterns, deterministic constraints)

### Option B — Crossplane Functions
**Pros:**
- Crossplane is very flexible and mature
- Fully declarative via Compositions and XRDs
- Built-in drift detection via reconciliation loops
- Good for Infrastructure as Code (IaC) use cases

**Cons:**
- Major limitation: Functions cannot be easily invoked on resource deletion (requires workarounds like finalizers)
- Limited workflow orchestration capabilities
- Limited multi-tenancy support
- Functions run per reconciliation cycle (not designed for long-running operations)
- One function container per execution (scalability concerns)
- Not designed for complex business process workflows

### Option C — Knative (Functions + Eventing)
**Pros:**
- CNCF Graduated, mature
- Auto-scaling from 0 (KPA/HPA)
- CloudEvents support

**Cons:**
- Too low-level component without ability to control executions efficiently
- Requires something on top like OpenFunction or Direktiv
- No built-in declarative workflow
- No drift-detection
- Stateless (relies on external stores)
- Application-level idempotency required
- No built-in saga support

### Option D — KubeVela
**Pros:**
- CNCF Sandbox, growing adoption
- Fully declarative via CUE-based workflow definitions
- Multi-cluster orchestration
- Good workflow primitives

**Cons:**
- CUE language has learning curve
- OAM model complexity
- Workflow restarts only if configuration changed
- Limited to application delivery use cases

### Option E — Dapr Workflows
**Pros:**
- CNCF Graduated
- Native support for long-running workflows
- Built-in idempotency, deterministic replay
- Event-sourced durable state

**Cons:**
- No built-in declarative workflow
- Not built-in drift-detection
- Limited workflow orchestration primitives compared to Temporal

### Comparison Matrix

| Criteria                | Temporal | Crossplane | Knative | KubeVela | Dapr |
|-------------------------|----------|------------|---------|----------|------|
| Long-running ops        | ✅ Native | ❌ Per reconciliation | ⚠️ Limited | ✅ Supported | ✅ Native |
| Saga/Compensation        | ✅ Built-in | ❌ Not built-in | ❌ Not built-in | ⚠️ Workflow-level | ❌ Not built-in |
| Observability            | ✅ Excellent | ⚠️ Basic | ⚠️ Basic | ⚠️ Moderate | ⚠️ Basic |
| Multi-tenancy            | ✅ Namespaces/queues | ⚠️ Limited | ✅ Namespace isolation | ✅ Namespace/cluster | ✅ Namespace |
| Maturity                 | ✅ Mature | ✅ Mature | ✅ Mature | ⚠️ Sandbox | ✅ Graduated |
| Drift Detection          | ❌ Not built-in | ✅ Built-in | ❌ Not built-in | ⚠️ Config-based | ❌ Not built-in |
| Language Support         | ✅ Rust (alpha/beta), Many others | ⚠️ Python template | ✅ Any containerized | ⚠️ CUE for defs | ✅ Many |
| Learning Curve           | ⚠️ Medium | ❌ High | ⚠️ Medium-High | ⚠️ Medium-High | ⚠️ Medium |
| Knowledge Sharing        | ✅ Sister companies | ❌ None | ❌ None | ❌ None | ❌ None |

### Five Quality Vectors Analysis

Evaluation of considered options using the [Five Quality Vectors method](https://virtuozzo.atlassian.net/wiki/spaces/Quality/pages/3878846569/Five+Quality+Vectors) (sorted by importance for Virtuozzo partners and customers):

#### 1. Efficiency (TCO: Datacenter, Hardware, Software, Operations)

| Option | Assessment | Rationale |
|--------|------------|-----------|
| **Temporal** | ✅ **High** | - Low operational overhead: mature, battle-tested platform reduces operational headcount<br>- Efficient resource utilization: horizontal worker scaling<br>- Minimal infrastructure footprint: single cluster deployment<br>- Knowledge sharing with sister companies reduces learning curve costs<br>- Rust SDK (alpha/beta) aligns with VHP's Rust-first approach, reducing integration costs |
| **Crossplane** | ⚠️ **Medium** | - Higher operational complexity: requires deep K8s expertise<br>- One function container per execution limits resource efficiency<br>- Limited multi-tenancy increases operational overhead<br>- Higher learning curve increases training costs |
| **Knative** | ⚠️ **Medium** | - Requires additional layers (OpenFunction/Direktiv) increasing TCO<br>- Auto-scaling from 0 is efficient but requires K8s infrastructure<br>- Stateless design requires external storage costs<br>- Medium-high learning curve |
| **KubeVela** | ⚠️ **Medium** | - CUE language learning curve increases training costs<br>- Multi-cluster orchestration adds infrastructure complexity<br>- OAM model complexity increases operational overhead |
| **Dapr** | ⚠️ **Medium** | - Sidecar pattern increases resource overhead per pod<br>- Additional infrastructure components (state stores, pub/sub) increase TCO<br>- Medium learning curve |

#### 2. Reliability (Maturity, Availability, Fault Tolerance, Recoverability)

| Option | Assessment | Rationale |
|--------|------------|-----------|
| **Temporal** | ✅ **Excellent** | - **Maturity**: Battle-tested at Uber scale, mature ecosystem<br>- **Availability**: Event-sourced durable state, automatic persistence, workflows survive restarts<br>- **Fault Tolerance**: Built-in retry policies, exponential backoff, saga patterns with compensation<br>- **Recoverability**: Deterministic replay, workflow versioning, automatic rollback on failures<br>- **MTTR**: Rich observability (Web UI, timeline, replay) enables fast incident resolution |
| **Crossplane** | ✅ **Good** | - **Maturity**: CNCF Graduated, mature<br>- **Availability**: Reconciliation-based, eventual consistency<br>- **Fault Tolerance**: Built-in drift detection and correction<br>- **Recoverability**: Reconciliation loops ensure state recovery<br>- **Limitation**: Functions not invoked on resource deletion (requires workarounds) |
| **Knative** | ⚠️ **Moderate** | - **Maturity**: CNCF Graduated<br>- **Availability**: Stateless design relies on external stores (single point of failure)<br>- **Fault Tolerance**: Basic retry/delivery spec, no built-in saga support<br>- **Recoverability**: Application-level implementation required |
| **KubeVela** | ⚠️ **Moderate** | - **Maturity**: CNCF Sandbox (less mature)<br>- **Availability**: Workflow restarts only if configuration changed<br>- **Fault Tolerance**: Rollback to previous version on failures<br>- **Recoverability**: Application revisions provide recovery mechanism |
| **Dapr** | ⚠️ **Moderate** | - **Maturity**: CNCF Graduated<br>- **Availability**: Event-sourced durable state<br>- **Fault Tolerance**: Built-in retry, idempotency<br>- **Recoverability**: Deterministic replay<br>- **Limitation**: Limited workflow orchestration primitives |

#### 3. Performance (Latency, Throughput, Scalability, Overhead)

| Option | Assessment | Rationale |
|--------|------------|-----------|
| **Temporal** | ✅ **Excellent** | - **Latency**: gRPC protocol, efficient task queue processing<br>- **Throughput**: Horizontal worker scaling, partitioned workflows, multi-cluster support<br>- **Scalability**: Auto-scaling workers, supports high-throughput scenarios<br>- **Overhead**: Minimal overhead for workflow orchestration, efficient state management<br>- **Predictability**: Deterministic execution ensures consistent performance |
| **Crossplane** | ⚠️ **Moderate** | - **Latency**: Per-reconciliation cycle (not designed for real-time)<br>- **Throughput**: One function container per execution limits throughput<br>- **Scalability**: Scales with K8s control plane (bottleneck risk)<br>- **Overhead**: Function execution overhead per reconciliation |
| **Knative** | ✅ **Good** | - **Latency**: HTTP-based, auto-scaling from 0<br>- **Throughput**: Auto-scaling (KPA/HPA), concurrent revisions<br>- **Scalability**: Excellent auto-scaling capabilities<br>- **Overhead**: Minimal when scaled to 0, but requires K8s infrastructure |
| **KubeVela** | ⚠️ **Moderate** | - **Latency**: CUE-based workflow definitions add processing overhead<br>- **Throughput**: Multi-cluster support but limited by OAM model<br>- **Scalability**: Application-level scaling via traits<br>- **Overhead**: OAM model complexity adds overhead |
| **Dapr** | ⚠️ **Moderate** | - **Latency**: Sidecar pattern adds network hop overhead<br>- **Throughput**: Sidecar communication via gRPC<br>- **Scalability**: Horizontal worker scaling<br>- **Overhead**: Sidecar per pod increases resource consumption |

#### 4. Security (Isolation, Privacy, Transparency)

| Option | Assessment | Rationale |
|--------|------------|-----------|
| **Temporal** | ✅ **Excellent** | - **Isolation**: Separate namespaces and task queues per tenant, strict multi-tenant isolation<br>- **Privacy**: mTLS, OIDC/LDAP authentication, encryption at rest, namespace authorization<br>- **Transparency**: Rich audit trail (workflow history), visibility APIs, structured logging<br>- **Compliance**: Supports SOC 2 requirements (CC6.x, CC7.x) |
| **Crossplane** | ⚠️ **Moderate** | - **Isolation**: Namespace isolation, limited multi-tenancy support<br>- **Privacy**: RBAC, provider credentials isolation<br>- **Transparency**: K8s events, status conditions<br>- **Limitation**: Limited audit trail for external consumers |
| **Knative** | ✅ **Good** | - **Isolation**: Namespace isolation, network policies<br>- **Privacy**: mTLS via Istio/Kourier, RBAC<br>- **Transparency**: CloudEvents, basic observability<br>- **Limitation**: Application-level security implementation required |
| **KubeVela** | ⚠️ **Moderate** | - **Isolation**: Namespace/cluster isolation<br>- **Privacy**: RBAC, traits for security policies<br>- **Transparency**: Application events, workflow step events<br>- **Limitation**: Security depends on underlying infrastructure |
| **Dapr** | ✅ **Good** | - **Isolation**: Namespace isolation<br>- **Privacy**: mTLS, authentication plugins<br>- **Transparency**: Metrics, tracing<br>- **Limitation**: Sidecar security model adds complexity |

#### 5. Versatility (Workload Support, Access & Integration, Hardware & Deployment)

| Option | Assessment | Rationale |
|--------|------------|-----------|
| **Temporal** | ✅ **Excellent** | - **Workload Support**: Supports any workflow type (long-running, complex business processes)<br>- **Access & Integration**: gRPC protocol, REST Visibility API, CloudEvents integration<br>- **Hardware & Deployment**: Runs on K8s, supports multi-cluster, flexible deployment<br>- **Language Support**: Rust (alpha/beta), Go, Java, Python, TypeScript, .NET<br>- **Extensibility**: Workflow versioning API, deterministic replay enables evolution |
| **Crossplane** | ⚠️ **Limited** | - **Workload Support**: Limited to IaC use cases, not designed for business workflows<br>- **Access & Integration**: K8s-native, gRPC functions<br>- **Hardware & Deployment**: K8s-based, requires provider ecosystem<br>- **Language Support**: Python template, limited language options |
| **Knative** | ⚠️ **Limited** | - **Workload Support**: Serverless functions, event-driven apps, request/response<br>- **Access & Integration**: HTTP, CloudEvents<br>- **Hardware & Deployment**: K8s-based, requires additional layers for workflows<br>- **Language Support**: Any containerized language<br>- **Limitation**: Too low-level, requires abstraction layers |
| **KubeVela** | ⚠️ **Moderate** | - **Workload Support**: Application delivery, multi-cloud deployments<br>- **Access & Integration**: CUE-based, multi-cluster<br>- **Hardware & Deployment**: Multi-cluster, flexible deployment<br>- **Language Support**: CUE for definitions, any for workloads<br>- **Limitation**: Limited to application delivery scenarios |
| **Dapr** | ⚠️ **Moderate** | - **Workload Support**: Distributed applications, long-running workflows<br>- **Access & Integration**: Sidecar pattern, gRPC<br>- **Hardware & Deployment**: K8s-based, sidecar deployment<br>- **Language Support**: Go, Java, Python, JavaScript, .NET, C++, Rust (poorly maintained)<br>- **Limitation**: Limited workflow orchestration primitives |

#### Summary by Quality Vector

| Quality Vector | Temporal | Crossplane | Knative | KubeVela | Dapr |
|----------------|----------|-----------|---------|----------|------|
| **1. Efficiency** | ✅ High | ⚠️ Medium | ⚠️ Medium | ⚠️ Medium | ⚠️ Medium |
| **2. Reliability** | ✅ Excellent | ✅ Good | ⚠️ Moderate | ⚠️ Moderate | ⚠️ Moderate |
| **3. Performance** | ✅ Excellent | ⚠️ Moderate | ✅ Good | ⚠️ Moderate | ⚠️ Moderate |
| **4. Security** | ✅ Excellent | ⚠️ Moderate | ✅ Good | ⚠️ Moderate | ✅ Good |
| **5. Versatility** | ✅ Excellent | ⚠️ Limited | ⚠️ Limited | ⚠️ Moderate | ⚠️ Moderate |

**Conclusion**: Temporal scores highest across all five quality vectors, particularly excelling in Reliability, Performance, Security, and Versatility—the most critical vectors for VHP platform's workflow orchestration needs.

## Decision Details

### Chosen Option
**Temporal** is selected as the workflow engine foundation for VHP platform.

**Why now:**
- VHP platform requires durable workflow orchestration for complex business processes
- Temporal provides the best balance of maturity, features, and operational capabilities
- Existing usage by sister companies (Constructor, Acronis) enables knowledge sharing and reduces operational risk
- Temporal's focus on durable task execution aligns with VHP's needs, while configuration drift/reconciliation can be handled separately via Crossplane or other IaC tools

### Invariants (MUST)
- **Multi-tenant isolation**: Separate Temporal namespaces and task queues per tenant; never share workflow state across tenants
- **Externalized authorization**: Workflow activities must call PDP for authorization decisions; fail-closed on authorization errors
- **Idempotency**: All workflow activities must be idempotent; use Temporal's built-in idempotency keys
- **Audit trail**: Emit CloudEvents-compatible audit events for workflow state transitions and material state changes
- **Deterministic execution**: Workflow code must be deterministic (no random, time-based, or external state access in workflow logic)
- **Versioning**: Workflow definitions must be versioned; use Temporal's workflow versioning API for backward compatibility
- **Encryption**: Temporal cluster must use encryption at rest and mTLS for inter-service communication
- **Observability**: All workflows must emit metrics, logs, and traces with correlation IDs

### Interfaces / Protocols
- **Workflow Definitions**: Code-based (Rust primary, Go/Java/Python/TypeScript/.NET optional) or Temporal DSL (YAML/JSON) when available
- **Worker Protocol**: gRPC-based communication with Temporal cluster
- **Visibility API**: REST API for querying workflow state, history, and metrics
- **Events**: CloudEvents-compatible events emitted for workflow state changes
- **Authentication**: OIDC/LDAP authentication plugins for Temporal cluster access

### Impact Radius
- **Services**: All VHP services that require workflow orchestration (RMS, AMS, CPMS, BSS services)
- **Infrastructure**: Temporal cluster deployment, worker pods, namespace management
- **Policies**: RBAC policies for Temporal namespace access, workflow execution policies
- **CI/CD**: Workflow definition testing, versioning, and deployment pipelines
- **Runbooks**: Operational procedures for Temporal cluster management, workflow debugging, and incident response
- **Observability**: Metrics, logs, and traces integration with VHP observability stack

## Consequences

### Positive
- **Durability**: Workflows survive service restarts, network partitions, and infrastructure failures
- **Operational Excellence**: Rich observability and debugging capabilities reduce MTTR
- **Knowledge Sharing**: Leverage expertise from sister companies reduces learning curve and operational risk
- **Developer Experience**: Strong SDK support and testing frameworks improve developer productivity
- **Scalability**: Horizontal worker scaling and partitioned workflows support high-throughput scenarios
- **Reliability**: Built-in retry policies, saga patterns, and compensation logic reduce manual error handling

### Negative / Risks

**Risk 1: Rust SDK Maturity**
- **Impact**: Low-Medium — Rust SDK is in alpha/beta phase; may have limitations or require workarounds
- **Likelihood**: Medium — Official Rust SDK Phase 1 proposal approved (November 2025) and actively being developed
- **Mitigation**: 
  - Use alpha/beta Rust SDK with understanding of potential limitations
  - Monitor official Rust SDK development progress (https://github.com/temporalio/proposals/pull/102)
  - Contribute feedback and improvements to Temporal Rust SDK as needed
  - Fallback to other language SDKs via gRPC/HTTP if critical issues arise (unlikely given active development)

**Risk 2: Configuration Drift/Reconciliation Not Covered**
- **Impact**: Medium — Temporal focuses on task execution, not infrastructure reconciliation
- **Likelihood**: High — This is by design
- **Mitigation**: 
  - Use Crossplane functions or other IaC tools for infrastructure reconciliation
  - Clearly separate concerns: Temporal for workflow orchestration, Crossplane for IaC

**Risk 3: Learning Curve**
- **Impact**: Medium — Team needs to learn Temporal concepts and deterministic constraints
- **Likelihood**: Medium — Temporal has medium learning curve
- **Mitigation**: 
  - Leverage knowledge from sister companies (Constructor, Acronis)
  - Provide training and documentation
  - Start with simple workflows and gradually increase complexity

**Risk 4: Temporal DSL Still in Progress**
- **Impact**: Low — Declarative workflows are nice-to-have, not required
- **Likelihood**: Medium — DSL project recently renamed, status unclear
- **Mitigation**: 
  - Use code-based workflow definitions initially
  - Monitor DSL development; adopt when stable

**Risk 5: Operational Overhead**
- **Impact**: Medium — Temporal cluster requires operational management
- **Likelihood**: Medium — Additional infrastructure to operate
- **Mitigation**: 
  - Use managed Temporal Cloud if available
  - Document operational procedures in runbooks
  - Integrate with VHP observability stack

### Neutral / Trade-offs
- **Declarative vs Imperative**: Temporal supports both declarative (DSL) and imperative (SDK) approaches; start with SDK, adopt DSL when stable
- **Workflow State vs External State**: Temporal manages workflow state; business state remains in service databases
- **Task Execution vs Infrastructure**: Temporal handles task execution; infrastructure reconciliation handled separately

## Security, Privacy & Compliance

### Threats Addressed
- **Spoofing**: mTLS and OIDC/LDAP authentication prevent unauthorized access
- **Tampering**: Workflow state is immutable and versioned; audit trail prevents tampering
- **Privilege Escalation**: Namespace-level RBAC prevents cross-tenant access
- **Data Leakage**: Multi-tenant isolation via separate namespaces and task queues
- **Denial of Service**: Rate limiting on task queues and worker resource limits prevent DoS

### Mitigations
- **Externalized Authorization**: Workflow activities call PDP for authorization decisions; fail-closed on errors
- **Structured Audit**: All workflow state transitions emit CloudEvents-compatible audit events
- **Least Privilege**: RBAC policies enforce least privilege access to Temporal namespaces
- **Input Validation**: Workflow inputs validated before execution
- **Encryption**: Encryption at rest and mTLS for inter-service communication

### Data Classification
- **Workflow State**: May contain PII depending on workflow type; workflow state encrypted at rest
- **Audit Events**: Contain workflow execution metadata; PII minimized where possible
- **Masking/Anonymization**: PII masking applied to audit logs and visibility API responses

### SOC 2 Mapping
- **CC6.x (Access Control)**: 
  - Temporal namespace RBAC controls access to workflows
  - OIDC/LDAP authentication ensures proper identity verification
  - Multi-tenant isolation prevents unauthorized cross-tenant access
- **CC7.x (Change/Operations)**:
  - Workflow versioning API tracks changes to workflow definitions
  - Audit trail records all workflow state transitions
  - Operational procedures documented in runbooks

## Impact on Interfaces, Events & Data

### APIs
- **Temporal Visibility API**: REST API for querying workflow state, history, and metrics
  - Create OpenAPI stub based on `architecture-workspace/templates/api/openapi.yaml`
  - Endpoints: `/workflows/{workflow_id}`, `/workflows/{workflow_id}/history`, `/workflows/{workflow_id}/query`
- **Workflow Management API**: REST API for starting, canceling, and signaling workflows
  - Endpoints: `/workflows/start`, `/workflows/{workflow_id}/cancel`, `/workflows/{workflow_id}/signal`

### Events
- **Workflow State Change Events**: CloudEvents-compatible events emitted for workflow state transitions
  - Create event catalog + JSON Schemas based on `architecture-workspace/templates/events/`
  - Event types: `workflow.started`, `workflow.completed`, `workflow.failed`, `workflow.canceled`, `activity.completed`, `activity.failed`
- **Workflow Audit Events**: Events for audit trail compliance
  - Event types: `workflow.execution.started`, `workflow.execution.completed`, `workflow.execution.failed`

### Compatibility
- **Backwards Compatibility**: Workflow versioning API ensures backward compatibility
- **Versioning**: Semantic versioning for workflow definitions
- **Migration/Backfill**: Workflow definitions versioned; old workflows continue with old versions
- **Deprecation Plan**: Deprecated workflow versions marked; new workflows cannot use deprecated versions

## Ops & NFR Impact

### SLIs/SLOs
- **Workflow Execution Latency p95**: ≤ 100ms for workflow start, ≤ 50ms for activity execution
- **Workflow Reliability**: ≥ 99.9% workflow completion rate (excluding business logic failures)
- **Visibility API Latency p95**: ≤ 200ms for workflow state queries
- **Temporal Cluster Availability**: ≥ 99.95% uptime

### Observability
- **Metrics**: 
  - Workflow execution rate, completion rate, failure rate
  - Activity execution latency, retry count
  - Worker pool size, task queue depth
  - Temporal cluster health metrics
- **Logs**: 
  - Structured logs with correlation IDs for workflow executions
  - Activity execution logs with input/output (PII masked)
  - Temporal cluster operational logs
- **Traces**: 
  - Distributed tracing across workflow activities
  - Trace correlation between workflow and service calls

### Rollout / Backout
- **Workflow Versioning**: Use Temporal's built-in workflow versioning API for safe rollouts and rollbacks
  - Deploy new workflow versions alongside existing versions
  - New workflow executions use new version; in-flight workflows continue with their version
  - Gradual migration: route new workflows to new version, monitor, then migrate existing workflows
  - Rollback: revert to previous workflow version for new executions if issues detected
- **Worker Deployment**: Deploy Temporal workers using standard Kubernetes deployment strategies (rolling updates, canary)
- **Cluster Upgrades**: Blue/green deployment for Temporal cluster upgrades with workflow history preservation

### Capacity & Costs
- **Expected Throughput**: 10,000 workflows/day initially, scaling to 100,000 workflows/day
- **Storage**: Workflow history stored in Temporal cluster; retention policy: 90 days
- **Compute**: Worker pods scale based on task queue depth (HPA)
- **Network**: gRPC traffic between workers and Temporal cluster; estimated 1GB/day initially
