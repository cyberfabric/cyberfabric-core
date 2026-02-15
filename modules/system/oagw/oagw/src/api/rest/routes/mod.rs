use axum::Router;
use modkit::api::OpenApiRegistry;
use modkit::api::operation_builder::{AuthReqAction, AuthReqResource, LicenseFeature};

use crate::module::AppState;

mod proxy;
mod route;
mod upstream;

pub(super) enum Resource {
    Upstreams,
    Routes,
}

pub(super) enum Action {
    Read,
    Create,
    Update,
    Delete,
}

pub(super) struct License;

impl AsRef<str> for Resource {
    fn as_ref(&self) -> &'static str {
        match self {
            Resource::Upstreams => "upstreams",
            Resource::Routes => "routes",
        }
    }
}

impl AuthReqResource for Resource {}

impl AsRef<str> for Action {
    fn as_ref(&self) -> &'static str {
        match self {
            Action::Read => "read",
            Action::Create => "create",
            Action::Update => "update",
            Action::Delete => "delete",
        }
    }
}

impl AuthReqAction for Action {}

impl AsRef<str> for License {
    fn as_ref(&self) -> &'static str {
        "gts.x.core.lic.feat.v1~x.core.oagw.base.v1"
    }
}

impl LicenseFeature for License {}

/// Register all OAGW REST routes with OpenAPI metadata.
pub fn register_routes(
    mut router: Router,
    openapi: &dyn OpenApiRegistry,
    state: AppState,
) -> Router {
    router = upstream::register(router, openapi);
    router = route::register(router, openapi);
    router = proxy::register(router);
    router.layer(axum::Extension(state))
}

/// Create a test router with all OAGW routes registered.
///
/// Uses manual route registration without OpenAPI metadata.
/// Suitable for integration tests that don't need an `OpenApiRegistry`.
#[cfg(any(test, feature = "test-utils"))]
pub fn test_router(state: AppState, ctx: modkit_security::SecurityContext) -> Router {
    use crate::api::rest::handlers::{proxy as proxy_h, route as route_h, upstream as upstream_h};
    use axum::routing::{any, get, post};

    Router::new()
        // Upstream CRUD
        .route("/oagw/v1/upstreams", post(upstream_h::create_upstream))
        .route("/oagw/v1/upstreams", get(upstream_h::list_upstreams))
        .route(
            "/oagw/v1/upstreams/{id}",
            get(upstream_h::get_upstream)
                .put(upstream_h::update_upstream)
                .delete(upstream_h::delete_upstream),
        )
        // Route CRUD
        .route("/oagw/v1/routes", post(route_h::create_route))
        .route(
            "/oagw/v1/routes/{id}",
            get(route_h::get_route)
                .put(route_h::update_route)
                .delete(route_h::delete_route),
        )
        .route(
            "/oagw/v1/upstreams/{upstream_id}/routes",
            get(route_h::list_routes),
        )
        // Proxy
        .route("/oagw/v1/proxy/{*path}", any(proxy_h::proxy_handler))
        .layer(axum::Extension(ctx))
        .layer(axum::Extension(state))
}
