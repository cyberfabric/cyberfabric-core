# Architecture Decision Record

## Decision Summary
- **Problem**: ADR-202512101033-m29s establishes that RMS transforms Resource Group templates to an imperative interim format for Temporal workflow execution, but does not specify the concrete format of this interim template.
- **Decision**: Use CNCF Serverless Workflow Specification (JSON/YAML) as the interim template format for Resource Group provisioning workflows.
- **Rationale**: Serverless Workflow provides a standardized, vendor-neutral workflow DSL that aligns with Temporal's declarative workflow capabilities, supports complex control flow (conditions, loops, parallel execution), and has mature tooling and validation libraries.
- **Status**: Proposed
- **Date**: 2025-12-10

## Context

### Background
ADR-202512101033-m29s establishes that RMS transforms Resource Group templates into an imperative interim format that explicitly describes provisioning steps, dependencies, order, and rollback logic. This imperative format is passed to Temporal Workflow Engine for execution via core workflows that orchestrate specialized workers.

However, ADR-202512101033-m29s does not specify the concrete format/schema of this imperative interim template. This ADR addresses that gap by selecting Serverless Workflow Specification as the interim template format.

### Drivers / Constraints
- **Standardization**: Interim format should be standardized, vendor-neutral, and well-documented
- **Temporal Compatibility**: Format must be compatible with Temporal workflow execution (see ADR-202512091406-4pd2)
- **Expressiveness**: Format must support imperative control flow (sequences, conditions, loops, parallel execution, error handling, compensation/rollback)
- **Tooling**: Format should have mature validation, parsing, and transformation tooling
- **Multi-tenancy**: Format must support tenant context and isolation
- **Observability**: Format should support structured metadata for tracing and debugging
- **Versioning**: Format must support versioning and backward compatibility
- **Language Support**: Format should have SDKs/libraries in Rust (primary) and optionally Python, Go, TypeScript

### Assumptions
- Temporal Workflow Engine is operational (see ADR-202512091406-4pd2)
- RMS transformation logic can parse Resource Group templates and generate Serverless Workflow definitions
- Temporal can execute workflows defined in Serverless Workflow format (via SDK conversion or interpretation by a single worker)
- Serverless Workflow format supports the required control flow primitives (sequences, conditions, loops, parallel, error handling, compensation)

### Out of Scope
- Specific Resource Group template format/schema (JSON/Bicep/etc.)
- Implementation details of RMS transformation logic
- Implementation details of Temporal workflow execution
- Infrastructure Adapter mechanism details
- Policy evaluation engine details

### References
- ADR-202512101033-m29s: Use imperative interim template format to provision Resource Groups via Workflow Engine
- ADR-202512091406-4pd2: Workflow Engine foundation
- CNCF Serverless Workflow Specification: https://github.com/serverlessworkflow/specification
- Serverless Workflow Rust SDK: https://github.com/serverlessworkflow/sdk-rust
- Zigflow (formerly Temporal DSL): https://github.com/mrsimonemms/zigflow

## Options Considered

### Option A — CNCF Serverless Workflow Specification (Chosen)
**Description**: Use CNCF Serverless Workflow Specification (JSON/YAML) as the interim template format. Serverless Workflow is a vendor-neutral, open-source specification that defines a high-level Domain Specific Language (DSL) for describing workflows in serverless environments. The specification supports JSON and YAML formats and provides rich control flow primitives including states (operation, switch, parallel, foreach, callback, sleep, event, inject), transitions, error handling, and compensation.

**Pros:**
- **Standardization**: CNCF specification ensures vendor-neutrality and interoperability
- **Rich Control Flow**: Supports sequences, conditions (switch states), loops (foreach states), parallel execution, error handling, compensation
- **Mature Tooling**: Rust SDK (https://github.com/serverlessworkflow/sdk-rust), Python SDK (`serverlessworkflow-sdk`), TypeScript SDK (`@serverlessworkflow/sdk`), VSCode extension for validation and editing
- **Rust-First Alignment**: Official Rust SDK available (v1.0.0-alpha6) with core models, builders, and IO services; aligns with VHP's Rust-first approach
- **Validation**: Specification includes JSON Schema for validation; SDKs provide validation APIs
- **Observability**: Supports metadata, annotations, and structured data for tracing
- **Versioning**: Specification versioning (currently v1.0+) with backward compatibility considerations
- **Documentation**: Well-documented specification with examples
- **Temporal Compatibility**: Can be interpreted dynamically by a single Temporal worker/workflow that executes actions based on the Serverless Workflow definition
- **Human-Readable**: JSON/YAML format is human-readable and debuggable

**Cons:**
- **Learning Curve**: Team must learn Serverless Workflow specification syntax and concepts
- **Temporal Integration**: Requires implementation of a single Temporal worker/workflow that interprets Serverless Workflow definitions dynamically (different from Zigflow's model of creating workers per template)
- **Rust SDK Maturity**: Rust SDK is in alpha phase (v1.0.0-alpha6); may have limitations or API changes
- **Specification Evolution**: Specification may evolve (currently v1.0+); need to track changes

### Option B — Custom JSON-Based Format
**Description**: Design a custom JSON-based format specifically for VHP's imperative interim template needs. The format would be tailored to RMS transformation requirements and Temporal workflow execution patterns.

**Pros:**
- **Tailored Design**: Format can be optimized specifically for VHP's use case
- **Full Control**: Complete control over format evolution and features
- **Rust-Native**: Can design format with Rust-first approach (JSON Schema + serde)
- **Simplified**: Can avoid unnecessary complexity from general-purpose workflow specifications

**Cons:**
- **No Standardization**: Custom format lacks standardization and vendor-neutrality
- **Tooling Overhead**: Must build and maintain validation, parsing, and transformation tooling
- **Documentation Burden**: Must maintain comprehensive documentation and examples
- **Learning Curve**: Team must learn custom format syntax
- **Maintenance**: Ongoing maintenance of format specification and tooling
- **Interoperability**: Limited interoperability with external tools and systems

### Option C — Python Scripts
**Description**: Generate Python scripts that define imperative provisioning steps. RMS transforms Resource Group templates to Python scripts that Temporal workers execute.

**Pros:**
- **Familiar Language**: Python is widely known and familiar to many developers
- **Rich Expressiveness**: Python provides full programming language expressiveness (loops, conditions, error handling, etc.)
- **Temporal Python SDK**: Temporal has mature Python SDK for workflow definitions
- **Flexibility**: Can implement any logic required for provisioning

**Cons:**
- **Security Risk**: Executing arbitrary Python scripts introduces security vulnerabilities (code injection, privilege escalation)
- **Not Declarative**: Python scripts are imperative code, not declarative templates; harder to validate, visualize, and reason about
- **No Standardization**: Python scripts are not standardized; each script is unique
- **Validation Complexity**: Difficult to validate Python scripts statically (requires execution or complex static analysis)
- **Multi-tenancy Risk**: Python scripts could violate tenant isolation if not carefully sandboxed
- **Audit Trail**: Harder to generate structured audit trails from Python code execution
- **Rust-First Mismatch**: VHP is Rust-first; Python scripts don't align with primary language

### Option D — Zigflow (formerly Temporal DSL)
**Description**: Use Zigflow (https://github.com/mrsimonemms/zigflow), a tool that converts declarative YAML workflows (based on CNCF Serverless Workflow specification) into production-ready Temporal workflows. RMS transforms Resource Group templates to Zigflow-compatible YAML format.

**Pros:**
- **Native Integration**: Direct compatibility with Temporal workflow execution (no conversion needed)
- **CNCF Standard**: Based on CNCF Serverless Workflow specification
- **Declarative**: YAML-based declarative workflow definitions

**Cons:**
- **Worker Creation Model**: Zigflow creates whole Temporal workers from DSL templates; designed for creating a worker once that processes workflows with different parameters many times
- **Use Case Mismatch**: In VHP's scenario, every Resource Group template produces a different imperative provisioning scenario. Spawning a new worker for each Resource Group template would be inefficient
- **Single Worker Requirement**: VHP needs a single worker/workflow that can interpret different interim format definitions dynamically, not create new workers per template

### Option E — AWS Step Functions ASL (Amazon States Language)
**Description**: Use AWS Step Functions ASL (Amazon States Language) JSON format as the interim template format. ASL is a mature, well-documented workflow definition language.

**Pros:**
- **Mature**: AWS Step Functions ASL is mature and battle-tested
- **Rich Features**: Supports states, transitions, error handling, retries, parallel execution
- **Documentation**: Well-documented with extensive examples

**Cons:**
- **AWS-Specific**: Format is AWS-specific; vendor lock-in
- **Not Vendor-Neutral**: Not a standard specification; tied to AWS ecosystem
- **Temporal Compatibility**: Requires conversion to Temporal workflow definitions
- **Licensing**: May have licensing restrictions for non-AWS use

### Comparison Matrix

| Criteria                | Serverless Workflow | Custom JSON | Python Scripts | Zigflow | AWS ASL |
|-------------------------|---------------------|-------------|----------------|---------|---------|
| Standardization         | ✅ CNCF Standard    | ❌ Custom   | ❌ Not standardized | ⚠️ Temporal-specific | ❌ AWS-specific |
| Vendor Neutrality       | ✅ Vendor-neutral   | ✅ Custom   | ✅ Language   | ❌ Temporal-only | ❌ AWS-only |
| Control Flow Support    | ✅ Rich (states, switch, foreach, parallel) | ✅ Custom | ✅ Full language | ✅ Temporal-native | ✅ Rich |
| Validation Tooling      | ✅ Python/TS SDKs, JSON Schema | ⚠️ Custom | ❌ Static analysis | ⚠️ Zigflow tooling | ⚠️ AWS tooling |
| Rust Support            | ✅ Official SDK available | ✅ Native | ❌ Python only | ⚠️ Go-based | ❌ No |
| Temporal Compatibility  | ⚠️ Requires conversion | ⚠️ Requires conversion | ✅ Python SDK | ✅ Native | ❌ Requires conversion |
| Single Worker Model     | ✅ Interpreted dynamically | ✅ Interpreted dynamically | ✅ Interpreted dynamically | ❌ Creates workers per template | ⚠️ N/A |
| Human-Readable          | ✅ JSON/YAML        | ✅ JSON     | ⚠️ Code       | ✅ YAML | ✅ JSON |
| Security                | ✅ Declarative, validated | ✅ Declarative | ❌ Code execution risk | ✅ Declarative | ✅ Declarative |
| Learning Curve          | ⚠️ Medium           | ⚠️ Medium   | ✅ Low (Python) | ⚠️ Medium | ⚠️ Medium |
| Maintenance Burden      | ✅ Specification maintained | ❌ Custom maintenance | ✅ Language maintained | ✅ Zigflow maintained | ✅ AWS maintained |
| Documentation          | ✅ Well-documented  | ⚠️ Custom docs | ✅ Python docs | ⚠️ Limited | ✅ Well-documented |
| Observability           | ✅ Metadata support | ✅ Custom   | ⚠️ Code-level | ✅ Temporal-native | ✅ CloudWatch |

## Decision Details

### Chosen Option
**Option A — CNCF Serverless Workflow Specification** is selected as the interim template format for Resource Group provisioning.

**Why now:**
- ADR-202512101033-m29s establishes the need for an imperative interim template format but does not specify the concrete format
- Serverless Workflow provides the best balance of standardization, expressiveness, tooling, and vendor-neutrality
- CNCF specification ensures long-term maintainability and interoperability
- Rich control flow primitives (states, switch, foreach, parallel, error handling, compensation) align with RMS transformation requirements
- Mature validation tooling (Python/TypeScript SDKs) enables early validation and transformation
- Temporal compatibility can be achieved via conversion layer or Temporal DSL alignment

### Invariants (MUST)
- **Format Validation**: RMS MUST validate Serverless Workflow definitions against JSON Schema before passing to Temporal
- **Versioning**: Serverless Workflow definitions MUST include `specVersion` field; support multiple specification versions
- **Tenant Context**: Serverless Workflow definitions MUST include tenant context in metadata/annotations
- **Idempotency**: Serverless Workflow definitions MUST support idempotent execution (same input → same result)
- **Error Handling**: Serverless Workflow definitions MUST include error handling and compensation logic for rollback
- **Deterministic Execution**: Serverless Workflow definitions MUST be deterministic (no random or time-based logic in workflow definition)
- **Audit Trail**: Serverless Workflow definitions MUST include metadata for audit trail and tracing
- **Interpretation**: RMS MUST pass Serverless Workflow definitions to a single Temporal worker/workflow that interprets and executes the workflow definition dynamically

### Interfaces / Protocols
- **Serverless Workflow Format**: JSON/YAML format per CNCF Serverless Workflow Specification v1.0+
- **Temporal Workflow Protocol**: Temporal workflow execution (see ADR-202512091406-4pd2)
- **Validation API**: JSON Schema validation for Serverless Workflow definitions
- **Workflow Execution API**: Single Temporal worker/workflow that interprets Serverless Workflow definitions and executes actions

### Impact Radius
- **Services**: RMS (transformation logic, Serverless Workflow generation), Temporal Workflow Engine (workflow execution)
- **APIs**: RMS API for Resource Group template submission (generates Serverless Workflow definitions)
- **Workflows**: Core provisioning workflows (defined in Serverless Workflow format, executed via Temporal)
- **Tooling**: Serverless Workflow validation libraries (Python SDK or Rust parser), conversion utilities
- **Events**: Provisioning lifecycle events (workflow started/completed/failed)
- **Observability**: Metrics, logs, traces for Serverless Workflow transformation and execution

## Consequences

### Positive
- **Standardization**: CNCF specification ensures vendor-neutrality and long-term maintainability
- **Rich Expressiveness**: Serverless Workflow supports complex control flow (sequences, conditions, loops, parallel, error handling, compensation)
- **Validation**: Mature tooling (Rust/Python/TypeScript SDKs) enables early validation and error detection
- **Rust-First Alignment**: Official Rust SDK (https://github.com/serverlessworkflow/sdk-rust) aligns with VHP's Rust-first approach
- **Human-Readable**: JSON/YAML format is human-readable and debuggable
- **Observability**: Metadata and annotations support structured tracing and debugging
- **Interoperability**: Standard format enables potential integration with other workflow systems

### Negative / Risks

**Risk 1: Temporal Integration Complexity**
- **Impact**: Medium — Serverless Workflow definitions may require conversion to Temporal workflow definitions
- **Likelihood**: Medium — Need to implement a single Temporal worker/workflow that interprets Serverless Workflow definitions
- **Mitigation**: 
  - Implement a single Temporal workflow that parses Serverless Workflow JSON/YAML definitions and executes actions dynamically
  - Use Temporal SDKs (Rust/Go/Python) to build workflow interpreter
  - Leverage Serverless Workflow JSON Schema for validation before execution
  - Consider using existing Serverless Workflow execution libraries if available

**Risk 2: Rust SDK Maturity**
- **Impact**: Low — Official Rust SDK exists but is in alpha phase (v1.0.0-alpha6)
- **Likelihood**: Low-Medium — Alpha SDK may have limitations or API changes
- **Mitigation**: 
  - Use official Rust SDK (https://github.com/serverlessworkflow/sdk-rust) for parsing and validation
  - Monitor SDK releases and upgrade as it stabilizes
  - Contribute feedback and improvements to Rust SDK project if needed
  - Fallback to JSON Schema validation using serde if SDK limitations encountered

**Risk 3: Specification Evolution**
- **Impact**: Low-Medium — Serverless Workflow specification may evolve; need to track changes
- **Likelihood**: Low-Medium — Specification is at v1.0+; may have breaking changes in future versions
- **Mitigation**: 
  - Pin to specific specification version (`specVersion` field)
  - Support multiple specification versions simultaneously
  - Monitor specification changes and plan migration path
  - Version Serverless Workflow definitions explicitly

**Risk 4: Learning Curve**
- **Impact**: Medium — Team must learn Serverless Workflow specification syntax and concepts
- **Likelihood**: Medium — New specification to learn
- **Mitigation**: 
  - Provide training and documentation
  - Start with simple workflows and gradually increase complexity
  - Use VSCode extension for validation and editing
  - Create internal examples and templates

### Neutral / Trade-offs
- **Standardization vs. Customization**: Serverless Workflow provides standardization but may have features we don't need; custom format would be tailored but lacks standardization
- **Vendor Neutrality vs. Native Integration**: Serverless Workflow is vendor-neutral and can be interpreted by a single Temporal worker; Zigflow creates workers per template which doesn't fit the use case
- **Expressiveness vs. Security**: Serverless Workflow is declarative and secure; Python scripts are more expressive but introduce security risks

## Security, Privacy & Compliance

### Threats Addressed
- **Code Injection**: Serverless Workflow is declarative JSON/YAML; no code execution risk (unlike Python scripts)
- **Template Tampering**: JSON Schema validation prevents malformed or malicious workflow definitions
- **Tenant Isolation**: Tenant context included in workflow metadata; Temporal enforces namespace isolation
- **Privilege Escalation**: Declarative format prevents arbitrary code execution; policy checks occur before workflow execution (see ADR-202512101033-m29s)

### Mitigations
- **Input Validation**: RMS validates Serverless Workflow definitions against JSON Schema before transformation
- **Externalized Authorization**: Policy checks occur in RMS before workflow execution (see ADR-202512101033-m29s)
- **Structured Audit**: Serverless Workflow metadata supports structured audit trails
- **Encryption**: Serverless Workflow definitions encrypted in transit and at rest
- **Sandboxing**: Temporal workers execute in sandboxed environments with least privilege

### Data Classification
- **Serverless Workflow Definitions**: Contain resource provisioning instructions; encrypted at rest and in transit
- **Workflow Metadata**: Contains tenant context and tracing information; PII minimized where possible

### SOC 2 Mapping
- **CC6.x (Access Control)**: 
  - Policy checks in RMS enforce access control before workflow execution
  - Tenant isolation prevents unauthorized cross-tenant access
- **CC7.x (Change/Operations)**:
  - Audit trail records all workflow definitions and executions
  - Versioning tracks changes to workflow format and definitions

## Impact on Interfaces, Events & Data

### APIs
- **RMS Template Submission API**: REST API for submitting Resource Group templates (generates Serverless Workflow definitions)
  - Create OpenAPI stub based on `architecture-workspace/templates/api/openapi.yaml`
- **Serverless Workflow Validation API**: REST API for validating Serverless Workflow definitions
  - Endpoints: `POST /api/v1/workflows/validate`

### Events
- **Workflow Lifecycle Events**: CloudEvents-compatible events emitted for workflow operations
  - Create event catalog + JSON Schemas based on `architecture-workspace/templates/events/`
  - Event types: `workflow.definition.created`, `workflow.definition.validated`, `workflow.execution.started`, `workflow.execution.completed`, `workflow.execution.failed`

### Compatibility
- **Backwards Compatibility**: Support multiple Serverless Workflow specification versions (`specVersion` field)
- **Versioning**: Semantic versioning for Serverless Workflow definitions and conversion logic
- **Migration/Backfill**: Old workflow definitions continue to work with conversion logic; new definitions use latest format
- **Deprecation Plan**: Deprecated specification versions marked; migration path documented

## Ops & NFR Impact

### SLIs/SLOs
- **Workflow Definition Validation Latency p95**: ≤ 50ms for Serverless Workflow JSON Schema validation
- **Workflow Interpretation Latency p95**: ≤ 100ms for Serverless Workflow definition parsing and workflow start
- **Workflow Definition Reliability**: ≥ 99.9% successful validation rate (excluding invalid definitions)

### Observability
- **Metrics**: 
  - Serverless Workflow definition generation rate, validation rate, error rate
  - Workflow interpretation rate, interpretation latency, interpretation error rate
  - Workflow execution rate, completion rate, failure rate
- **Logs**: 
  - Structured logs with correlation IDs for workflow definition generation and validation
  - Workflow interpretation logs (parsing Serverless Workflow definitions)
  - Workflow execution logs with step-by-step progress
- **Traces**: 
  - Distributed tracing across RMS transformation → Serverless Workflow generation → Temporal workflow interpretation → Action execution
  - Trace correlation between template submission and workflow completion

### Rollout / Backout
- **Format Versioning**: Support multiple Serverless Workflow specification versions simultaneously
  - Deploy new workflow interpreter logic alongside existing logic
  - Route new templates to new format; old templates continue with old format
  - Gradual migration: route percentage of templates to new format, monitor, then migrate remaining
  - Rollback: revert to previous format version if issues detected
- **Workflow Interpreter Deployment**: Blue/green deployment for Temporal workflow interpreter (single worker that interprets Serverless Workflow definitions)

### Capacity & Costs
- **Expected Throughput**: 1,000 workflow definitions/day initially, scaling to 10,000/day
- **Storage**: Serverless Workflow definitions stored in RMS (retention policy: 90 days)
- **Compute**: Validation and workflow interpretation logic (CPU-bound); estimated 10ms per workflow definition parsing
- **Network**: gRPC traffic between RMS and Temporal; estimated 100MB/day initially
