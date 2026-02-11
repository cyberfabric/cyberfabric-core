use modkit_macros::domain_model;
use crate::domain::dto::{ListQuery, Route, Upstream};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors returned by repository operations.
#[domain_model]
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("internal: {0}")]
    #[allow(dead_code)]
    Internal(String),
}

// ---------------------------------------------------------------------------
// Repository traits
// ---------------------------------------------------------------------------

/// Repository trait for upstream persistence.
#[async_trait::async_trait]
pub trait UpstreamRepository: Send + Sync {
    /// Insert a new upstream. Returns Conflict if alias is taken for the tenant.
    async fn create(&self, upstream: Upstream) -> Result<Upstream, RepositoryError>;

    /// Get an upstream by id, scoped to a tenant.
    async fn get_by_id(&self, tenant_id: Uuid, id: Uuid) -> Result<Upstream, RepositoryError>;

    /// Get an upstream by alias, scoped to a tenant.
    async fn get_by_alias(&self, tenant_id: Uuid, alias: &str)
    -> Result<Upstream, RepositoryError>;

    /// List upstreams for a tenant with pagination.
    async fn list(
        &self,
        tenant_id: Uuid,
        query: &ListQuery,
    ) -> Result<Vec<Upstream>, RepositoryError>;

    /// Update an existing upstream. Preserves id and tenant_id.
    async fn update(&self, upstream: Upstream) -> Result<Upstream, RepositoryError>;

    /// Delete an upstream. Returns NotFound if it does not exist.
    async fn delete(&self, tenant_id: Uuid, id: Uuid) -> Result<(), RepositoryError>;
}

/// Repository trait for route persistence.
#[async_trait::async_trait]
pub trait RouteRepository: Send + Sync {
    /// Insert a new route.
    async fn create(&self, route: Route) -> Result<Route, RepositoryError>;

    /// Get a route by id, scoped to a tenant.
    async fn get_by_id(&self, tenant_id: Uuid, id: Uuid) -> Result<Route, RepositoryError>;

    /// List routes for a specific upstream with pagination.
    async fn list_by_upstream(
        &self,
        tenant_id: Uuid,
        upstream_id: Uuid,
        query: &ListQuery,
    ) -> Result<Vec<Route>, RepositoryError>;

    /// Find the best matching route for a given method and path.
    /// Match criteria: enabled=true, method matches, longest path prefix, highest priority.
    async fn find_matching(
        &self,
        tenant_id: Uuid,
        upstream_id: Uuid,
        method: &str,
        path: &str,
    ) -> Result<Route, RepositoryError>;

    /// Update an existing route.
    async fn update(&self, route: Route) -> Result<Route, RepositoryError>;

    /// Delete a route.
    async fn delete(&self, tenant_id: Uuid, id: Uuid) -> Result<(), RepositoryError>;

    /// Delete all routes for a given upstream. Returns the count of deleted routes.
    async fn delete_by_upstream(
        &self,
        tenant_id: Uuid,
        upstream_id: Uuid,
    ) -> Result<u64, RepositoryError>;
}
