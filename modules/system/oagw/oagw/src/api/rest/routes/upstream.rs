use axum::Router;
use modkit::api::OpenApiRegistry;
use modkit::api::operation_builder::OperationBuilder;

use super::{Action, License, Resource};
use super::super::handlers;

pub(super) fn register(mut router: Router, openapi: &dyn OpenApiRegistry) -> Router {
    // POST /oagw/v1/upstreams — Create upstream
    router = OperationBuilder::post("/oagw/v1/upstreams")
        .operation_id("oagw.create_upstream")
        .summary("Create upstream")
        .description("Create a new upstream service configuration")
        .tag("upstreams")
        .require_auth(&Resource::Upstreams, &Action::Create)
        .require_license_features::<License>([])
        .handler(handlers::upstream::create_upstream)
        .json_response(http::StatusCode::CREATED, "Created upstream")
        .error_400(openapi)
        .error_500(openapi)
        .register(router, openapi);

    // GET /oagw/v1/upstreams — List upstreams
    router = OperationBuilder::get("/oagw/v1/upstreams")
        .operation_id("oagw.list_upstreams")
        .summary("List upstreams")
        .description("Retrieve a paginated list of upstream services")
        .tag("upstreams")
        .query_param_typed(
            "limit",
            false,
            "Maximum number of results (default 50, max 100)",
            "integer",
        )
        .query_param_typed("offset", false, "Number of results to skip", "integer")
        .require_auth(&Resource::Upstreams, &Action::Read)
        .require_license_features::<License>([])
        .handler(handlers::upstream::list_upstreams)
        .json_response(http::StatusCode::OK, "List of upstreams")
        .error_400(openapi)
        .error_500(openapi)
        .register(router, openapi);

    // GET /oagw/v1/upstreams/{id} — Get upstream
    router = OperationBuilder::get("/oagw/v1/upstreams/{id}")
        .operation_id("oagw.get_upstream")
        .summary("Get upstream by ID")
        .description("Retrieve a specific upstream by its GTS identifier")
        .tag("upstreams")
        .path_param("id", "Upstream GTS identifier")
        .require_auth(&Resource::Upstreams, &Action::Read)
        .require_license_features::<License>([])
        .handler(handlers::upstream::get_upstream)
        .json_response(http::StatusCode::OK, "Upstream found")
        .error_400(openapi)
        .error_404(openapi)
        .error_500(openapi)
        .register(router, openapi);

    // PUT /oagw/v1/upstreams/{id} — Update upstream
    router = OperationBuilder::put("/oagw/v1/upstreams/{id}")
        .operation_id("oagw.update_upstream")
        .summary("Update upstream")
        .description("Update an existing upstream service configuration")
        .tag("upstreams")
        .path_param("id", "Upstream GTS identifier")
        .require_auth(&Resource::Upstreams, &Action::Update)
        .require_license_features::<License>([])
        .handler(handlers::upstream::update_upstream)
        .json_response(http::StatusCode::OK, "Updated upstream")
        .error_400(openapi)
        .error_404(openapi)
        .error_500(openapi)
        .register(router, openapi);

    // DELETE /oagw/v1/upstreams/{id} — Delete upstream
    router = OperationBuilder::delete("/oagw/v1/upstreams/{id}")
        .operation_id("oagw.delete_upstream")
        .summary("Delete upstream")
        .description("Delete an upstream and cascade-delete its routes")
        .tag("upstreams")
        .path_param("id", "Upstream GTS identifier")
        .require_auth(&Resource::Upstreams, &Action::Delete)
        .require_license_features::<License>([])
        .handler(handlers::upstream::delete_upstream)
        .json_response(http::StatusCode::NO_CONTENT, "Upstream deleted")
        .error_404(openapi)
        .error_500(openapi)
        .register(router, openapi);

    router
}
