//! Test utilities for CP and DP integration tests.

use std::sync::Arc;
use std::time::Duration;

use modkit::client_hub::ClientHub;
use oagw_sdk::api::ServiceGatewayClientV1;
use crate::domain::credential::CredentialResolver;

use crate::domain::services::{
    ControlPlaneService, ControlPlaneServiceImpl, DataPlaneService, DataPlaneServiceImpl,
    ServiceGatewayClientV1Facade,
};
use crate::infra::storage::{InMemoryCredentialResolver, InMemoryRouteRepo, InMemoryUpstreamRepo};

/// Re-export for tests that need to set credentials after creation.
pub use crate::infra::storage::credential_repo::InMemoryCredentialResolver as TestCredentialResolver;

/// Re-export plugin ID constants for test configurations.
pub use crate::infra::plugin::apikey_auth::APIKEY_AUTH_PLUGIN_ID;

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
}

impl TestDpBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            request_timeout: None,
        }
    }

    /// Override the request timeout (useful for timeout tests).
    #[must_use]
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = Some(timeout);
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

        let mut svc = DataPlaneServiceImpl::new(cp, cred_resolver);
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
        state: crate::module::AppState { cp, dp },
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
    let oagw: Arc<dyn ServiceGatewayClientV1> =
        Arc::new(ServiceGatewayClientV1Facade::new(cp, dp));
    hub.register::<dyn ServiceGatewayClientV1>(oagw.clone());
    oagw
}
