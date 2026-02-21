# ADR-0009: Trait Enforcement Architecture

**Date**: 2026-02-03

**Status**: Proposed

**ADR ID**: `fdd-settings-service-adr-trait-enforcement`

## Context and Problem Statement

Setting types are configured through traits (SettingOptions, EventConfig, SettingsOperations) that control inheritance, compliance, events, and access control. We need to determine where and how these traits are enforced in the architecture.

## Decision Drivers

* Traits must be enforced consistently across all operations
* Enforcement logic should be centralized to avoid duplication
* Performance impact should be minimal
* Trait validation must occur at type creation and runtime
* Must support CEL expressions for access control traits
* Error messages should clearly indicate trait violations

## Considered Options

* **Option 1**: Domain layer enforcement with TraitsManager component
* **Option 2**: Middleware-based enforcement at API layer
* **Option 3**: Database-level enforcement with triggers and constraints

## Decision Outcome

Chosen option: "Option 1 - Domain layer enforcement with TraitsManager component", because it centralizes trait logic in the domain layer, maintains separation of concerns, and provides flexibility for complex trait rules including CEL expression evaluation.

### Consequences

* Good, because domain layer is the natural place for business rules
* Good, because TraitsManager can be tested independently
* Good, because CEL expressions can be evaluated in domain context
* Bad, because all operations must go through domain layer
* Bad, because trait enforcement adds latency to all operations

## CEL Implementation Guidance for TraitsManager

### Performance Requirements

**Target Evaluation SLA**:

* **p95 latency**: <100ms for CEL expression evaluation
* **p99 latency**: <200ms for complex expressions
* **Timeout**: 500ms hard limit per expression evaluation
* **Memory limit**: 10MB per expression evaluation context

**Optimization Strategy**:

1. **Precompile Expressions**:
   * Parse and compile CEL expressions at setting type creation time
   * Store compiled AST in memory alongside setting type metadata
   * Reject invalid expressions during type creation (fail-fast)

2. **Memoize Parsed ASTs**:
   * Cache compiled expressions in-memory per instance
   * Key: `setting_type_id:expression_hash`
   * TTL: Indefinite (invalidate only on setting type update)
   * Max cache size: 10,000 compiled expressions per instance

3. **Per-Tenant Expression Caches**:
   * Cache evaluation results for identical context
   * Key: `setting_type_id:tenant_id:context_hash`
   * TTL: 60 seconds (balance freshness vs performance)
   * Max cache size: 50,000 results per instance
   * LRU eviction when limit reached

4. **Short-Circuiting**:
   * Evaluate `mutable_access_scope` conditions in order
   * Stop evaluation on first `false` result (AND semantics)
   * Skip expression evaluation if tenant context unchanged

### Validation at Setting Type Creation

**Syntax Validation**:

* Parse CEL expression using CEL parser
* Verify expression compiles without errors
* Check for balanced parentheses, valid operators, proper quoting
* Reject expressions with syntax errors immediately

**Operator Whitelist**:

```text
Allowed operators:
- Comparison: ==, !=, <, <=, >, >=
- Logical: &&, ||, !
- Membership: in
- String: contains, startsWith, endsWith, matches (regex)
- Arithmetic: +, -, *, / (for numeric comparisons only)

Disallowed operators:
- Macro expansion
- Custom function definitions
- External function calls
```

**Type Checking**:

* Validate variable references: `$tenant_kind`, `$subject_tenant_kind`, `$tenant_id`, `$user_id`, `$mfa_enabled`
* Ensure comparison types match (string vs string, bool vs bool)
* Verify function arguments match expected types
* Reject expressions with type mismatches

**Example Validation**:

```rust
// Valid expressions
"$tenant_kind == 'PARTNER'"
"$tenant_kind == $subject_tenant_kind"
"$mfa_enabled == true && $tenant_kind in ['ENTERPRISE', 'PARTNER']"

// Invalid expressions (rejected at creation)
"$tenant_kind = 'PARTNER'"  // Wrong operator (= instead of ==)
"$invalid_var == true"       // Unknown variable
"$tenant_kind == 123"        // Type mismatch (string vs number)
"eval('malicious code')"     // Disallowed function
```

**Validation Response**:

* **On syntax error**: Return HTTP 400 with detailed error message
* **On type error**: Return HTTP 400 with type mismatch details
* **On security violation**: Return HTTP 400 with security policy violation
* **Warning mode**: Log warnings for deprecated patterns but allow creation

### Runtime Error Handling

**Evaluation Failure Scenarios**:

1. **Expression Timeout** (>500ms):
   * Fallback: Deny access (fail-secure)
   * Audit log: `trait_evaluation_timeout`
   * Error code: `TRAIT_EVAL_TIMEOUT`
   * User message: "Access control evaluation timed out. Contact administrator."

2. **Evaluation Exception** (runtime error):
   * Fallback: Deny access (fail-secure)
   * Audit log: `trait_evaluation_error` with exception details
   * Error code: `TRAIT_EVAL_ERROR`
   * User message: "Access control evaluation failed. Contact administrator."

3. **Missing Context Variable**:
   * Fallback: Treat as `null` or deny access based on expression
   * Audit log: `trait_evaluation_missing_context`
   * Error code: `TRAIT_EVAL_MISSING_VAR`
   * User message: "Required context not available for access check."

4. **Memory Limit Exceeded**:
   * Fallback: Deny access (fail-secure)
   * Audit log: `trait_evaluation_oom`
   * Error code: `TRAIT_EVAL_OOM`
   * User message: "Access control evaluation exceeded memory limit."

**Fallback Policies**:

* **Default**: Deny access on any evaluation failure (fail-secure)
* **Audit**: Log all evaluation failures with full context
* **Metrics**: Track failure rate per setting type
* **Alerting**: Alert on >1% evaluation failure rate

**User-Facing Error Messages**:

```json
{
  "type": "https://hyperspot.dev/problems/settings/trait-evaluation-failed",
  "title": "Access Control Evaluation Failed",
  "status": 403,
  "detail": "The access control expression for this setting could not be evaluated",
  "error_code": "TRAIT_EVAL_ERROR",
  "setting_type": "data.retention",
  "tenant_id": "f3e557f0-8bc1-421e-9781-1f3456d21742",
  "evaluation_context": {
    "tenant_kind": "CUSTOMER",
    "subject_tenant_kind": "CUSTOMER"
  },
  "support_action": "Contact support with this error code and timestamp"
}
```

### Supported CEL Operators and Functions

**Available Variables**:

```text
$tenant_kind          : string  - Kind of tenant (ROOT, PARTNER, RESELLER, CUSTOMER, etc.)
$subject_tenant_kind  : string  - Kind of subject tenant (for hierarchy checks)
$tenant_id            : string  - UUID of tenant
$user_id              : string  - UUID of authenticated user
$mfa_enabled          : bool    - Whether tenant has MFA enabled
$provisioning_state   : string  - Tenant provisioning state
$is_barrier_tenant    : bool    - Whether tenant is a barrier tenant
```

**Supported Functions**:

```text
// String functions
contains(str, substr)     - Check if string contains substring
startsWith(str, prefix)   - Check if string starts with prefix
endsWith(str, suffix)     - Check if string ends with suffix
matches(str, regex)       - Check if string matches regex pattern
size(str)                 - Get string length

// Collection functions
in(value, list)          - Check if value is in list
size(list)               - Get list size

// Type functions
type(value)              - Get type of value (for debugging)
```

**Security Restrictions**:

* **No file system access**: Cannot read/write files
* **No network access**: Cannot make HTTP requests
* **No code execution**: Cannot execute arbitrary code
* **No reflection**: Cannot inspect or modify runtime state
* **Sandboxed evaluation**: Runs in isolated context
* **Resource limits**: CPU time, memory, stack depth limits enforced
* **No recursion**: Recursive expressions rejected at validation

### Debugging Guidance

**Logging Levels**:

1. **DEBUG**: Log all expression evaluations with context

   ```text
   [DEBUG] TraitsManager: Evaluating mutable_access_scope
     setting_type: data.retention
     expression: $tenant_kind == $subject_tenant_kind
     context: {tenant_kind: "CUSTOMER", subject_tenant_kind: "CUSTOMER"}
     result: true
     duration: 2ms
   ```

2. **INFO**: Log evaluation failures and slow evaluations (>50ms)

   ```text
   [INFO] TraitsManager: Slow expression evaluation
     setting_type: data.retention
     expression: $tenant_kind in ['ENTERPRISE', 'PARTNER'] && $mfa_enabled
     duration: 87ms
   ```

3. **WARN**: Log evaluation timeouts and errors

   ```text
   [WARN] TraitsManager: Expression evaluation timeout
     setting_type: data.retention
     expression: complex_expression_here
     timeout: 500ms
   ```

4. **ERROR**: Log critical failures affecting multiple evaluations

   ```text
   [ERROR] TraitsManager: Expression cache corruption detected
     affected_types: 15
     action: Rebuilding cache
   ```

**Example Trace Output**:

```text
[TRACE] TraitsManager::check_mutable_access_scope START
  setting_type_id: 550e8400-e29b-41d4-a716-446655440000
  tenant_id: f3e557f0-8bc1-421e-9781-1f3456d21742
[TRACE] TraitsManager: Loading compiled expression from cache
  cache_key: 550e8400:mutable_access_scope:abc123
  cache_hit: true
[TRACE] TraitsManager: Building evaluation context
  tenant_kind: CUSTOMER
  subject_tenant_kind: CUSTOMER
  mfa_enabled: true
[TRACE] TraitsManager: Evaluating expression
  expression: $tenant_kind == $subject_tenant_kind && $mfa_enabled
  result: true
  duration: 3ms
[TRACE] TraitsManager::check_mutable_access_scope END (allowed)
```

**Sample Expressions for Testing**:

```cel
// Basic equality checks
"$tenant_kind == 'PARTNER'"
"$tenant_kind == $subject_tenant_kind"

// Logical combinations
"$tenant_kind == 'ENTERPRISE' && $mfa_enabled"
"$tenant_kind in ['PARTNER', 'RESELLER'] || $mfa_enabled"

// String operations
"$tenant_kind.startsWith('ENTER')"
"$provisioning_state.contains('ACTIVE')"

// Complex conditions
"($tenant_kind == 'ENTERPRISE' || $tenant_kind == 'PARTNER') && $mfa_enabled && !$is_barrier_tenant"

// Hierarchy checks
"$tenant_kind == $subject_tenant_kind || $subject_tenant_kind == 'CUSTOMER'"
```

**Sandbox Testing**:

Create a test harness for expression validation:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expression_evaluation() {
        let manager = TraitsManager::new();
        let context = EvaluationContext {
            tenant_kind: "CUSTOMER".to_string(),
            subject_tenant_kind: "CUSTOMER".to_string(),
            mfa_enabled: true,
            // ...
        };
        
        // Test valid expression
        let expr = "$tenant_kind == $subject_tenant_kind";
        assert!(manager.evaluate(expr, &context).unwrap());
        
        // Test invalid expression (should fail at compile time)
        let expr = "$invalid_var == true";
        assert!(manager.compile(expr).is_err());
        
        // Test timeout scenario
        let expr = "/* complex expression */";
        // Mock timeout and verify fallback behavior
    }
}
```

**Performance Testing**:

```rust
#[bench]
fn bench_expression_evaluation(b: &mut Bencher) {
    let manager = TraitsManager::new();
    let context = /* ... */;
    let expr = "$tenant_kind == $subject_tenant_kind";
    
    b.iter(|| {
        manager.evaluate(expr, &context)
    });
    // Target: <100μs per evaluation (p95)
}
```

## Implementation Checklist

* [ ] Implement CEL parser integration with precompilation
* [ ] Add expression validation at setting type creation
* [ ] Implement in-memory AST cache with LRU eviction
* [ ] Add per-tenant evaluation result cache
* [ ] Implement timeout and resource limits
* [ ] Add comprehensive error handling with fallback policies
* [ ] Create audit log entries for evaluation failures
* [ ] Implement debug logging with configurable levels
* [ ] Add performance metrics and alerting
* [ ] Create test harness for expression validation
* [ ] Write integration tests for all supported operators
* [ ] Document security restrictions and operator whitelist
* [ ] Add performance benchmarks (target: p95 <100ms)

## Related Design Elements

**Principles**:

* `fdd-settings-service-principle-trait-configuration` - Trait-based configuration
* `fdd-settings-service-principle-ddd-light` - Domain-driven design

**Requirements**:

* `fdd-settings-service-fr-setting-type-definition` - Trait configuration
* `fdd-settings-service-fr-compliance-mode` - Compliance trait enforcement
* `fdd-settings-service-fr-access-control` - Access control trait enforcement

**Design Components**:

* TraitsManager - Domain component responsible for trait enforcement
* SettingsOperations.mutable_access_scope - CEL expressions for write access control
* SettingsOperations.read_access_scope - CEL expressions for read access control (future)
