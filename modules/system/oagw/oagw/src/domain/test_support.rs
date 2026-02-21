//! Test utilities for CP and DP integration tests.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::domain::credential::CredentialResolver;
use async_trait::async_trait;
use authz_resolver_sdk::{
    AuthZResolverClient, AuthZResolverError, EvaluationRequest, EvaluationResponse,
    EvaluationResponseContext, PolicyEnforcer,
};
use modkit::client_hub::ClientHub;
use oagw_sdk::api::ServiceGatewayClientV1;

use crate::domain::services::{
    ControlPlaneService, ControlPlaneServiceImpl, DataPlaneService, ServiceGatewayClientV1Facade,
};
use crate::infra::proxy::DataPlaneServiceImpl;
use crate::infra::storage::{InMemoryCredentialResolver, InMemoryRouteRepo, InMemoryUpstreamRepo};

/// Mock AuthZ resolver that always allows access for testing.
struct MockAuthZResolverClient;

/// Always returns `Allow` so tests that do not care about authorization pass by default.
#[async_trait]
impl AuthZResolverClient for MockAuthZResolverClient {
    async fn evaluate(
        &self,
        _request: EvaluationRequest,
    ) -> Result<EvaluationResponse, AuthZResolverError> {
        Ok(EvaluationResponse {
            decision: true,
            context: EvaluationResponseContext {
                constraints: Vec::new(),
                deny_reason: None,
            },
        })
    }
}

/// Mock AuthZ resolver that always denies access for testing.
pub struct DenyingAuthZResolverClient;

#[async_trait]
impl AuthZResolverClient for DenyingAuthZResolverClient {
    async fn evaluate(
        &self,
        _request: EvaluationRequest,
    ) -> Result<EvaluationResponse, AuthZResolverError> {
        Ok(EvaluationResponse {
            decision: false,
            context: EvaluationResponseContext {
                constraints: Vec::new(),
                deny_reason: None,
            },
        })
    }
}

/// Records all evaluation requests for post-hoc inspection.
/// Configurable decision (default: allow).
pub struct CapturingAuthZResolverClient {
    pub requests: Arc<Mutex<Vec<EvaluationRequest>>>,
    decision: bool,
}

impl CapturingAuthZResolverClient {
    /// Create a new allowing [`CapturingAuthZResolverClient`].
    pub fn new() -> Self {
        Self {
            requests: Arc::new(Mutex::new(vec![])),
            decision: true,
        }
    }

    /// Create a denying variant that records requests and returns `Deny`.
    pub fn denying() -> Self {
        Self {
            decision: false,
            ..Self::new()
        }
    }

    /// Return a snapshot of all recorded evaluation requests.
    pub fn recorded(&self) -> Vec<EvaluationRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl Default for CapturingAuthZResolverClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AuthZResolverClient for CapturingAuthZResolverClient {
    async fn evaluate(
        &self,
        request: EvaluationRequest,
    ) -> Result<EvaluationResponse, AuthZResolverError> {
        self.requests.lock().unwrap().push(request);
        Ok(EvaluationResponse {
            decision: self.decision,
            context: EvaluationResponseContext {
                constraints: Vec::new(),
                deny_reason: None,
            },
        })
    }
}

/// Re-export for tests that need to set credentials after creation.
pub use crate::infra::storage::credential_repo::InMemoryCredentialResolver as TestCredentialResolver;

/// Re-export plugin ID constants for test configurations.
pub use crate::domain::gts_helpers::APIKEY_AUTH_PLUGIN_ID;

/// Builder for a fully-wired Control Plane test environment.
pub struct TestCpBuilder {
    credentials: Vec<(String, String)>,
}

impl TestCpBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            credentials: Vec::new(),
        }
    }

    /// Pre-load credentials into the credential resolver.
    #[must_use]
    pub fn with_credentials(mut self, creds: Vec<(String, String)>) -> Self {
        self.credentials = creds;
        self
    }

    /// Create repos, service, and credential resolver, register them in the
    /// provided `ClientHub`, and return the CP service trait object.
    pub(crate) fn build_and_register(self, hub: &ClientHub) -> Arc<dyn ControlPlaneService> {
        let upstream_repo = Arc::new(InMemoryUpstreamRepo::new());
        let route_repo = Arc::new(InMemoryRouteRepo::new());
        let cp: Arc<dyn ControlPlaneService> =
            Arc::new(ControlPlaneServiceImpl::new(upstream_repo, route_repo));

        let cred_resolver: Arc<dyn CredentialResolver> = Arc::new(
            InMemoryCredentialResolver::with_credentials(self.credentials),
        );

        hub.register::<dyn CredentialResolver>(cred_resolver);

        cp
    }
}

impl Default for TestCpBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for a fully-wired Data Plane test environment.
///
/// Requires that a `CredentialResolver` is already registered in the
/// `ClientHub` (e.g., via `TestCpBuilder`).
pub struct TestDpBuilder {
    request_timeout: Option<Duration>,
    authz_client: Option<Arc<dyn AuthZResolverClient>>,
}

impl TestDpBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            request_timeout: None,
            authz_client: None,
        }
    }

    /// Override the request timeout (useful for timeout tests).
    #[must_use]
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = Some(timeout);
        self
    }

    /// Override the AuthZ client (useful for authorization tests).
    #[must_use]
    pub fn with_authz_client(mut self, client: Arc<dyn AuthZResolverClient>) -> Self {
        self.authz_client = Some(client);
        self
    }

    /// Fetch CredentialResolver from the hub, create a DP service with
    /// the given CP, and return the trait object.
    pub(crate) fn build_and_register(
        self,
        hub: &ClientHub,
        cp: Arc<dyn ControlPlaneService>,
    ) -> Arc<dyn DataPlaneService> {
        let cred_resolver = hub
            .get::<dyn CredentialResolver>()
            .expect("CredentialResolver must be registered before building DP");

        let authz_client = self
            .authz_client
            .unwrap_or_else(|| Arc::new(MockAuthZResolverClient));
        let policy_enforcer = PolicyEnforcer::new(authz_client);

        let mut svc = DataPlaneServiceImpl::new(cp, cred_resolver, policy_enforcer)
            .expect("failed to build DataPlaneServiceImpl in test");
        if let Some(timeout) = self.request_timeout {
            svc = svc.with_request_timeout(timeout);
        }

        Arc::new(svc)
    }
}

impl Default for TestDpBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Test harness providing both an `AppState` (for REST handlers) and a
/// `ServiceGatewayClientV1` facade (for programmatic data setup in tests).
pub struct TestAppState {
    pub state: crate::module::AppState,
    pub facade: Arc<dyn ServiceGatewayClientV1>,
}

/// Build an `AppState` and facade for integration tests.
///
/// Use `result.state` when constructing an axum test router and
/// `result.facade` when you need to create data programmatically
/// (e.g. `facade.create_upstream(…)`).
pub fn build_test_app_state(
    hub: &ClientHub,
    cp_builder: TestCpBuilder,
    dp_builder: TestDpBuilder,
) -> TestAppState {
    let cp = cp_builder.build_and_register(hub);
    let dp = dp_builder.build_and_register(hub, cp.clone());
    let facade: Arc<dyn ServiceGatewayClientV1> =
        Arc::new(ServiceGatewayClientV1Facade::new(cp.clone(), dp.clone()));
    hub.register::<dyn ServiceGatewayClientV1>(facade.clone());
    TestAppState {
        state: crate::module::AppState {
            cp,
            dp,
            config: crate::config::RuntimeConfig {
                max_body_size_bytes: 100 * 1024 * 1024, // 100 MB default for tests
            },
        },
        facade,
    }
}

/// Build a fully wired `ServiceGatewayClientV1` facade for integration tests.
/// Returns the facade registered in `client_hub`.
pub fn build_test_gateway(
    hub: &ClientHub,
    cp_builder: TestCpBuilder,
    dp_builder: TestDpBuilder,
) -> Arc<dyn ServiceGatewayClientV1> {
    let cp = cp_builder.build_and_register(hub);
    let dp = dp_builder.build_and_register(hub, cp.clone());
    let oagw: Arc<dyn ServiceGatewayClientV1> = Arc::new(ServiceGatewayClientV1Facade::new(cp, dp));
    hub.register::<dyn ServiceGatewayClientV1>(oagw.clone());
    oagw
}
