//! Policy Enforcement Point (`PEP`) object.
//!
//! [`PolicyEnforcer`] encapsulates the full PEP flow:
//! build evaluation request → call PDP → compile constraints to `AccessScope`.
//!
//! Constructed once during service initialisation with the `AuthZ` client.
//! The resource type is supplied per call via a [`ResourceType`] descriptor,
//! so a single enforcer can serve all resource types in a service.

use std::collections::HashMap;
use std::sync::Arc;

use modkit_security::{AccessScope, SecurityContext};

use super::IntoPropertyValue;
use uuid::Uuid;

use crate::api::AuthZResolverClient;
use crate::error::AuthZResolverError;
use crate::models::{
    Action, BarrierMode, Capability, EvaluationRequest, EvaluationRequestContext, Resource,
    Subject, TenantContext, TenantMode,
};
use crate::pep::compiler::{ConstraintCompileError, compile_to_access_scope};

/// Error from the PEP enforcement flow.
#[derive(Debug, thiserror::Error)]
pub enum EnforcerError {
    /// The PDP explicitly denied access.
    #[error("access denied by PDP")]
    Denied {
        /// Optional deny reason from the PDP.
        deny_reason: Option<crate::models::DenyReason>,
    },

    /// The `AuthZ` evaluation RPC failed.
    #[error("authorization evaluation failed: {0}")]
    EvaluationFailed(#[from] AuthZResolverError),

    /// Constraint compilation failed (missing or unsupported constraints).
    #[error("constraint compilation failed: {0}")]
    CompileFailed(#[from] ConstraintCompileError),
}

/// Per-request evaluation parameters for advanced authorization scenarios.
///
/// Used with [`PolicyEnforcer::access_scope_with()`] when the simple
/// [`PolicyEnforcer::access_scope()`] defaults don't suffice (ABAC resource
/// properties, custom tenant mode, barrier bypass, etc.).
///
/// All fields default to "not overridden" — only set what you need.
///
/// # Examples
///
/// ```ignore
/// use authz_resolver_sdk::pep::{AccessRequest, PolicyEnforcer, ResourceType};
///
/// // CREATE with target tenant + resource properties (constrained scope)
/// let scope = enforcer.access_scope_with(
///     &ctx, &RESOURCE, "create", None,
///     &AccessRequest::new()
///         .context_tenant_id(target_tenant_id)
///         .tenant_mode(TenantMode::RootOnly)
///         .resource_property(pep_properties::OWNER_TENANT_ID, target_tenant_id),
/// ).await?;
///
/// // Billing — ignore barriers (constrained scope)
/// let scope = enforcer.access_scope_with(
///     &ctx, &RESOURCE, "list", None,
///     &AccessRequest::new().barrier_mode(BarrierMode::Ignore),
/// ).await?;
/// ```
#[derive(Debug, Clone, Default)]
pub struct AccessRequest {
    resource_properties: HashMap<String, serde_json::Value>,
    tenant_context: Option<TenantContext>,
    require_constraints: Option<bool>,
}

impl AccessRequest {
    /// Create a new empty access request (all defaults).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a single resource property for ABAC evaluation.
    #[must_use]
    pub fn resource_property(
        mut self,
        key: impl Into<String>,
        value: impl IntoPropertyValue,
    ) -> Self {
        self.resource_properties
            .insert(key.into(), value.into_filter_value());
        self
    }

    /// Set all resource properties at once (replaces any previously set).
    #[must_use]
    pub fn resource_properties(mut self, props: HashMap<String, serde_json::Value>) -> Self {
        self.resource_properties = props;
        self
    }

    /// Override the context tenant ID (default: subject's tenant).
    #[must_use]
    pub fn context_tenant_id(mut self, id: Uuid) -> Self {
        self.tenant_context.get_or_insert_default().root_id = Some(id);
        self
    }

    /// Override the tenant hierarchy mode (default: `Subtree`).
    #[must_use]
    pub fn tenant_mode(mut self, mode: TenantMode) -> Self {
        self.tenant_context.get_or_insert_default().mode = mode;
        self
    }

    /// Override the barrier enforcement mode (default: `Respect`).
    #[must_use]
    pub fn barrier_mode(mut self, mode: BarrierMode) -> Self {
        self.tenant_context.get_or_insert_default().barrier_mode = mode;
        self
    }

    /// Set a tenant status filter (e.g., `["active"]`).
    #[must_use]
    pub fn tenant_status(mut self, statuses: Vec<String>) -> Self {
        self.tenant_context.get_or_insert_default().tenant_status = Some(statuses);
        self
    }

    /// Set the entire tenant context at once.
    #[must_use]
    pub fn tenant_context(mut self, tc: TenantContext) -> Self {
        self.tenant_context = Some(tc);
        self
    }

    /// Override the `require_constraints` flag (default: `true`).
    ///
    /// When `false`, the PDP is told that constraints are optional.
    /// If the PDP returns no constraints, the resulting scope is
    /// `allow_all()` (no row-level filtering). If the PDP still returns
    /// constraints, they are compiled normally.
    ///
    /// Primary use cases:
    /// - **GET with prefetch**: if scope is unconstrained, return the
    ///   prefetched entity directly; otherwise do a scoped re-read.
    /// - **CREATE**: if scope is unconstrained, skip insert validation;
    ///   otherwise validate the insert against the scope.
    #[must_use]
    pub fn require_constraints(mut self, require: bool) -> Self {
        self.require_constraints = Some(require);
        self
    }
}

/// Static descriptor for a resource type and its supported constraint properties.
///
/// Passed per call to [`PolicyEnforcer`] methods so a single enforcer can
/// serve multiple resource types within one service.
#[derive(Debug, Clone, Copy)]
pub struct ResourceType {
    /// Dotted resource type name (e.g. `"gts.x.core.users.user.v1~"`).
    pub name: &'static str,
    /// Properties the PEP can compile from PDP constraints.
    pub supported_properties: &'static [&'static str],
}

/// Policy Enforcement Point.
///
/// Holds the `AuthZ` client and optional PEP capabilities.
/// Constructed once during service init; cloneable and cheap to pass
/// around (`Arc` inside). The resource type is supplied per call via
/// [`ResourceType`].
///
/// # Example
///
/// ```ignore
/// use authz_resolver_sdk::pep::{PolicyEnforcer, ResourceType};
/// use modkit_security::pep_properties;
///
/// const USER: ResourceType = ResourceType {
///     name: "gts.x.core.users.user.v1~",
///     supported_properties: &[pep_properties::OWNER_TENANT_ID, pep_properties::RESOURCE_ID],
/// };
///
/// let enforcer = PolicyEnforcer::new(authz.clone());
///
/// // All CRUD operations return AccessScope (PDP always returns constraints)
/// let scope = enforcer.access_scope(&ctx, &USER, "get", Some(id)).await?;
/// let scope = enforcer.access_scope(&ctx, &USER, "create", None).await?;
/// ```
#[derive(Clone)]
pub struct PolicyEnforcer {
    authz: Arc<dyn AuthZResolverClient>,
    capabilities: Vec<Capability>,
}

impl PolicyEnforcer {
    /// Create a new enforcer.
    pub fn new(authz: Arc<dyn AuthZResolverClient>) -> Self {
        Self {
            authz,
            capabilities: Vec::new(),
        }
    }

    /// Set PEP capabilities advertised to the PDP.
    #[must_use]
    pub fn with_capabilities(mut self, capabilities: Vec<Capability>) -> Self {
        self.capabilities = capabilities;
        self
    }

    // ── Low-level: build request only ────────────────────────────────

    /// Build an evaluation request using the subject's tenant as context tenant
    /// and default settings.
    #[must_use]
    pub fn build_request(
        &self,
        ctx: &SecurityContext,
        resource: &ResourceType,
        action: &str,
        resource_id: Option<Uuid>,
        require_constraints: bool,
    ) -> EvaluationRequest {
        self.build_request_with(
            ctx,
            resource,
            action,
            resource_id,
            require_constraints,
            &AccessRequest::default(),
        )
    }

    /// Build an evaluation request with per-request overrides from [`AccessRequest`].
    #[must_use]
    pub fn build_request_with(
        &self,
        ctx: &SecurityContext,
        resource: &ResourceType,
        action: &str,
        resource_id: Option<Uuid>,
        require_constraints: bool,
        request: &AccessRequest,
    ) -> EvaluationRequest {
        // Pass through the caller's tenant context as-is.
        // If no context_tenant_id was set, the PDP determines it by its own rules.
        let tenant_context = request.tenant_context.clone();

        // Put subject's tenant_id into properties per AuthZEN spec
        let mut subject_properties = HashMap::new();
        subject_properties.insert(
            "tenant_id".to_owned(),
            serde_json::Value::String(ctx.subject_tenant_id().to_string()),
        );

        let bearer_token = ctx.bearer_token().cloned();

        EvaluationRequest {
            subject: Subject {
                id: ctx.subject_id(),
                subject_type: ctx.subject_type().map(ToOwned::to_owned),
                properties: subject_properties,
            },
            action: Action {
                name: action.to_owned(),
            },
            resource: Resource {
                resource_type: resource.name.to_owned(),
                id: resource_id,
                properties: request.resource_properties.clone(),
            },
            context: EvaluationRequestContext {
                tenant_context,
                token_scopes: ctx.token_scopes().to_vec(),
                require_constraints,
                capabilities: self.capabilities.clone(),
                supported_properties: resource
                    .supported_properties
                    .iter()
                    .map(|s| (*s).to_owned())
                    .collect(),
                bearer_token,
            },
        }
    }

    // ── High-level: full PEP flow (all CRUD operations) ─────────────

    /// Execute the full PEP flow with constraints: build request → evaluate
    /// → compile constraints to `AccessScope`.
    ///
    /// Always sets `require_constraints=true`. PDP returns constraints for
    /// all CRUD operations (GET, LIST, UPDATE, DELETE, CREATE).
    ///
    /// # Errors
    ///
    /// - [`EnforcerError::EvaluationFailed`] if the PDP call fails
    /// - [`EnforcerError::CompileFailed`] if constraint compilation fails (denied, missing, etc.)
    pub async fn access_scope(
        &self,
        ctx: &SecurityContext,
        resource: &ResourceType,
        action: &str,
        resource_id: Option<Uuid>,
    ) -> Result<AccessScope, EnforcerError> {
        self.access_scope_with(
            ctx,
            resource,
            action,
            resource_id,
            &AccessRequest::default(),
        )
        .await
    }

    /// Execute the full PEP flow with constraints and per-request overrides.
    ///
    /// Uses `require_constraints` from [`AccessRequest`] (default: `true`).
    /// When `false`, the PDP may return no constraints; the resulting scope
    /// is `allow_all()`. When `true`, empty constraints trigger a compile error.
    ///
    /// # Errors
    ///
    /// - [`EnforcerError::EvaluationFailed`] if the PDP call fails
    /// - [`EnforcerError::CompileFailed`] if constraint compilation fails (denied, missing, etc.)
    pub async fn access_scope_with(
        &self,
        ctx: &SecurityContext,
        resource: &ResourceType,
        action: &str,
        resource_id: Option<Uuid>,
        request: &AccessRequest,
    ) -> Result<AccessScope, EnforcerError> {
        let require = request.require_constraints.unwrap_or(true);
        let eval_request =
            self.build_request_with(ctx, resource, action, resource_id, require, request);
        let response = self.authz.evaluate(eval_request).await?;

        // Check decision first: if denied, return error immediately
        // without attempting constraint compilation.
        if !response.decision {
            return Err(EnforcerError::Denied {
                deny_reason: response.context.deny_reason,
            });
        }

        Ok(compile_to_access_scope(
            &response,
            require,
            resource.supported_properties,
        )?)
    }
}

impl std::fmt::Debug for PolicyEnforcer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PolicyEnforcer")
            .field("capabilities", &self.capabilities)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use async_trait::async_trait;

    use super::*;
    use crate::constraints::{Constraint, InPredicate, Predicate};
    use crate::models::{EvaluationResponse, EvaluationResponseContext};
    use modkit_security::pep_properties;

    fn uuid(s: &str) -> Uuid {
        Uuid::parse_str(s).expect("valid test UUID")
    }

    const TENANT: &str = "11111111-1111-1111-1111-111111111111";
    const SUBJECT: &str = "22222222-2222-2222-2222-222222222222";
    const RESOURCE: &str = "33333333-3333-3333-3333-333333333333";

    /// Mock that returns `decision=true` with a tenant constraint from
    /// the request's `TenantContext.root_id` (always returns constraints,
    /// regardless of `require_constraints`).
    struct AllowAllMock;

    #[async_trait]
    impl AuthZResolverClient for AllowAllMock {
        async fn evaluate(
            &self,
            req: EvaluationRequest,
        ) -> Result<EvaluationResponse, AuthZResolverError> {
            let constraints = if let Some(ref tc) = req.context.tenant_context {
                if let Some(root_id) = tc.root_id {
                    vec![Constraint {
                        predicates: vec![Predicate::In(InPredicate::new(
                            pep_properties::OWNER_TENANT_ID,
                            [root_id],
                        ))],
                    }]
                } else {
                    vec![]
                }
            } else {
                vec![]
            };
            Ok(EvaluationResponse {
                decision: true,
                context: EvaluationResponseContext {
                    constraints,
                    ..Default::default()
                },
            })
        }
    }

    /// Mock that always returns `decision=false` with an optional deny reason.
    struct DenyMock {
        deny_reason: Option<crate::models::DenyReason>,
    }

    impl DenyMock {
        fn new() -> Self {
            Self { deny_reason: None }
        }

        fn with_reason(error_code: &str, details: Option<&str>) -> Self {
            Self {
                deny_reason: Some(crate::models::DenyReason {
                    error_code: error_code.to_owned(),
                    details: details.map(ToOwned::to_owned),
                }),
            }
        }
    }

    #[async_trait]
    impl AuthZResolverClient for DenyMock {
        async fn evaluate(
            &self,
            _req: EvaluationRequest,
        ) -> Result<EvaluationResponse, AuthZResolverError> {
            Ok(EvaluationResponse {
                decision: false,
                context: EvaluationResponseContext {
                    deny_reason: self.deny_reason.clone(),
                    ..Default::default()
                },
            })
        }
    }

    /// Mock that always returns an RPC error.
    struct FailMock;

    #[async_trait]
    impl AuthZResolverClient for FailMock {
        async fn evaluate(
            &self,
            _req: EvaluationRequest,
        ) -> Result<EvaluationResponse, AuthZResolverError> {
            Err(AuthZResolverError::Internal("boom".to_owned()))
        }
    }

    fn test_ctx() -> SecurityContext {
        SecurityContext::builder()
            .subject_id(uuid(SUBJECT))
            .subject_tenant_id(uuid(TENANT))
            .build()
            .unwrap()
    }

    const TEST_RESOURCE: ResourceType = ResourceType {
        name: "gts.x.core.users.user.v1~",
        supported_properties: &[pep_properties::OWNER_TENANT_ID, pep_properties::RESOURCE_ID],
    };

    fn enforcer(mock: impl AuthZResolverClient + 'static) -> PolicyEnforcer {
        PolicyEnforcer::new(Arc::new(mock))
    }

    // ── build_request ────────────────────────────────────────────────

    #[test]
    fn build_request_populates_fields() {
        let e = enforcer(AllowAllMock);
        let ctx = test_ctx();
        let req = e.build_request(&ctx, &TEST_RESOURCE, "get", Some(uuid(RESOURCE)), true);

        assert_eq!(req.resource.resource_type, "gts.x.core.users.user.v1~");
        assert_eq!(req.action.name, "get");
        assert_eq!(req.resource.id, Some(uuid(RESOURCE)));
        assert!(req.context.require_constraints);
        // No explicit context_tenant_id → tenant_context is None (PDP decides)
        assert!(req.context.tenant_context.is_none());
    }

    #[test]
    fn build_request_with_overrides_tenant() {
        let custom_tenant = uuid("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let e = enforcer(AllowAllMock);
        let ctx = test_ctx();
        let req = e.build_request_with(
            &ctx,
            &TEST_RESOURCE,
            "list",
            None,
            false,
            &AccessRequest::new().context_tenant_id(custom_tenant),
        );

        assert_eq!(
            req.context
                .tenant_context
                .as_ref()
                .and_then(|tc| tc.root_id),
            Some(custom_tenant),
        );
        assert!(!req.context.require_constraints);
    }

    // ── access_scope ─────────────────────────────────────────────────

    #[tokio::test]
    async fn access_scope_no_explicit_tenant_returns_compile_error() {
        let e = enforcer(AllowAllMock);
        let ctx = test_ctx();
        // No explicit context_tenant_id → mock returns empty constraints
        // → require_constraints=true → CompileFailed
        let result = e
            .access_scope(&ctx, &TEST_RESOURCE, "get", Some(uuid(RESOURCE)))
            .await;

        assert!(matches!(result, Err(EnforcerError::CompileFailed(_))));
    }

    #[tokio::test]
    async fn access_scope_with_explicit_tenant_returns_scope() {
        let e = enforcer(AllowAllMock);
        let ctx = test_ctx();
        let scope = e
            .access_scope_with(
                &ctx,
                &TEST_RESOURCE,
                "get",
                Some(uuid(RESOURCE)),
                &AccessRequest::new().context_tenant_id(uuid(TENANT)),
            )
            .await
            .expect("should succeed");

        assert_eq!(
            scope.all_uuid_values_for(pep_properties::OWNER_TENANT_ID),
            &[uuid(TENANT)]
        );
    }

    #[tokio::test]
    async fn access_scope_with_for_create() {
        let e = enforcer(AllowAllMock);
        let ctx = test_ctx();
        let scope = e
            .access_scope_with(
                &ctx,
                &TEST_RESOURCE,
                "create",
                None,
                &AccessRequest::new()
                    .context_tenant_id(uuid(TENANT))
                    .tenant_mode(TenantMode::RootOnly),
            )
            .await
            .expect("should succeed");

        assert_eq!(
            scope.all_uuid_values_for(pep_properties::OWNER_TENANT_ID),
            &[uuid(TENANT)]
        );
    }

    #[tokio::test]
    async fn access_scope_denied_returns_denied_error() {
        let e = enforcer(DenyMock::new());
        let ctx = test_ctx();
        let result = e.access_scope(&ctx, &TEST_RESOURCE, "get", None).await;

        assert!(matches!(
            result,
            Err(EnforcerError::Denied { deny_reason: None })
        ));
    }

    #[tokio::test]
    async fn access_scope_denied_with_reason() {
        let e = enforcer(DenyMock::with_reason(
            "INSUFFICIENT_PERMISSIONS",
            Some("Missing admin role"),
        ));
        let ctx = test_ctx();
        let result = e.access_scope(&ctx, &TEST_RESOURCE, "get", None).await;

        match result {
            Err(EnforcerError::Denied { deny_reason }) => {
                let reason = deny_reason.expect("should have deny_reason");
                assert_eq!(reason.error_code, "INSUFFICIENT_PERMISSIONS");
                assert_eq!(reason.details.as_deref(), Some("Missing admin role"));
            }
            other => panic!("Expected Denied with reason, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn access_scope_evaluation_failure() {
        let e = enforcer(FailMock);
        let ctx = test_ctx();
        let result = e.access_scope(&ctx, &TEST_RESOURCE, "get", None).await;

        assert!(matches!(result, Err(EnforcerError::EvaluationFailed(_))));
    }

    #[tokio::test]
    async fn access_scope_anonymous_no_tenant_returns_compile_error() {
        let e = enforcer(AllowAllMock);
        let ctx = SecurityContext::anonymous();
        // No explicit tenant context → mock returns empty constraints → CompileFailed
        let result = e.access_scope(&ctx, &TEST_RESOURCE, "list", None).await;

        assert!(matches!(result, Err(EnforcerError::CompileFailed(_))));
    }

    // ── builder methods ──────────────────────────────────────────────

    #[test]
    fn with_capabilities() {
        let e = enforcer(AllowAllMock).with_capabilities(vec![Capability::TenantHierarchy]);

        assert_eq!(e.capabilities, vec![Capability::TenantHierarchy]);
    }

    #[test]
    fn debug_impl() {
        let e = enforcer(AllowAllMock);
        let dbg = format!("{e:?}");
        assert!(dbg.contains("PolicyEnforcer"));
    }

    // ── AccessRequest builder ─────────────────────────────────────────

    #[test]
    fn access_request_default_is_empty() {
        let req = AccessRequest::new();
        assert!(req.resource_properties.is_empty());
        assert!(req.tenant_context.is_none());
    }

    #[test]
    fn access_request_builder_chain() {
        let tid = uuid(TENANT);
        let req = AccessRequest::new()
            .resource_property(pep_properties::OWNER_TENANT_ID, tid)
            .context_tenant_id(tid)
            .tenant_mode(TenantMode::RootOnly)
            .barrier_mode(BarrierMode::Ignore)
            .tenant_status(vec!["active".to_owned()]);

        assert_eq!(req.resource_properties.len(), 1);
        let tc = req.tenant_context.as_ref().expect("tenant context");
        assert_eq!(tc.root_id, Some(tid));
        assert_eq!(tc.mode, TenantMode::RootOnly);
        assert_eq!(tc.barrier_mode, BarrierMode::Ignore);
        assert_eq!(tc.tenant_status, Some(vec!["active".to_owned()]));
    }

    #[test]
    fn access_request_tenant_context_setter() {
        let tid = uuid(TENANT);
        let req = AccessRequest::new().tenant_context(TenantContext {
            mode: TenantMode::RootOnly,
            root_id: Some(tid),
            ..Default::default()
        });

        let tc = req.tenant_context.as_ref().expect("tenant context");
        assert_eq!(tc.root_id, Some(tid));
        assert_eq!(tc.mode, TenantMode::RootOnly);
        assert_eq!(tc.barrier_mode, BarrierMode::Respect);
    }

    #[test]
    fn access_request_resource_properties_replaces() {
        let mut props = HashMap::new();
        props.insert("a".to_owned(), serde_json::json!("1"));
        props.insert("b".to_owned(), serde_json::json!("2"));

        let req = AccessRequest::new()
            .resource_property("old_key", serde_json::json!("old"))
            .resource_properties(props);

        assert_eq!(req.resource_properties.len(), 2);
        assert!(!req.resource_properties.contains_key("old_key"));
    }

    #[test]
    fn into_property_value_implementations() {
        let uuid_val = uuid(TENANT);
        let req = AccessRequest::new()
            .resource_property("uuid_prop", uuid_val)
            .resource_property("uuid_ref_prop", uuid_val)
            .resource_property("string_prop", "test".to_owned())
            .resource_property("str_prop", "test")
            .resource_property("int_prop", 42i64)
            .resource_property("json_prop", serde_json::json!({"key": "value"}));

        assert_eq!(req.resource_properties.len(), 6);
        assert_eq!(
            req.resource_properties.get("uuid_prop"),
            Some(&serde_json::json!(uuid_val.to_string())),
        );
        assert_eq!(
            req.resource_properties.get("string_prop"),
            Some(&serde_json::json!("test")),
        );
        assert_eq!(
            req.resource_properties.get("int_prop"),
            Some(&serde_json::json!(42)),
        );
    }

    // ── build_request_with ────────────────────────────────────────────

    #[test]
    fn build_request_with_applies_resource_properties() {
        let e = enforcer(AllowAllMock);
        let ctx = test_ctx();
        let tid = uuid(TENANT);
        let req = e.build_request_with(
            &ctx,
            &TEST_RESOURCE,
            "create",
            None,
            false,
            &AccessRequest::new().resource_property(pep_properties::OWNER_TENANT_ID, tid),
        );

        assert_eq!(
            req.resource.properties.get(pep_properties::OWNER_TENANT_ID),
            Some(&serde_json::json!(tid.to_string())),
        );
    }

    #[test]
    fn build_request_with_applies_tenant_mode_and_barrier() {
        let e = enforcer(AllowAllMock);
        let ctx = test_ctx();
        let req = e.build_request_with(
            &ctx,
            &TEST_RESOURCE,
            "list",
            None,
            true,
            &AccessRequest::new()
                .tenant_mode(TenantMode::RootOnly)
                .barrier_mode(BarrierMode::Ignore)
                .tenant_status(vec!["active".to_owned()]),
        );

        let tc = req.context.tenant_context.as_ref().expect("tenant context");
        assert_eq!(tc.mode, TenantMode::RootOnly);
        assert_eq!(tc.barrier_mode, BarrierMode::Ignore);
        assert_eq!(tc.tenant_status, Some(vec!["active".to_owned()]));
    }

    #[test]
    fn build_request_with_default_has_no_tenant_context() {
        let e = enforcer(AllowAllMock);
        let ctx = test_ctx();
        let req = e.build_request_with(
            &ctx,
            &TEST_RESOURCE,
            "get",
            None,
            true,
            &AccessRequest::default(),
        );

        // No explicit context_tenant_id → tenant_context is None (PDP decides)
        assert!(req.context.tenant_context.is_none());
    }

    // ── access_scope_with ─────────────────────────────────────────────

    #[tokio::test]
    async fn access_scope_with_custom_tenant() {
        let custom_tenant = uuid("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let e = enforcer(AllowAllMock);
        let ctx = test_ctx();
        let scope = e
            .access_scope_with(
                &ctx,
                &TEST_RESOURCE,
                "list",
                None,
                &AccessRequest::new().context_tenant_id(custom_tenant),
            )
            .await
            .expect("should succeed");

        assert_eq!(
            scope.all_uuid_values_for(pep_properties::OWNER_TENANT_ID),
            &[custom_tenant]
        );
    }

    #[tokio::test]
    async fn access_scope_with_resource_properties() {
        let e = enforcer(AllowAllMock);
        let ctx = test_ctx();
        let scope = e
            .access_scope_with(
                &ctx,
                &TEST_RESOURCE,
                "get",
                None,
                &AccessRequest::new()
                    .resource_property(
                        pep_properties::OWNER_TENANT_ID,
                        serde_json::json!(uuid(TENANT).to_string()),
                    )
                    .context_tenant_id(uuid(TENANT))
                    .tenant_mode(TenantMode::RootOnly),
            )
            .await
            .expect("should succeed");

        assert_eq!(
            scope.all_uuid_values_for(pep_properties::OWNER_TENANT_ID),
            &[uuid(TENANT)]
        );
    }

    // ── request builder internals ────────────────────────────────────

    #[test]
    fn builds_request_with_all_fields() {
        const USERS_RESOURCE: ResourceType = ResourceType {
            name: "gts.x.core.users.user.v1~",
            supported_properties: &[pep_properties::OWNER_TENANT_ID],
        };

        let context_tenant_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let subject_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
        let subject_tenant_id = Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap();
        let resource_id = Uuid::parse_str("44444444-4444-4444-4444-444444444444").unwrap();

        let ctx = SecurityContext::builder()
            .subject_id(subject_id)
            .subject_tenant_id(subject_tenant_id)
            .subject_type("user")
            .token_scopes(vec!["admin".to_owned()])
            .bearer_token("test-token".to_owned())
            .build()
            .unwrap();

        let e = PolicyEnforcer::new(Arc::new(AllowAllMock))
            .with_capabilities(vec![Capability::TenantHierarchy]);

        let access_req = AccessRequest::new().tenant_context(TenantContext {
            root_id: Some(context_tenant_id),
            ..Default::default()
        });

        let request = e.build_request_with(
            &ctx,
            &USERS_RESOURCE,
            "get",
            Some(resource_id),
            true,
            &access_req,
        );

        assert_eq!(request.subject.id, subject_id);
        assert_eq!(
            request.subject.properties.get("tenant_id").unwrap(),
            &serde_json::Value::String(subject_tenant_id.to_string())
        );
        assert_eq!(request.subject.subject_type.as_deref(), Some("user"));
        assert_eq!(request.action.name, "get");
        assert_eq!(request.resource.resource_type, "gts.x.core.users.user.v1~");
        assert_eq!(request.resource.id, Some(resource_id));
        assert!(request.context.require_constraints);
        assert_eq!(
            request.context.tenant_context.as_ref().unwrap().root_id,
            Some(context_tenant_id)
        );
        assert_eq!(request.context.token_scopes, vec!["admin"]);
        assert_eq!(
            request.context.capabilities,
            vec![Capability::TenantHierarchy]
        );
        assert!(request.context.bearer_token.is_some());
        assert_eq!(
            request.context.supported_properties,
            vec![pep_properties::OWNER_TENANT_ID]
        );
    }

    #[test]
    fn builds_request_without_tenant_context() {
        let ctx = SecurityContext::anonymous();

        let e = enforcer(AllowAllMock);

        let request = e.build_request_with(
            &ctx,
            &TEST_RESOURCE,
            "create",
            None,
            false,
            &AccessRequest::default(),
        );

        assert!(request.context.tenant_context.is_none());
        assert!(!request.context.require_constraints);
        assert_eq!(request.resource.id, None);
        assert!(request.context.capabilities.is_empty());
        assert!(request.context.bearer_token.is_none());
    }

    #[test]
    fn applies_resource_properties() {
        let tenant_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let ctx = SecurityContext::builder()
            .subject_id(Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap())
            .subject_tenant_id(tenant_id)
            .build()
            .unwrap();

        let e = enforcer(AllowAllMock);
        let access_req = AccessRequest::new()
            .resource_property(
                pep_properties::OWNER_TENANT_ID,
                serde_json::Value::String(tenant_id.to_string()),
            )
            .context_tenant_id(tenant_id);

        let request =
            e.build_request_with(&ctx, &TEST_RESOURCE, "create", None, false, &access_req);

        assert_eq!(
            request
                .resource
                .properties
                .get(pep_properties::OWNER_TENANT_ID),
            Some(&serde_json::Value::String(tenant_id.to_string())),
        );
    }

    #[test]
    fn applies_tenant_mode_and_barrier_mode() {
        let tenant_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let ctx = SecurityContext::builder()
            .subject_id(Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap())
            .subject_tenant_id(tenant_id)
            .build()
            .unwrap();

        let e = enforcer(AllowAllMock);
        let access_req = AccessRequest::new().tenant_context(TenantContext {
            mode: TenantMode::RootOnly,
            root_id: Some(tenant_id),
            barrier_mode: BarrierMode::Ignore,
            tenant_status: Some(vec!["active".to_owned()]),
        });

        let request = e.build_request_with(&ctx, &TEST_RESOURCE, "list", None, true, &access_req);

        let tc = request.context.tenant_context.as_ref().unwrap();
        assert_eq!(tc.mode, TenantMode::RootOnly);
        assert_eq!(tc.barrier_mode, BarrierMode::Ignore);
        assert_eq!(tc.tenant_status, Some(vec!["active".to_owned()]));
    }

    #[test]
    fn no_implicit_fallback_to_subject_tenant_id() {
        let subject_tenant = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let ctx = SecurityContext::builder()
            .subject_id(Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap())
            .subject_tenant_id(subject_tenant)
            .build()
            .unwrap();

        let e = enforcer(AllowAllMock);

        // No tenant_context provided — PDP decides, no implicit fallback
        let request = e.build_request_with(
            &ctx,
            &TEST_RESOURCE,
            "list",
            None,
            true,
            &AccessRequest::default(),
        );

        assert!(request.context.tenant_context.is_none());
    }

    #[test]
    fn explicit_root_id_overrides_subject_tenant() {
        let subject_tenant = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let explicit_tenant = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let ctx = SecurityContext::builder()
            .subject_id(Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap())
            .subject_tenant_id(subject_tenant)
            .build()
            .unwrap();

        let e = enforcer(AllowAllMock);
        let access_req = AccessRequest::new().context_tenant_id(explicit_tenant);

        let request = e.build_request_with(&ctx, &TEST_RESOURCE, "get", None, true, &access_req);

        let tc = request.context.tenant_context.as_ref().unwrap();
        assert_eq!(tc.root_id, Some(explicit_tenant));
    }
}
