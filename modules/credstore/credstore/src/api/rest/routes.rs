//! Route registration for the credstore REST API.

use std::sync::Arc;

use axum::http::StatusCode;
use axum::{Extension, Router};
use modkit::api::operation_builder::LicenseFeature;
use modkit::api::{OpenApiRegistry, OperationBuilder};

use crate::domain::Service;

use super::{dto, handlers};

struct License;

impl AsRef<str> for License {
    fn as_ref(&self) -> &'static str {
        "gts.x.core.lic.feat.v1~x.core.global.base.v1"
    }
}

impl LicenseFeature for License {}

/// Register all credstore REST routes onto `router`.
pub fn register_routes(
    mut router: Router,
    openapi: &dyn OpenApiRegistry,
    service: Arc<Service>,
) -> Router {
    router = OperationBuilder::get("/api/credstore/v1/secrets/{secret_ref}")
        .operation_id("credstore.get_secret")
        .summary("Get a secret")
        .description("Retrieve a secret value and metadata. Returns 404 if not found or inaccessible (anti-enumeration).")
        .tag("Secrets")
        .path_param("secret_ref", "Secret reference key ([a-zA-Z0-9_-]+, max 255 chars)")
        .authenticated()
        .require_license_features::<License>([])
        .handler(handlers::get_secret)
        .json_response_with_schema::<dto::GetSecretResponseDto>(
            openapi,
            StatusCode::OK,
            "Secret retrieved successfully",
        )
        .error_400(openapi)
        .error_401(openapi)
        .error_403(openapi)
        .error_404(openapi)
        .error_500(openapi)
        .register(router, openapi);

    router = OperationBuilder::put("/api/credstore/v1/secrets/{secret_ref}")
        .operation_id("credstore.put_secret")
        .summary("Create or update a secret")
        .description(
            "Upsert a secret in the caller's tenant. Creates if absent, updates if present.",
        )
        .tag("Secrets")
        .path_param(
            "secret_ref",
            "Secret reference key ([a-zA-Z0-9_-]+, max 255 chars)",
        )
        .authenticated()
        .require_license_features::<License>([])
        .json_request::<dto::PutSecretRequest>(openapi, "Secret value and sharing mode")
        .handler(handlers::put_secret)
        .json_response(StatusCode::NO_CONTENT, "Secret stored")
        .error_400(openapi)
        .error_401(openapi)
        .error_403(openapi)
        .error_500(openapi)
        .register(router, openapi);

    router = OperationBuilder::delete("/api/credstore/v1/secrets/{secret_ref}")
        .operation_id("credstore.delete_secret")
        .summary("Delete a secret")
        .description("Delete a secret owned by the caller in the caller's tenant.")
        .tag("Secrets")
        .path_param(
            "secret_ref",
            "Secret reference key ([a-zA-Z0-9_-]+, max 255 chars)",
        )
        .authenticated()
        .require_license_features::<License>([])
        .handler(handlers::delete_secret)
        .json_response(StatusCode::NO_CONTENT, "Secret deleted")
        .error_400(openapi)
        .error_401(openapi)
        .error_403(openapi)
        .error_404(openapi)
        .error_500(openapi)
        .register(router, openapi);

    router = router.layer(Extension(service));

    router
}
