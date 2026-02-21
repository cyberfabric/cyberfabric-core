//! REST handlers for the credstore module.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Extension, Path};
use axum::http::StatusCode;
use credstore_sdk::{SecretRef, SecretValue};
use modkit::api::prelude::*;
use modkit_security::SecurityContext;

use super::dto::{GetSecretResponseDto, PutSecretRequest};
use crate::domain::Service;

/// `GET /api/credstore/v1/secrets/{secret_ref}`
///
/// Returns the secret value and metadata for the given key.
/// Returns 404 if the secret does not exist or is not accessible (anti-enumeration).
///
/// # Errors
///
/// Returns a `Problem` for invalid `secret_ref` format (400), plugin failures (503),
/// or internal errors (500). Returns 404 when the secret is absent or inaccessible.
pub async fn get_secret(
    Extension(ctx): Extension<SecurityContext>,
    Extension(svc): Extension<Arc<Service>>,
    Path(key_str): Path<String>,
) -> ApiResult<JsonBody<GetSecretResponseDto>> {
    let key = SecretRef::new(key_str).map_err(|e| {
        Problem::new(
            StatusCode::BAD_REQUEST,
            "Invalid Secret Reference",
            e.to_string(),
        )
    })?;

    match svc.get(&ctx, &key).await? {
        Some(resp) => Ok(Json(GetSecretResponseDto::try_from(resp)?)),
        None => Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "Secret not found or inaccessible",
        )),
    }
}

/// `PUT /api/credstore/v1/secrets/{secret_ref}`
///
/// Creates or updates a secret for the caller's tenant (upsert semantics).
///
/// # Errors
///
/// Returns a `Problem` for invalid `secret_ref` format (400), plugin failures (503),
/// or internal errors (500).
pub async fn put_secret(
    Extension(ctx): Extension<SecurityContext>,
    Extension(svc): Extension<Arc<Service>>,
    Path(key_str): Path<String>,
    Json(req): Json<PutSecretRequest>,
) -> ApiResult<impl IntoResponse> {
    let key = SecretRef::new(key_str).map_err(|e| {
        Problem::new(
            StatusCode::BAD_REQUEST,
            "Invalid Secret Reference",
            e.to_string(),
        )
    })?;
    let value = SecretValue::from(req.value);
    let sharing = req.sharing.into();

    svc.put(&ctx, &key, value, sharing).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /api/credstore/v1/secrets/{secret_ref}`
///
/// Deletes a secret owned by the caller in the caller's tenant.
///
/// # Errors
///
/// Returns a `Problem` for invalid `secret_ref` format (400), not-found (404),
/// plugin failures (503), or internal errors (500).
pub async fn delete_secret(
    Extension(ctx): Extension<SecurityContext>,
    Extension(svc): Extension<Arc<Service>>,
    Path(key_str): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let key = SecretRef::new(key_str).map_err(|e| {
        Problem::new(
            StatusCode::BAD_REQUEST,
            "Invalid Secret Reference",
            e.to_string(),
        )
    })?;

    svc.delete(&ctx, &key).await?;
    Ok(StatusCode::NO_CONTENT)
}
