use axum::Router;
use axum::routing::any;

use super::super::handlers;

/// Register the proxy catch-all route.
///
/// Uses manual route registration because the proxy endpoint accepts all HTTP
/// methods on a wildcard path, which doesn't fit the OperationBuilder's
/// single-method pattern.
pub(super) fn register(router: Router) -> Router {
    router.route(
        "/oagw/v1/proxy/{*path}",
        any(handlers::proxy::proxy_handler),
    )
}
