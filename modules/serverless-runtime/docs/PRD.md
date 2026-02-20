<!-- cpt:#:prd -->
# PRD

<!-- cpt:##:overview -->
## 1. Overview

<!-- cpt:paragraph:purpose -->
**Purpose**: Provide a platform capability that enables tenants and their users to create, modify, register, and execute custom automation (functions and workflows) at runtime, without requiring a product rebuild or redeploy, while maintaining strong isolation, governance, and operational visibility.
<!-- cpt:paragraph:purpose -->

<!-- cpt:paragraph:context -->
The platform requires a unified way to automate long-running and multi-step business processes across modules and external systems. Today, automation capability is limited by release cycles and lacks durable, tenant-isolated execution with governance, controls, and observability. This PRD defines the business requirements for a Serverless Runtime capability that is deliberately implementation-agnostic — the requirements can be satisfied by embedded interpreters (e.g., Starlark, WASM), workflow orchestration engines (e.g., Temporal, Cadence), cloud-native FaaS platforms (e.g., AWS Lambda + Step Functions), or other serverless/workflow technologies. The system MAY support domain-specific languages (DSLs) for workflow/function definition; DSL support is implementation-specific and not required to satisfy these business requirements.
<!-- cpt:paragraph:context -->

**Target Users**:
<!-- cpt:list:target-users required="true" -->
- Application developers who provide custom functions and workflows for integrations at runtime, isolate orchestration logic, and reduce direct dependencies between services
- Tenant administrators who manage automation assets, schedules, permissions, and governance at the tenant level
- Tenant users who create and manage personal workflow automations within their user scope
- Platform operators who monitor, maintain, and develop the serverless runtime environment and Infrastructure Adapters
<!-- cpt:list:target-users -->

**Key Problems Solved**:
<!-- cpt:list:key-problems required="true" -->
- Automation capability is limited by release cycles — tenants cannot deploy custom logic without platform rebuilds
- No unified mechanism for durable, long-running, multi-step business process orchestration across modules and external systems
- Lack of tenant-isolated execution with governance controls, resource limits, and auditability
- Insufficient operational visibility and debugging tools for distributed workflow executions
<!-- cpt:list:key-problems -->

**Success Criteria**:
<!-- cpt:list:success-criteria required="true" -->
- Runtime service availability ≥ 99.95% monthly (Baseline: N/A; Target: production launch)
- New execution start latency p95 ≤ 100ms under normal load (Baseline: N/A; Target: v1.0)
- Step dispatch latency p95 ≤ 50ms under normal load (Baseline: N/A; Target: v1.0)
- Execution visibility query latency p95 ≤ 200ms under normal load (Baseline: N/A; Target: v1.0)
- Scheduled executions start within 1 second of scheduled time under normal conditions (Baseline: N/A; Target: v1.0)
- Execution completion success ≥ 99.9% (excluding business-logic failures) via retries/compensation (Baseline: N/A; Target: v1.0)
- Business continuity: RTO ≤ 30 seconds, RPO ≤ 1 minute for execution state (Baseline: N/A; Target: v1.0)
- Audit trails and tenant isolation support SOC 2-aligned controls (Baseline: N/A; Target: v1.0)
<!-- cpt:list:success-criteria -->

**Capabilities**:
<!-- cpt:list:capabilities required="true" -->
- Runtime creation, modification, registration, and execution of functions and workflows without platform rebuild
- Tenant- and user-scoped registries with isolation, ownership, and access control
- Long-running asynchronous and synchronous execution with durable state persistence
- Multiple trigger modes: schedule-based, API-triggered, and event-driven
- Governance controls via resource limits, quotas, and policies per tenant and per definition
- Rich operational tooling: execution visibility, debugging, audit trails, and observability
- Built-in support for saga/compensation patterns and idempotency mechanisms
- Infrastructure adapter integration with hot-plug registration
<!-- cpt:list:capabilities -->
<!-- cpt:##:overview -->

<!-- cpt:##:actors -->
## 2. Actors

**Human Actors**

<!-- cpt:###:actor-title repeat="many" -->
### Application Developer

<!-- cpt:id:actor -->
**ID**: `cpt-serverless-runtime-actor-app-developer`

<!-- cpt:paragraph:actor-role -->
**Role**: Provides custom functions and workflows for integrations at runtime. Uses the serverless runtime to isolate and consolidate orchestration logic, reducing direct dependencies between services by delegating multi-step business processes to durable workflow orchestrations. Creates platform-level and integration-specific workflow/function definitions. Uses debugging and profiling tools to troubleshoot failures and performance issues. Reviews audit records and operational metrics for compliance and incident response.
<!-- cpt:paragraph:actor-role -->
<!-- cpt:id:actor -->
<!-- cpt:###:actor-title repeat="many" -->

<!-- cpt:###:actor-title repeat="many" -->
### Tenant Administrator

<!-- cpt:id:actor -->
**ID**: `cpt-serverless-runtime-actor-tenant-admin`

<!-- cpt:paragraph:actor-role -->
**Role**: Manages automation assets within a tenant scope — creates, modifies, versions, and governs workflow/function definitions at the tenant level. Configures schedules, permissions, resource quotas, and retention policies. Monitors execution status and manages the lifecycle of running executions (cancel, retry, suspend, resume). Acts as the authority for tenant-level governance, sharing, and access control.
<!-- cpt:paragraph:actor-role -->
<!-- cpt:id:actor -->
<!-- cpt:###:actor-title repeat="many" -->

<!-- cpt:###:actor-title repeat="many" -->
### Tenant User

<!-- cpt:id:actor -->
**ID**: `cpt-serverless-runtime-actor-tenant-user`

<!-- cpt:paragraph:actor-role -->
**Role**: Creates and manages personal workflow/function automations within a user-scoped context. Has the same operational capabilities as a Tenant Administrator but limited to personal (user-scoped) definitions and executions. Can create, modify, execute, and monitor their own workflows/functions. User-scoped definitions are private by default and subject to tenant-level governance and resource quotas.
<!-- cpt:paragraph:actor-role -->
<!-- cpt:id:actor -->
<!-- cpt:###:actor-title repeat="many" -->

<!-- cpt:###:actor-title repeat="many" -->
### Platform Operator

<!-- cpt:id:actor -->
**ID**: `cpt-serverless-runtime-actor-platform-operator`

<!-- cpt:paragraph:actor-role -->
**Role**: Monitors and maintains the serverless runtime environment. Views execution state, history, and pending work across tenants. Uses debugging and profiling tools to troubleshoot failures and performance issues. Reviews audit records and operational metrics for compliance and incident response. Maintains and develops Infrastructure Adapters, including hot-plug registration of adapter-specific workflow definitions.
<!-- cpt:paragraph:actor-role -->
<!-- cpt:id:actor -->
<!-- cpt:###:actor-title repeat="many" -->

**System Actors**

<!-- cpt:###:actor-title repeat="many" -->
### Outbound API Gateway

<!-- cpt:id:actor -->
**ID**: `cpt-serverless-runtime-actor-outbound-api-gw`

<!-- cpt:paragraph:actor-role -->
**Role**: Manages all outbound API requests from CyberFabric to external services. Provides the runtime capability for workflows and functions to make outbound HTTP calls, enforcing policies such as rate limiting, authentication injection, circuit breaking, and audit logging for all external communication.
<!-- cpt:paragraph:actor-role -->
<!-- cpt:id:actor -->
<!-- cpt:###:actor-title repeat="many" -->

<!-- cpt:###:actor-title repeat="many" -->
### Types Registry

<!-- cpt:id:actor -->
**ID**: `cpt-serverless-runtime-actor-types-registry`

<!-- cpt:paragraph:actor-role -->
**Role**: Provides GTS (Global Type System) schema and instance registration, validation, and resolution. Used by the serverless runtime to validate workflow/function definition schemas, input/output type contracts, and to ensure type compatibility across workflow composition and versioning.
<!-- cpt:paragraph:actor-role -->
<!-- cpt:id:actor -->
<!-- cpt:###:actor-title repeat="many" -->

<!-- cpt:###:actor-title repeat="many" -->
### Authorization Engine

<!-- cpt:id:actor -->
**ID**: `cpt-serverless-runtime-actor-authz-engine`

<!-- cpt:paragraph:actor-role -->
**Role**: Provides authorization decisions for all workflow/function management and execution operations. The serverless runtime delegates authorization checks to this engine for definition management, execution start, execution query, execution cancellation, lifecycle operations, and sharing/visibility changes.
<!-- cpt:paragraph:actor-role -->
<!-- cpt:id:actor -->
<!-- cpt:###:actor-title repeat="many" -->

<!-- cpt:###:actor-title repeat="many" -->
### Audit Engine

<!-- cpt:id:actor -->
**ID**: `cpt-serverless-runtime-actor-audit-engine`

<!-- cpt:paragraph:actor-role -->
**Role**: Stores audit trail records for definition lifecycle events and execution lifecycle events, ensuring records identify tenant, actor, and correlation identifier.
<!-- cpt:paragraph:actor-role -->
<!-- cpt:id:actor -->
<!-- cpt:###:actor-title repeat="many" -->

<!-- cpt:###:actor-title repeat="many" -->
### Infrastructure Adapter

<!-- cpt:id:actor -->
**ID**: `cpt-serverless-runtime-actor-infra-adapter`

<!-- cpt:paragraph:actor-role -->
**Role**: A modular component that integrates external infrastructure (clouds, on-prem systems) with the serverless runtime. Provides adapter-specific workflow/function definitions and execution capabilities. May use per-tenant or shared infrastructure depending on compliance and performance requirements.
<!-- cpt:paragraph:actor-role -->
<!-- cpt:id:actor -->
<!-- cpt:###:actor-title repeat="many" -->

<!-- cpt:###:actor-title repeat="many" -->
### Event Broker

<!-- cpt:id:actor -->
**ID**: `cpt-serverless-runtime-actor-event-broker`

<!-- cpt:paragraph:actor-role -->
**Role**: Provides publish/subscribe event delivery infrastructure. The serverless runtime consumes events as triggers for event-driven workflow starts and publishes lifecycle events, audit events, and notification events for downstream consumers.
<!-- cpt:paragraph:actor-role -->
<!-- cpt:id:actor -->
<!-- cpt:###:actor-title repeat="many" -->

<!-- cpt:##:actors -->

<!-- cpt:##:frs -->
## 3. Functional Requirements

<!-- cpt:###:fr-title repeat="many" -->
### FR-001 Runtime Authoring and Definition Management

<!-- cpt:id:fr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p1` - **ID**: `cpt-serverless-runtime-fr-runtime-authoring`

<!-- cpt:free:fr-summary -->
The system MUST allow tenants and their users to create and modify functions and workflows at runtime, such that changes can be applied without rebuilding or redeploying the platform (BR-001).

The system MUST validate workflow/function definitions before registration and reject invalid definitions with actionable feedback including: the specific validation error type, the location in the definition (line number, field path, or step identifier), a human-readable error message, and suggested corrections where applicable (BR-011).

The system MUST support versioning of workflow/function definitions so that new executions can use an updated version, in-flight executions continue with the version they started with, and changes are traceable and can be rolled back where needed (BR-018).

Workflow/function definitions MUST be expressible in a form that allows automated tools (including LLMs) to reliably create, update, validate, and explain the workflow/function behavior in human-readable form (BR-031).

The system MUST support starting workflows/functions with typed input parameters and receiving typed outputs, such that inputs/outputs can be validated before execution and safely inspected in execution history subject to privacy controls (BR-032).
<!-- cpt:free:fr-summary -->

**Actors**:
<!-- cpt:id-ref:actor -->
`cpt-serverless-runtime-actor-app-developer`, `cpt-serverless-runtime-actor-tenant-admin`, `cpt-serverless-runtime-actor-tenant-user`
<!-- cpt:id-ref:actor -->
<!-- cpt:id:fr -->
<!-- cpt:###:fr-title repeat="many" -->

<!-- cpt:###:fr-title repeat="many" -->
### FR-002 Tenant-Isolated Registry and Access Control

<!-- cpt:id:fr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p1` - **ID**: `cpt-serverless-runtime-fr-tenant-registry`

<!-- cpt:free:fr-summary -->
The system MUST provide a registry of functions and workflows per tenant with the following characteristics (BR-002, BR-036):

- Default tenant isolation: workflows and functions MUST be isolated per tenant by default; a tenant MUST NOT see or access another tenant's workflows/functions without explicit sharing
- Ownership scoping: workflows and functions MUST support both tenant-level and user-level ownership; user-scoped definitions are private to the owning user by default, tenant-scoped definitions are visible to authorized users within the tenant
- Access control integration: the system MUST integrate with access control mechanisms to manage who can create, modify, execute, view, and share workflows/functions
- Querying available definitions visible to the requesting actor (based on ownership and sharing policies)
- Filtering by ownership scope (tenant-level or user-level), category, or tags
- Hot-plug registration and deregistration

The system MUST support enabling the workflow/function runtime for a tenant in a way that provisions required isolation and governance settings (including quotas) so the tenant can safely use the capability (BR-020).

**Note**: The specific access control model (RBAC, ReBAC, ABAC, or other) is out of scope for this PRD. The system MUST provide integration points for authorization checks at: definition management, execution start, execution query, execution cancellation, lifecycle operations, and sharing/visibility changes.
<!-- cpt:free:fr-summary -->

**Actors**:
<!-- cpt:id-ref:actor -->
`cpt-serverless-runtime-actor-app-developer`, `cpt-serverless-runtime-actor-tenant-admin`, `cpt-serverless-runtime-actor-tenant-user`, `cpt-serverless-runtime-actor-authz-engine`
<!-- cpt:id-ref:actor -->
<!-- cpt:id:fr -->
<!-- cpt:###:fr-title repeat="many" -->

<!-- cpt:###:fr-title repeat="many" -->
### FR-003 Execution Engine and Durability

<!-- cpt:id:fr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p1` - **ID**: `cpt-serverless-runtime-fr-execution-engine`

<!-- cpt:free:fr-summary -->
The system MUST support long-running asynchronous functions and workflows, including executions lasting days and longer where needed for business processes (BR-003).

The system MUST support synchronous request/response invocation as a first-class feature for short-running executions, where the caller receives the result (or error) in the same API response. This mode MUST be optional and MUST NOT replace long-running asynchronous execution as the primary workload (BR-004).

Workflows MUST provide a mechanism enabling suspend and resume behavior when waiting for events, survival across service restarts, and continuation without losing progress. The system MUST support suspension periods of at least 30 days. Suspended workflows exceeding the maximum suspension duration (configurable per tenant) MUST be handled according to tenant policy: auto-cancel with notification, indefinite suspension, or escalation (BR-009).

Workflow definitions MUST support conditional branching and loop constructs to model complex business logic (BR-010).
<!-- cpt:free:fr-summary -->

**Actors**:
<!-- cpt:id-ref:actor -->
`cpt-serverless-runtime-actor-app-developer`, `cpt-serverless-runtime-actor-tenant-admin`, `cpt-serverless-runtime-actor-tenant-user`
<!-- cpt:id-ref:actor -->
<!-- cpt:id:fr -->
<!-- cpt:###:fr-title repeat="many" -->

<!-- cpt:###:fr-title repeat="many" -->
### FR-004 Trigger and Schedule Management

<!-- cpt:id:fr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p1` - **ID**: `cpt-serverless-runtime-fr-trigger-schedule`

<!-- cpt:free:fr-summary -->
The system MUST support starting functions/workflows via three trigger modes: schedule-based triggers, API-triggered starts, and event-driven triggers (BR-007).

The system MUST support schedule lifecycle management (create, update, pause/resume, and delete) and MUST support a configurable policy for handling missed schedules during downtime. The system MUST support at minimum the following policies: skip (ignore missed execution), catch-up (execute once for all missed instances), and backfill (execute each missed instance individually). The default policy SHOULD be "skip" (BR-022).

The system SHOULD support defining schedule-level input parameters and overrides so that recurring executions can run with consistent defaults and can be adjusted without modifying the underlying definition (BR-110).
<!-- cpt:free:fr-summary -->

**Actors**:
<!-- cpt:id-ref:actor -->
`cpt-serverless-runtime-actor-app-developer`, `cpt-serverless-runtime-actor-tenant-admin`, `cpt-serverless-runtime-actor-tenant-user`, `cpt-serverless-runtime-actor-platform-operator`, `cpt-serverless-runtime-actor-event-broker`
<!-- cpt:id-ref:actor -->
<!-- cpt:id:fr -->
<!-- cpt:###:fr-title repeat="many" -->

<!-- cpt:###:fr-title repeat="many" -->
### FR-005 Runtime Capabilities and Adapter Integration

<!-- cpt:id:fr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p1` - **ID**: `cpt-serverless-runtime-fr-runtime-capabilities`

<!-- cpt:free:fr-summary -->
Workflows and functions MUST be able to invoke runtime-provided capabilities needed for business automation (BR-008):

- Making outbound HTTP requests
- Emitting or publishing business events
- Writing to audit logs
- Invoking platform-provided operations required for orchestration

The system MUST support runtime registration of workflow/function definitions from Infrastructure Adapters with the following characteristics (BR-035):

- Hot-plug registration without requiring platform restart
- Per-tenant workflow definition registration; adapters connected to one tenant MUST NOT affect other tenants
- Flexible infrastructure models: adapters MAY use either per-tenant infrastructure (dedicated resources) or shared infrastructure (multi-tenant resources) based on configuration
- Tenant isolation enforcement: regardless of infrastructure model, tenant isolation MUST be enforced at the execution and data level

When an integration adapter or external dependency is disconnected, the system MUST reject new workflow/function starts that depend on the disconnected component and allow in-flight executions to complete or fail gracefully (BR-136).
<!-- cpt:free:fr-summary -->

**Actors**:
<!-- cpt:id-ref:actor -->
`cpt-serverless-runtime-actor-app-developer`, `cpt-serverless-runtime-actor-platform-operator`, `cpt-serverless-runtime-actor-outbound-api-gw`, `cpt-serverless-runtime-actor-types-registry`, `cpt-serverless-runtime-actor-infra-adapter`
<!-- cpt:id-ref:actor -->
<!-- cpt:id:fr -->
<!-- cpt:###:fr-title repeat="many" -->

<!-- cpt:###:fr-title repeat="many" -->
### FR-006 Execution Lifecycle and Resilience

<!-- cpt:id:fr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p1` - **ID**: `cpt-serverless-runtime-fr-execution-lifecycle`

<!-- cpt:free:fr-summary -->
The system MUST support lifecycle management for workflows/functions and their executions, including the ability to: start executions, cancel or terminate executions, retry failed executions, suspend and resume executions, and apply compensation behavior on cancellation where applicable (BR-014).

The system MUST support configurable retry and failure-handling policies including: maximum retry attempts, backoff behavior, and classification of non-retryable failures (BR-019).

The system MUST provide dead letter handling for executions that repeatedly fail after all retry attempts, ensuring failed executions are preserved for analysis and manual recovery (BR-027).

The system MUST enforce a maximum execution duration guardrail to prevent infinite or runaway executions. This guardrail MUST be configurable per tenant and workflow/function and MUST apply even if higher timeouts are requested (BR-028).

The system MUST ensure that updating a workflow/function definition does not affect executions currently running with the previous version (BR-029).

The system MUST support error boundary mechanisms that contain failures within specific workflow sections and prevent cascading failures across the entire workflow (BR-030).

The system MUST support configurable execution timeouts at both the workflow/function level and individual step level. Configured timeouts MUST NOT exceed the maximum execution duration guardrail (BR-112).

The system MUST monitor and terminate executions that consume excessive resources relative to configured limits, including detection of CPU spinning, memory leaks, and excessive I/O. Terminated executions MUST be logged with detailed resource consumption metrics (BR-040).

The system SHOULD support controlled interaction with in-flight executions, including the ability for authorized actors to provide external signals/inputs for event-driven continuation and to perform manual intervention actions needed to resolve operational issues (BR-108).
<!-- cpt:free:fr-summary -->

**Actors**:
<!-- cpt:id-ref:actor -->
`cpt-serverless-runtime-actor-app-developer`, `cpt-serverless-runtime-actor-tenant-admin`, `cpt-serverless-runtime-actor-tenant-user`, `cpt-serverless-runtime-actor-platform-operator`
<!-- cpt:id-ref:actor -->
<!-- cpt:id:fr -->
<!-- cpt:###:fr-title repeat="many" -->

<!-- cpt:###:fr-title repeat="many" -->
### FR-007 Execution Visibility and Querying

<!-- cpt:id:fr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p1` - **ID**: `cpt-serverless-runtime-fr-execution-visibility`

<!-- cpt:free:fr-summary -->
The system MUST provide an interface for authorized users/operators to (BR-015):

- List available workflow/function definitions in their scope
- List executions and their current status
- Inspect execution history and the current/pending step
- Filter/search by tenant, initiator, time range, status, and correlation identifier

The system MUST provide lifecycle controls for individual executions including: querying execution status until completion, canceling an in-flight execution, and replaying an execution for controlled recovery and incident analysis (BR-128).
<!-- cpt:free:fr-summary -->

**Actors**:
<!-- cpt:id-ref:actor -->
`cpt-serverless-runtime-actor-app-developer`, `cpt-serverless-runtime-actor-tenant-admin`, `cpt-serverless-runtime-actor-tenant-user`, `cpt-serverless-runtime-actor-platform-operator`, `cpt-serverless-runtime-actor-audit-engine`
<!-- cpt:id-ref:actor -->
<!-- cpt:id:fr -->
<!-- cpt:###:fr-title repeat="many" -->

<!-- cpt:###:fr-title repeat="many" -->
### FR-008 Input Validation and Security Enforcement

<!-- cpt:id:fr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p1` - **ID**: `cpt-serverless-runtime-fr-input-security`

<!-- cpt:free:fr-summary -->
The system MUST validate all workflow/function inputs against defined schemas before execution begins, including type validation, range and format validation, and detection and rejection of excessive payload sizes. Invalid inputs MUST be rejected with clear error messages (BR-037).

The system MUST prevent injection attacks by ensuring workflow/function inputs cannot be used to execute unintended operations. SQL injection: inputs MUST be parameterized. Command injection: inputs MUST NOT be interpolated into shell commands. Path traversal: file path inputs MUST be validated and restricted to allowed directories. Error messages MUST NOT expose internal system details (BR-038).

The system MUST prevent privilege escalation through input manipulation: inputs that specify or influence execution identity MUST be validated against the caller's authorization scope; inputs requesting elevated permissions MUST be rejected unless explicitly authorized; definitions MUST NOT escalate their own privileges beyond registration-time grants. The system MUST fail if privilege validation cannot be performed (BR-039).
<!-- cpt:free:fr-summary -->

**Actors**:
<!-- cpt:id-ref:actor -->
`cpt-serverless-runtime-actor-app-developer`, `cpt-serverless-runtime-actor-tenant-admin`, `cpt-serverless-runtime-actor-tenant-user`, `cpt-serverless-runtime-actor-types-registry`
<!-- cpt:id-ref:actor -->
<!-- cpt:id:fr -->
<!-- cpt:###:fr-title repeat="many" -->

<!-- cpt:###:fr-title repeat="many" -->
### FR-009 Debugging and Operational Tooling

<!-- cpt:id:fr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p2` - **ID**: `cpt-serverless-runtime-fr-debugging`

<!-- cpt:free:fr-summary -->
The platform MUST provide a way to debug workflow executions, including setting breakpoints, logging each action/function call with input parameters and return values, and retaining sufficient execution history to troubleshoot failures (BR-101).

The platform SHOULD provide step-through capabilities for workflow execution to support troubleshooting and controlled execution (BR-102).

The system SHOULD support a dry-run mode for workflows/functions that validates execution readiness (definition validity, permissions, configured limits) using user-provided input. Dry-run MUST NOT create a durable execution record and MUST NOT cause external side effects (BR-103).

The system MUST provide a debug view for an execution that includes an ordered list of invoked calls with input parameters, execution duration per call, and the exact call response (result or error). The debug view MUST NOT expose secrets; sensitive inputs/outputs MUST be masked by default unless explicitly permitted (BR-130).

The system MUST expose a standardized error taxonomy for workflow/function execution failures, including upstream HTTP/integration failures, runtime/environment failures (timeouts, resource limits), and code execution and validation failures. Errors MUST include a stable error identifier, a human-readable message, and a structured details object (BR-129).
<!-- cpt:free:fr-summary -->

**Actors**:
<!-- cpt:id-ref:actor -->
`cpt-serverless-runtime-actor-app-developer`, `cpt-serverless-runtime-actor-platform-operator`
<!-- cpt:id-ref:actor -->
<!-- cpt:id:fr -->
<!-- cpt:###:fr-title repeat="many" -->

<!-- cpt:###:fr-title repeat="many" -->
### FR-010 Advanced Execution Patterns

<!-- cpt:id:fr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p2` - **ID**: `cpt-serverless-runtime-fr-advanced-patterns`

<!-- cpt:free:fr-summary -->
The system SHOULD support invoking child workflows/functions from a parent workflow for modular composition and reuse (BR-104).

The system SHOULD support parallel execution of independent steps/functions within a workflow, with controllable concurrency caps and configurable concurrency limits (BR-105).

The system MUST provide built-in support for saga-style orchestration, including compensation logic to reverse the effects of completed steps when a workflow cannot complete successfully (BR-133).

The platform MUST provide mechanisms to implement idempotency for workflow/function execution. The system SHOULD support common idempotency patterns including idempotency keys, deduplication windows, and correlation identifiers to track and deduplicate requests (BR-134).
<!-- cpt:free:fr-summary -->

**Actors**:
<!-- cpt:id-ref:actor -->
`cpt-serverless-runtime-actor-app-developer`
<!-- cpt:id-ref:actor -->
<!-- cpt:id:fr -->
<!-- cpt:###:fr-title repeat="many" -->

<!-- cpt:###:fr-title repeat="many" -->
### FR-011 Governance, Sharing, and Notifications

<!-- cpt:id:fr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p2` - **ID**: `cpt-serverless-runtime-fr-governance-sharing`

<!-- cpt:free:fr-summary -->
The system SHOULD support governance controls for workflow/function changes (such as review/approval and controlled activation) to reduce operational risk and support compliance (BR-122).

The system SHOULD support sharing of workflow/function definitions beyond the default ownership scope, enabling authorized actors to grant discovery and execution access to other users, groups, or tenants. Sharing capabilities SHOULD be extensible — concrete mechanisms (group-based sharing, cross-tenant federation, marketplace publishing) MAY be implemented as external modules or plugins. The system MUST NOT require a specific sharing implementation to be embedded in the core runtime (BR-123).

The system SHOULD support notifying authorized users/operators about important workflow/function events, including failures, repeated retries, and abnormal execution duration, to reduce time-to-detection and time-to-recovery (BR-109).
<!-- cpt:free:fr-summary -->

**Actors**:
<!-- cpt:id-ref:actor -->
`cpt-serverless-runtime-actor-tenant-admin`, `cpt-serverless-runtime-actor-platform-operator`, `cpt-serverless-runtime-actor-event-broker`
<!-- cpt:id-ref:actor -->
<!-- cpt:id:fr -->
<!-- cpt:###:fr-title repeat="many" -->

<!-- cpt:###:fr-title repeat="many" -->
### FR-012 Execution Replay and Visualization

<!-- cpt:id:fr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p2` - **ID**: `cpt-serverless-runtime-fr-replay-visualization`

<!-- cpt:free:fr-summary -->
The system SHOULD support replaying an execution from a recorded history or saved state, to support debugging, incident analysis, and controlled recovery (BR-124).

The system SHOULD make it easy for authorized users to visualize workflow structure (execution blocks and decisions/branches) and to understand which path is taken for a given execution (BR-125).
<!-- cpt:free:fr-summary -->

**Actors**:
<!-- cpt:id-ref:actor -->
`cpt-serverless-runtime-actor-app-developer`, `cpt-serverless-runtime-actor-tenant-admin`, `cpt-serverless-runtime-actor-tenant-user`, `cpt-serverless-runtime-actor-platform-operator`
<!-- cpt:id-ref:actor -->
<!-- cpt:id:fr -->
<!-- cpt:###:fr-title repeat="many" -->

<!-- cpt:###:fr-title repeat="many" -->
### FR-013 Advanced Deployment and Portability

<!-- cpt:id:fr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p3` - **ID**: `cpt-serverless-runtime-fr-advanced-deployment`

<!-- cpt:free:fr-summary -->
The system SHOULD support importing and exporting workflow/function definitions to enable backup, migration, and cross-environment management (BR-202).

The system SHOULD support execution time travel from historical states for debugging and compliance investigation purposes (BR-203).

The system SHOULD support A/B testing of workflow/function versions to validate changes before full deployment (BR-204).

The system SHOULD support canary release patterns for gradual rollout of workflow/function updates (BR-205).

The system SHOULD support long-term archival of execution history and audit records for tenants with extended compliance and reporting requirements (BR-201).
<!-- cpt:free:fr-summary -->

**Actors**:
<!-- cpt:id-ref:actor -->
`cpt-serverless-runtime-actor-tenant-admin`, `cpt-serverless-runtime-actor-platform-operator`
<!-- cpt:id-ref:actor -->
<!-- cpt:id:fr -->
<!-- cpt:###:fr-title repeat="many" -->

<!-- cpt:###:fr-title repeat="many" -->
### FR-014 Deployment Safety

<!-- cpt:id:fr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p2` - **ID**: `cpt-serverless-runtime-fr-deployment-safety`

<!-- cpt:free:fr-summary -->
The system SHOULD support blue-green deployment strategies for workflow/function updates to minimize risk during changes (BR-121).
<!-- cpt:free:fr-summary -->

**Actors**:
<!-- cpt:id-ref:actor -->
`cpt-serverless-runtime-actor-tenant-admin`, `cpt-serverless-runtime-actor-platform-operator`
<!-- cpt:id-ref:actor -->
<!-- cpt:id:fr -->
<!-- cpt:###:fr-title repeat="many" -->

<!-- cpt:##:frs -->

<!-- cpt:##:usecases -->
## 4. Use Cases

<!-- cpt:###:uc-title repeat="many" -->
### UC-001 Resource Provisioning with Rollback

<!-- cpt:id:usecase -->
**ID**: `cpt-serverless-runtime-usecase-resource-provisioning`

**Actors**:
<!-- cpt:id-ref:actor -->
`cpt-serverless-runtime-actor-app-developer`
<!-- cpt:id-ref:actor -->

<!-- cpt:paragraph:preconditions -->
**Preconditions**: Platform service team has registered a multi-step provisioning workflow definition. Target tenant has the serverless runtime enabled with appropriate resource quotas.
<!-- cpt:paragraph:preconditions -->

<!-- cpt:paragraph:flow -->
**Flow**: Multi-Step Resource Provisioning
<!-- cpt:paragraph:flow -->

<!-- cpt:numbered-list:flow-steps -->
1. Application developer triggers the provisioning workflow via API with typed input parameters (resource type, configuration, tenant context)
2. System validates input against the definition schema and starts an asynchronous execution under system context
3. Workflow provisions the first resource and records the completed step in durable state
4. Workflow provisions subsequent resources sequentially, persisting state after each step
5. If any step fails, the system invokes compensation logic to roll back previously completed steps (saga pattern)
6. On success, system records completion with output results and emits a lifecycle event
<!-- cpt:numbered-list:flow-steps -->

<!-- cpt:paragraph:postconditions -->
**Postconditions**: All resources are provisioned and confirmed, or all changes are rolled back with compensation steps logged. Execution history is queryable with correlation identifier.
<!-- cpt:paragraph:postconditions -->

**Alternative Flows**:
<!-- cpt:list:alternative-flows -->
- **Transient failure**: If a step fails with a retryable error, the system retries according to the configured retry policy (backoff, max attempts) before escalating
- **Service restart during execution**: The workflow resumes from the last completed step without duplicating side effects
- **Compensation failure**: If a compensation step also fails, the execution is moved to the dead letter queue for manual recovery
<!-- cpt:list:alternative-flows -->
<!-- cpt:id:usecase -->
<!-- cpt:###:uc-title repeat="many" -->

<!-- cpt:###:uc-title repeat="many" -->
### UC-002 Tenant Onboarding with External Approvals

<!-- cpt:id:usecase -->
**ID**: `cpt-serverless-runtime-usecase-tenant-onboarding`

**Actors**:
<!-- cpt:id-ref:actor -->
`cpt-serverless-runtime-actor-app-developer`, `cpt-serverless-runtime-actor-tenant-admin`
<!-- cpt:id-ref:actor -->

<!-- cpt:paragraph:preconditions -->
**Preconditions**: Tenant onboarding workflow is registered. The workflow definition includes steps that wait for external approval events.
<!-- cpt:paragraph:preconditions -->

<!-- cpt:paragraph:flow -->
**Flow**: Staged Onboarding with Event-Driven Continuation
<!-- cpt:paragraph:flow -->

<!-- cpt:numbered-list:flow-steps -->
1. Tenant administrator initiates the onboarding workflow via API trigger
2. System creates the execution under tenant-admin context, validates inputs, and begins the first stage (account setup)
3. Workflow completes initial setup steps and suspends, waiting for an external approval event
4. The workflow remains suspended (up to 30+ days) with state persisted and queryable
5. External approval event arrives; system resumes the workflow from the suspension point
6. Workflow completes remaining onboarding stages (integration setup, configuration, verification)
7. System records completion and emits lifecycle events for audit
<!-- cpt:numbered-list:flow-steps -->

<!-- cpt:paragraph:postconditions -->
**Postconditions**: Tenant is fully onboarded with all stages completed. Execution history shows each stage, including suspension/resume events. Security context is preserved throughout the multi-day execution.
<!-- cpt:paragraph:postconditions -->

**Alternative Flows**:
<!-- cpt:list:alternative-flows -->
- **Approval timeout**: If the external approval does not arrive within the configured suspension duration, the tenant policy determines the action (auto-cancel with notification, indefinite suspension, or escalation)
- **Credential expiration**: System automatically refreshes the initiator's authentication tokens during the long-running execution to prevent failure due to token expiration
<!-- cpt:list:alternative-flows -->
<!-- cpt:id:usecase -->
<!-- cpt:###:uc-title repeat="many" -->

<!-- cpt:###:uc-title repeat="many" -->
### UC-003 Subscription Lifecycle Management

<!-- cpt:id:usecase -->
**ID**: `cpt-serverless-runtime-usecase-subscription-lifecycle`

**Actors**:
<!-- cpt:id-ref:actor -->
`cpt-serverless-runtime-actor-tenant-admin`
<!-- cpt:id-ref:actor -->

<!-- cpt:paragraph:preconditions -->
**Preconditions**: Subscription lifecycle workflows (activation, renewal, suspension, cancellation) are registered and versioned. Tenant has appropriate resource quotas.
<!-- cpt:paragraph:preconditions -->

<!-- cpt:paragraph:flow -->
**Flow**: Subscription Renewal with Schedule
<!-- cpt:paragraph:flow -->

<!-- cpt:numbered-list:flow-steps -->
1. Tenant administrator creates a schedule-based trigger for the subscription renewal workflow with recurring parameters
2. System validates the schedule configuration and registers it
3. At the scheduled time, system starts a new execution with the schedule-level input parameters
4. Workflow checks subscription status, processes renewal logic, and communicates with billing and notification services
5. System records the execution outcome and captures audit trail entries
<!-- cpt:numbered-list:flow-steps -->

<!-- cpt:paragraph:postconditions -->
**Postconditions**: Subscription is renewed or appropriately handled. Schedule continues for next cycle. Execution metrics are available for metering and cost allocation.
<!-- cpt:paragraph:postconditions -->

**Alternative Flows**:
<!-- cpt:list:alternative-flows -->
- **Missed schedule during downtime**: System applies the configured missed-schedule policy (skip, catch-up, or backfill)
- **Schedule paused**: Administrator pauses the schedule; no new executions are started until resumed
- **Definition updated**: New schedule executions use the updated definition version; in-flight executions continue with the version they started with
<!-- cpt:list:alternative-flows -->
<!-- cpt:id:usecase -->
<!-- cpt:###:uc-title repeat="many" -->

<!-- cpt:###:uc-title repeat="many" -->
### UC-004 Policy Enforcement and Remediation

<!-- cpt:id:usecase -->
**ID**: `cpt-serverless-runtime-usecase-policy-enforcement`

**Actors**:
<!-- cpt:id-ref:actor -->
`cpt-serverless-runtime-actor-app-developer`, `cpt-serverless-runtime-actor-platform-operator`, `cpt-serverless-runtime-actor-event-broker`
<!-- cpt:id-ref:actor -->

<!-- cpt:paragraph:preconditions -->
**Preconditions**: Drift-detection and remediation workflow is registered. Event-driven trigger is configured to fire on policy drift events.
<!-- cpt:paragraph:preconditions -->

<!-- cpt:paragraph:flow -->
**Flow**: Detect Drift and Execute Corrective Actions
<!-- cpt:paragraph:flow -->

<!-- cpt:numbered-list:flow-steps -->
1. Event infrastructure publishes a policy-drift event
2. System receives the event trigger and starts the remediation workflow under system context
3. Workflow assesses drift severity using conditional branching logic
4. Workflow executes corrective actions (may invoke child workflows for complex remediation)
5. If corrective actions fail, compensation logic rolls back partial changes
6. System logs all actions to the audit trail with tenant and correlation identifiers
<!-- cpt:numbered-list:flow-steps -->

<!-- cpt:paragraph:postconditions -->
**Postconditions**: Drift is remediated or the execution is preserved in the dead letter queue for manual review. Audit records identify the actor, tenant, and correlation chain.
<!-- cpt:paragraph:postconditions -->

**Alternative Flows**:
<!-- cpt:list:alternative-flows -->
- **Resource limit exceeded**: If the workflow exceeds CPU or memory limits, the system terminates it with detailed resource consumption metrics logged
- **Parallel remediation**: If multiple independent remediations are needed, the workflow uses parallel execution with configurable concurrency caps
<!-- cpt:list:alternative-flows -->
<!-- cpt:id:usecase -->
<!-- cpt:###:uc-title repeat="many" -->

<!-- cpt:###:uc-title repeat="many" -->
### UC-005 Adapter Hot-Plug Registration

<!-- cpt:id:usecase -->
**ID**: `cpt-serverless-runtime-usecase-adapter-hotplug`

**Actors**:
<!-- cpt:id-ref:actor -->
`cpt-serverless-runtime-actor-platform-operator`, `cpt-serverless-runtime-actor-infra-adapter`
<!-- cpt:id-ref:actor -->

<!-- cpt:paragraph:preconditions -->
**Preconditions**: Platform operator has developed an Infrastructure Adapter and connected it to the platform. Target tenant has serverless runtime enabled.
<!-- cpt:paragraph:preconditions -->

<!-- cpt:paragraph:flow -->
**Flow**: Register Adapter Workflows at Runtime
<!-- cpt:paragraph:flow -->

<!-- cpt:numbered-list:flow-steps -->
1. Platform operator connects the Infrastructure Adapter to the platform for a specific tenant
2. Adapter registers its workflow/function definitions via the hot-plug registration API
3. System validates definitions (schema, input/output types, resource requirements)
4. System registers definitions in the tenant's registry, enforcing tenant isolation
5. Definitions become immediately available for invocation without platform restart
6. System records registration in the audit trail
<!-- cpt:numbered-list:flow-steps -->

<!-- cpt:paragraph:postconditions -->
**Postconditions**: Adapter-provided workflows/functions are available in the tenant's registry. Existing in-flight executions are unaffected. Definitions are scoped to the connected tenant only.
<!-- cpt:paragraph:postconditions -->

**Alternative Flows**:
<!-- cpt:list:alternative-flows -->
- **Validation failure**: If definitions are invalid, the system rejects them with actionable feedback and does not register any definitions from the batch
- **Adapter disconnection**: If the adapter is disconnected, the system rejects new starts for dependent workflows but allows in-flight executions to complete gracefully
- **Definition update**: Adapter registers an updated version; new executions use the new version, in-flight executions continue with the started version
<!-- cpt:list:alternative-flows -->
<!-- cpt:id:usecase -->
<!-- cpt:###:uc-title repeat="many" -->

<!-- cpt:###:uc-title repeat="many" -->
### UC-006 Debugging a Failed Workflow Execution

<!-- cpt:id:usecase -->
**ID**: `cpt-serverless-runtime-usecase-debug-execution`

**Actors**:
<!-- cpt:id-ref:actor -->
`cpt-serverless-runtime-actor-app-developer`, `cpt-serverless-runtime-actor-platform-operator`
<!-- cpt:id-ref:actor -->

<!-- cpt:paragraph:preconditions -->
**Preconditions**: A workflow execution has failed and is preserved in the dead letter queue or execution history. Operator has appropriate debugging permissions.
<!-- cpt:paragraph:preconditions -->

<!-- cpt:paragraph:flow -->
**Flow**: Investigate and Recover Failed Execution
<!-- cpt:paragraph:flow -->

<!-- cpt:numbered-list:flow-steps -->
1. Operator queries execution history filtered by status=failed, tenant, and time range
2. Operator inspects the failed execution's debug call trace (ordered calls with inputs, durations, and responses)
3. Operator identifies the failing step and reviews the standardized error taxonomy classification
4. Operator uses step-through or replay capabilities to reproduce the failure in a controlled manner
5. After identifying the root cause, operator triggers a retry of the failed execution or initiates a manual recovery action
<!-- cpt:numbered-list:flow-steps -->

<!-- cpt:paragraph:postconditions -->
**Postconditions**: Root cause is identified. Execution is either retried successfully or escalated with full debug context. All debugging operations are logged in the audit trail.
<!-- cpt:paragraph:postconditions -->

**Alternative Flows**:
<!-- cpt:list:alternative-flows -->
- **Sensitive data in trace**: Secrets and sensitive inputs/outputs are masked by default; operator must have explicit permission to view unmasked values
- **Cross-tenant debugging**: All cross-tenant debugging operations are logged with tenant_id and correlation_id for traceability
<!-- cpt:list:alternative-flows -->
<!-- cpt:id:usecase -->
<!-- cpt:###:uc-title repeat="many" -->

<!-- cpt:###:uc-title repeat="many" -->
### UC-007 Data Migration with Checkpoint/Resume

<!-- cpt:id:usecase -->
**ID**: `cpt-serverless-runtime-usecase-data-migration`

**Actors**:
<!-- cpt:id-ref:actor -->
`cpt-serverless-runtime-actor-app-developer`, `cpt-serverless-runtime-actor-tenant-admin`
<!-- cpt:id-ref:actor -->

<!-- cpt:paragraph:preconditions -->
**Preconditions**: Data migration workflow is registered with appropriate resource quotas for long-running execution. Source and target data stores are accessible.
<!-- cpt:paragraph:preconditions -->

<!-- cpt:paragraph:flow -->
**Flow**: Long-Running Data Migration with Durable State
<!-- cpt:paragraph:flow -->

<!-- cpt:numbered-list:flow-steps -->
1. Tenant administrator triggers the data migration workflow with typed input parameters (source, target, batch size, filters)
2. System validates inputs and starts an asynchronous execution
3. Workflow processes data in batches, persisting checkpoint state after each batch
4. If the service restarts mid-migration, the workflow resumes from the last checkpoint without reprocessing completed batches
5. Workflow tracks progress metrics (records processed, errors, duration) available for real-time querying
6. On completion, system records final results and emits lifecycle events
<!-- cpt:numbered-list:flow-steps -->

<!-- cpt:paragraph:postconditions -->
**Postconditions**: All data is migrated or the execution is available for retry/resume from the last checkpoint. Execution metrics support cost allocation. Audit trail records the complete migration lifecycle.
<!-- cpt:paragraph:postconditions -->

**Alternative Flows**:
<!-- cpt:list:alternative-flows -->
- **Permanent failure in a batch**: If a batch fails with a non-retryable error after retry exhaustion, the workflow marks the batch as failed and either continues with remaining batches or halts based on error boundary configuration
- **Max duration guardrail**: If the migration approaches the maximum execution duration, the system checkpoints and allows controlled restart rather than abrupt termination
<!-- cpt:list:alternative-flows -->
<!-- cpt:id:usecase -->
<!-- cpt:###:uc-title repeat="many" -->

<!-- cpt:##:usecases -->

<!-- cpt:##:nfrs -->
## 5. Non-functional requirements

<!-- cpt:###:nfr-title repeat="many" -->
### Security and Access Control

<!-- cpt:id:nfr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p1` - **ID**: `cpt-serverless-runtime-nfr-security`

<!-- cpt:list:nfr-statements -->
- Functions/workflows MUST support being executed under a system account, an API client context, or a user context (BR-006)
- For long-running workflows, the system MUST support automatic refresh of initiator/caller authentication tokens or credentials ensuring workflows do not fail due to token expiration and security context remains valid and auditable (BR-013)
- The system MUST enforce authenticated and authorized access to all workflow/function management and execution operations and MUST fail closed on authorization failures. The system SHOULD support separation of duties (author vs execute vs administer) (BR-016)
- The system MUST protect workflow/function definitions, execution state, and audit records with data protection controls including: protection at rest and in transit, minimization of sensitive data exposure in logs and execution history, and controls for handling sensitive inputs/outputs (BR-017)
- The system MUST ensure audit records are protected from unauthorized modification and deletion and are available for compliance review within the configured retention period (BR-023)
- The system MUST ensure the execution security context is available throughout the lifetime of an execution and to every workflow/function step so that all actions are attributable, authorized, and auditable (BR-024)
- The system MUST support secure handling of secrets ensuring: no exposure via logs/history/debugging, access restricted to authorized actors, integration with platform secret management, and secrets NEVER persisted in plaintext (BR-025)
- The system MUST ensure definitions, execution state, and execution history are encrypted at rest and all network communication encrypted in transit (BR-033)
- The system MUST maintain a complete audit trail for definition lifecycle events and execution lifecycle events. Audit records MUST identify tenant, actor, and correlation identifier (BR-034)
- The system MUST enforce access control for debugging capabilities respecting tenant isolation, with audit trail for cross-tenant operations, and sensitive data masked by default (BR-127)
<!-- cpt:list:nfr-statements -->
<!-- cpt:id:nfr -->
<!-- cpt:###:nfr-title repeat="many" -->

<!-- cpt:###:nfr-title repeat="many" -->
### Resource Governance

<!-- cpt:id:nfr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p1` - **ID**: `cpt-serverless-runtime-nfr-resource-governance`

<!-- cpt:list:nfr-statements -->
- The system MUST support defining and enforcing resource limits for execution including CPU limits and memory limits. The implementation MAY use runtime-controlled resource isolation or OS-level resource isolation based on architectural decisions (BR-005)
- The system MUST support defining resource limits at the individual definition level including: maximum concurrent executions, maximum memory allocation per execution, and maximum CPU allocation per execution (BR-012)
- The system MUST support per-tenant resource quotas including: maximum total concurrent executions per tenant and maximum execution history retention size per tenant (BR-106)
- The system MUST support configurable limits on execution frequency and total execution volume over time to prevent abuse and ensure fair resource allocation across tenants (BR-116)
- The system SHOULD support throttling of execution starts to protect downstream systems and prevent resource exhaustion under high load (BR-113)
<!-- cpt:list:nfr-statements -->
<!-- cpt:id:nfr -->
<!-- cpt:###:nfr-title repeat="many" -->

<!-- cpt:###:nfr-title repeat="many" -->
### Reliability and State Consistency

<!-- cpt:id:nfr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p1` - **ID**: `cpt-serverless-runtime-nfr-reliability`

<!-- cpt:list:nfr-statements -->
- The system MUST ensure workflow/function state remains consistent during concurrent operations and system failures with no partial updates or corrupted states (BR-026)
- Runtime service availability MUST meet or exceed 99.95% monthly
- Excluding business-logic failures, the platform MUST achieve ≥ 99.9% completion success via retries/compensation
- Recovery objectives MUST target RTO ≤ 30 seconds and RPO ≤ 1 minute for execution state
- Under normal load and with no external dependencies, scheduled executions MUST start within 1 second of their scheduled time
<!-- cpt:list:nfr-statements -->
<!-- cpt:id:nfr -->
<!-- cpt:###:nfr-title repeat="many" -->

<!-- cpt:###:nfr-title repeat="many" -->
### Operational Traceability

<!-- cpt:id:nfr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p1` - **ID**: `cpt-serverless-runtime-nfr-ops-traceability`

<!-- cpt:list:nfr-statements -->
- The system MUST ensure tenant identifiers and correlation identifiers are consistently present across audit records, logs, and operational metrics (BR-021)
- The system MUST provide operational metrics including volume, latency, error rates, and queue/backlog indicators, with segmentation by tenant (BR-135)
<!-- cpt:list:nfr-statements -->
<!-- cpt:id:nfr -->
<!-- cpt:###:nfr-title repeat="many" -->

<!-- cpt:###:nfr-title repeat="many" -->
### Observability and Metrics

<!-- cpt:id:nfr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p2` - **ID**: `cpt-serverless-runtime-nfr-observability`

<!-- cpt:list:nfr-statements -->
- The system SHOULD provide distributed tracing capabilities that follow execution across multiple services and external system calls (BR-115)
- The system SHOULD provide execution-level compute and memory metrics including wall-clock duration, CPU time, memory usage and limits (BR-131)
- The system SHOULD provide pre-built monitoring dashboards for common operational metrics and health indicators (BR-119)
- The system SHOULD support performance profiling of executions to identify bottlenecks (BR-120)
<!-- cpt:list:nfr-statements -->
<!-- cpt:id:nfr -->
<!-- cpt:###:nfr-title repeat="many" -->

<!-- cpt:###:nfr-title repeat="many" -->
### Retention and Compliance

<!-- cpt:id:nfr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p2` - **ID**: `cpt-serverless-runtime-nfr-retention`

<!-- cpt:list:nfr-statements -->
- The system MUST support configurable retention policies for execution history and audit records including tenant-level defaults and deletion policies aligned to contractual and compliance needs (BR-107)
- The system MUST provide a default retention period for execution history; the default SHOULD be 7 days and MUST be configurable per tenant and/or per function type (BR-126)
- The system MUST provide metering of resource consumption per tenant and per workflow/function to support cost allocation and billing (BR-111)
- Audit trails and tenant isolation MUST support SOC 2-aligned controls
<!-- cpt:list:nfr-statements -->
<!-- cpt:id:nfr -->
<!-- cpt:###:nfr-title repeat="many" -->

<!-- cpt:###:nfr-title repeat="many" -->
### Performance

<!-- cpt:id:nfr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p2` - **ID**: `cpt-serverless-runtime-nfr-performance`

<!-- cpt:list:nfr-statements -->
- Under normal load, workflow start latency p95 MUST be ≤ 100ms from start request to first step scheduling (BR-207)
- Under normal load, step dispatch latency p95 MUST be ≤ 50ms from step scheduled to execution start (BR-207)
- Under normal load, monitoring query latency p95 MUST be ≤ 200ms for execution state/history queries (BR-207)
- Runtime overhead SHOULD be ≤ 10ms per step excluding business logic (BR-207)
- Execution timeouts per FR-006 (BR-112)
- The system SHOULD support customizing execution environment settings (time zones, locale, regional compliance) per tenant (BR-117)
- The system SHOULD support caching of execution results for idempotent operations with configurable TTL to improve performance and reduce redundant processing (BR-118, BR-132)
<!-- cpt:list:nfr-statements -->
<!-- cpt:id:nfr -->
<!-- cpt:###:nfr-title repeat="many" -->

<!-- cpt:###:nfr-title repeat="many" -->
### Composition Dependencies

<!-- cpt:id:nfr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p2` - **ID**: `cpt-serverless-runtime-nfr-composition-deps`

<!-- cpt:list:nfr-statements -->
- The system SHOULD support declaring and managing dependencies between workflows/functions to ensure proper deployment order and compatibility (BR-114)
<!-- cpt:list:nfr-statements -->
<!-- cpt:id:nfr -->
<!-- cpt:###:nfr-title repeat="many" -->

<!-- cpt:###:nfr-title repeat="many" -->
### Scalability

<!-- cpt:id:nfr has="priority,task" covered_by="DESIGN,DECOMPOSITION,SPEC" -->
- [ ] `p3` - **ID**: `cpt-serverless-runtime-nfr-scalability`

<!-- cpt:list:nfr-statements -->
- The system SHOULD support ≥ 10,000 concurrent executions per region under normal load (BR-208)
- The system SHOULD support sustained workflow starts ≥ 1,000/sec per region under normal load (BR-208)
- The system SHOULD support ≥ 100,000 workflow executions/day initially with a growth plan to ≥ 1,000,000/day (BR-208)
- The system SHOULD support ≥ 1,000 tenants with a clear partitioning/isolation strategy (BR-208)
- The system SHOULD support ≥ 10,000 registered workflow definitions across tenants including per-tenant hot-plug (BR-208)
- The system SHOULD provide stronger isolation boundaries for execution to ensure one tenant's code cannot affect another tenant's environment and resource consumption does not negatively impact other executions (noisy neighbor prevention) (BR-206)
<!-- cpt:list:nfr-statements -->
<!-- cpt:id:nfr -->
<!-- cpt:###:nfr-title repeat="many" -->

<!-- cpt:###:intentional-exclusions -->
### Intentional Exclusions

<!-- cpt:list:exclusions -->
- **Usability / UX** (UX-PRD-001, UX-PRD-002, UX-PRD-003, UX-PRD-004, UX-PRD-005): Not applicable — this is a platform runtime capability, not a user-facing application; consumers interact via APIs and programmatic interfaces, not graphical UIs
- **Visual Workflow Designer UI**: Explicitly out of scope as a future capability; the PRD covers the runtime engine and APIs only
- **External Marketplace**: Explicitly out of scope; sharing is supported via extensible mechanisms (BR-123) but marketplace is not in scope
- **Privacy by Design** (SEC-PRD-005): Not directly applicable at the PRD level — the serverless runtime processes tenant automation logic, not end-user PII; data protection controls (BR-017, BR-025) address sensitive data handling within executions
- **Safety** (SAFE-PRD-001, SAFE-PRD-002): Not applicable — this is a software automation runtime with no physical interaction, medical, or industrial control implications
<!-- cpt:list:exclusions -->
<!-- cpt:###:intentional-exclusions -->
<!-- cpt:##:nfrs -->

<!-- cpt:##:nongoals -->
## 6. Non-Goals & Risks

<!-- cpt:###:nongoals-title -->
### Non-Goals

<!-- cpt:list:nongoals -->
- Visual workflow designer UI (future capability — not part of this PRD)
- External workflow template marketplace
- Real-time event streaming infrastructure (assumed to exist as a separate platform capability)
- Optimizing for short-lived synchronous request/response patterns as the primary/dominant workload (synchronous invocation is supported as a secondary pattern per FR-003)
<!-- cpt:list:nongoals -->
<!-- cpt:###:nongoals-title -->

<!-- cpt:###:risks-title -->
### Risks

<!-- cpt:list:risks -->
- **Workflow logic complexity**: Authoring and governance may be complex for tenants. Mitigation: definition validation with actionable feedback (FR-001), LLM-manageable definitions (BR-031), visualization (FR-012).
- **Hot-plug reliability**: Runtime updates must not destabilize ongoing operations. Mitigation: execution isolation during updates (BR-029), versioning (BR-018), blue-green deployment (BR-121).
- **Security context propagation**: Long-running state must preserve identity reliably. Mitigation: security context availability throughout execution (BR-024), credential refresh (BR-013).
- **Scheduling scale**: Large numbers of schedules may require careful scaling. Mitigation: per-tenant quotas (BR-106), throttling (BR-113), missed schedule policies (BR-022).
- **Noisy neighbor**: Multi-tenant runtime must enforce per-tenant limits to prevent impact. Mitigation: resource governance (BR-005, BR-012), stronger isolation boundaries (BR-206).
- **Sandbox escape / isolation boundary failure**: User-provided code could attempt to break isolation and access host resources or other tenant data. Mitigation: injection prevention (BR-038), privilege escalation prevention (BR-039), resource exhaustion protection (BR-040), stronger isolation (BR-206).
- **Secret exfiltration**: Workflows/functions could attempt to read or emit secrets via outputs, events, HTTP calls, or logs. Mitigation: secure secrets handling (BR-025), debug view masking (BR-130), data protection controls (BR-017).
- **Privilege escalation via execution identity**: Misconfiguration of system/user/API-client execution contexts could grant unintended permissions. Mitigation: privilege escalation prevention (BR-039), access control (BR-016).
<!-- cpt:list:risks -->
<!-- cpt:###:risks-title -->
<!-- cpt:##:nongoals -->

<!-- cpt:##:assumptions -->
## 7. Assumptions & Open Questions

<!-- cpt:###:assumptions-title -->
### Assumptions

<!-- cpt:list:assumptions -->
- Platform identity and authorization are available and can be used to determine user/system context. If identity service is unavailable, the serverless runtime cannot start new executions.
- Event infrastructure exists to deliver event triggers and record lifecycle events. If event infrastructure is degraded, event-triggered workflows will not start but API-triggered and scheduled workflows continue.
- Persistent storage exists to support durability of execution state. Storage unavailability is a critical dependency that blocks all execution.
- Scheduling is logically part of the Serverless Runtime but may be implemented as a cooperating internal service with its own persistence and scaling characteristics.
<!-- cpt:list:assumptions -->
<!-- cpt:###:assumptions-title -->

<!-- cpt:###:open-questions-title -->
### Open Questions

<!-- cpt:list:open-questions -->
- What is the maximum number of concurrent tenants for the initial deployment, and does this require partitioned infrastructure? — Owner: Platform Engineering, Target: DESIGN phase
- Should scheduled workflows support IANA timezone specifications or UTC-only for schedule definitions? — Owner: Product, Target: DESIGN phase
<!-- cpt:list:open-questions -->
<!-- cpt:###:open-questions-title -->
<!-- cpt:##:assumptions -->

<!-- cpt:##:context -->
## 8. Additional context

<!-- cpt:###:context-title repeat="many" -->
### Glossary

<!-- cpt:free:prd-context-notes -->
| Term | Definition |
|------|------------|
| **Workflow** | A durable, resumable process that orchestrates a sequence of steps; maintains state across failures and can run for extended periods (days to years). |
| **Workflow Definition** | A specification of workflow steps, inputs, outputs, error handling logic, and compensation behavior. |
| **Workflow Instance** | A single execution of a workflow definition with specific input parameters and state. |
| **Function** | A single unit of custom logic that can be invoked independently or as part of a workflow. |
| **Entrypoint** | Unified abstraction for functions and workflows; a registered definition that can be invoked via the runtime API. |
| **Entrypoint Definition** | Canonical definition for an entrypoint using a GTS-identified JSON Schema with pinned params/returns/errors and traits. |
| **Invocation Mode** | Execution mode for an entrypoint: `sync` (caller waits for result) or `async` (caller receives an invocation id and polls for status). |
| **Compensation** | A rollback action that reverses the effect of previously completed steps when a workflow fails (saga pattern). |
| **Scheduled Workflow** | A workflow that executes on a recurring schedule (periodic/cron-based). |
| **Infrastructure Adapter** | A modular component that integrates external infrastructure (clouds, on-prem systems) and can provide adapter-specific workflow definitions. |
| **Hot-Plug** | The ability to register new workflow/function definitions at runtime without system restart. |
| **Security Context** | Workflow-scoped state containing identity, tenant, and authorization context, preserved throughout workflow execution for communication with platform services. |
| **Execution** | A running or completed instance of a workflow or function, tracked with a unique invocation ID and correlation ID. |
| **Saga Pattern** | A distributed transaction pattern where a long-running workflow is broken into steps, each with a compensating action to undo its effects if the workflow fails. |
| **Tenant Isolation** | Strict separation ensuring one tenant's workflows, executions, and data cannot be accessed by another tenant. |
| **Durable Execution** | Execution that persists state and survives infrastructure failures, allowing workflows to resume from the last completed step. |
| **RTO** | Recovery Time Objective — the maximum acceptable time to restore service after a failure. |
| **RPO** | Recovery Point Objective — the maximum acceptable amount of data loss measured in time. |
| **TTL** | Time To Live — the duration for which a cached result remains valid before expiring. |
<!-- cpt:free:prd-context-notes -->
<!-- cpt:###:context-title repeat="many" -->

<!-- cpt:###:context-title repeat="many" -->
### Dependencies

<!-- cpt:free:prd-context-notes -->
| Dependency | Description | Criticality |
|------------|-------------|-------------|
| **Identity & Authorization** | Authentication and authorization of workflow operations | P0 |
| **Event Infrastructure** | Event bus for delivering event triggers and publishing audit/lifecycle events | P0 |
| **Observability Stack** | Metrics collection, logging, and distributed tracing infrastructure | P0 |
| **Secret Management** | Secret management service for secure storage and injection of sensitive values | P0 |
| **Infrastructure Adapter Contract** | Standard interface specification for adapter integration and hot-plug registration | P0 |
<!-- cpt:free:prd-context-notes -->
<!-- cpt:###:context-title repeat="many" -->

<!-- cpt:###:context-title repeat="many" -->
### Target Use Cases

<!-- cpt:free:prd-context-notes -->
The following business scenarios are the primary motivators for this capability. Each is elaborated as a formal use case in Section 4:

- **Resource provisioning** (UC-001): Multi-step provisioning with rollback on failure
- **Tenant onboarding** (UC-002): Staged setup, waiting on external approvals/events
- **Subscription lifecycle** (UC-003): Activation, renewal, suspension, cancellation flows
- **Billing cycles**: Metering aggregation and invoice preparation workflows
- **Policy enforcement/remediation** (UC-004): Detect drift and execute corrective actions
- **Data migration** (UC-007): Long-running copy/checkpoint/resume processes
- **Disaster recovery orchestration**: Controlled failover/failback sequences
<!-- cpt:free:prd-context-notes -->
<!-- cpt:###:context-title repeat="many" -->

<!-- cpt:###:context-title repeat="many" -->
### BR-to-Cypilot ID Cross-Reference

<!-- cpt:free:prd-context-notes -->
This PRD was reformatted from the original BR-xxx numbering scheme. The following table maps original BR IDs to Cypilot FR/NFR IDs for traceability:

| Original BR | Cypilot ID | Section |
|-------------|------------|---------|
| BR-001, BR-011, BR-018, BR-031, BR-032 | `cpt-serverless-runtime-fr-runtime-authoring` | FR-001 |
| BR-002, BR-020, BR-036 | `cpt-serverless-runtime-fr-tenant-registry` | FR-002 |
| BR-003, BR-004, BR-009, BR-010 | `cpt-serverless-runtime-fr-execution-engine` | FR-003 |
| BR-007, BR-022, BR-110 | `cpt-serverless-runtime-fr-trigger-schedule` | FR-004 |
| BR-008, BR-035, BR-136 | `cpt-serverless-runtime-fr-runtime-capabilities` | FR-005 |
| BR-014, BR-019, BR-027, BR-028, BR-029, BR-030, BR-040, BR-108, BR-112 | `cpt-serverless-runtime-fr-execution-lifecycle` | FR-006 |
| BR-015, BR-128 | `cpt-serverless-runtime-fr-execution-visibility` | FR-007 |
| BR-037, BR-038, BR-039 | `cpt-serverless-runtime-fr-input-security` | FR-008 |
| BR-101, BR-102, BR-103, BR-129, BR-130 | `cpt-serverless-runtime-fr-debugging` | FR-009 |
| BR-104, BR-105, BR-133, BR-134 | `cpt-serverless-runtime-fr-advanced-patterns` | FR-010 |
| BR-109, BR-122, BR-123 | `cpt-serverless-runtime-fr-governance-sharing` | FR-011 |
| BR-124, BR-125 | `cpt-serverless-runtime-fr-replay-visualization` | FR-012 |
| BR-201, BR-202, BR-203, BR-204, BR-205 | `cpt-serverless-runtime-fr-advanced-deployment` | FR-013 |
| BR-121 | `cpt-serverless-runtime-fr-deployment-safety` | FR-014 |
| BR-006, BR-013, BR-016, BR-017, BR-023, BR-024, BR-025, BR-033, BR-034, BR-127 | `cpt-serverless-runtime-nfr-security` | NFR Security |
| BR-005, BR-012, BR-106, BR-113, BR-116 | `cpt-serverless-runtime-nfr-resource-governance` | NFR Resource Governance |
| BR-026 | `cpt-serverless-runtime-nfr-reliability` | NFR Reliability |
| BR-021, BR-135 | `cpt-serverless-runtime-nfr-ops-traceability` | NFR Operational Traceability |
| BR-115, BR-119, BR-120, BR-131 | `cpt-serverless-runtime-nfr-observability` | NFR Observability and Metrics |
| BR-107, BR-111, BR-126 | `cpt-serverless-runtime-nfr-retention` | NFR Retention |
| BR-117, BR-118, BR-132, BR-207 | `cpt-serverless-runtime-nfr-performance` | NFR Performance |
| BR-114 | `cpt-serverless-runtime-nfr-composition-deps` | NFR Composition Dependencies |
| BR-206, BR-208 | `cpt-serverless-runtime-nfr-scalability` | NFR Scalability |
<!-- cpt:free:prd-context-notes -->
<!-- cpt:###:context-title repeat="many" -->

<!-- cpt:###:context-title repeat="many" -->
### Acceptance Criteria (Business-Level)

<!-- cpt:free:prd-context-notes -->
**Workflow Execution**:
- Workflows can be started with inputs and produce a completion outcome (success or failure) with a correlation identifier
- In-progress workflows resume after a service restart without duplicating completed step side effects
- Transient failures result in automatic retries per defined policy until success or exhaustion
- Permanent failures in multi-step workflows invoke compensation for previously completed steps
- Workflows can remain active for 30+ days with state preserved and queryable, and can continue on external signals/events

**Tenant Isolation & Security Context**:
- A tenant can only see and manage its own functions/workflows and executions
- Security context is preserved through long-running executions, ensuring actions are attributable to the correct tenant/user or system identity
- Unauthorized operations fail closed

**Hot-Plug / Runtime Updates**:
- New or updated functions/workflows become available without interrupting existing in-flight executions
- Updates do not retroactively change the behavior of already-running executions (safe evolution)

**Scheduling**:
- Tenants can create, update, pause/resume, and cancel schedules for recurring workflows
- Missed schedules during downtime follow a defined policy (e.g., skip or catch-up) and are recorded

**Observability & Operations**:
- Operators can view current execution state, history/timeline, and pending work
- Workflow lifecycle events are captured for audit and compliance
- Operational metrics exist for volume, latency, and error rates, and can be segmented by tenant
<!-- cpt:free:prd-context-notes -->
<!-- cpt:###:context-title repeat="many" -->

<!-- cpt:##:context -->
<!-- cpt:#:prd -->
