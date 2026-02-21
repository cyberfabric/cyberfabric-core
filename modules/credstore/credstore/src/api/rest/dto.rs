//! REST DTOs for the credstore module.

use axum::http::StatusCode;
use credstore_sdk::{GetSecretResponse, SharingMode};
use modkit::api::problem::Problem;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Sharing mode DTO for REST API.
///
/// Local mirror of [`credstore_sdk::SharingMode`] with `ToSchema` so it can
/// be included in `OpenAPI` schemas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SharingModeDto {
    /// Only the owner can access the secret.
    Private,
    /// All users within the owner's tenant can access the secret.
    #[default]
    Tenant,
    /// The secret is accessible across tenant boundaries.
    Shared,
}

impl From<SharingMode> for SharingModeDto {
    fn from(m: SharingMode) -> Self {
        match m {
            SharingMode::Private => Self::Private,
            SharingMode::Tenant => Self::Tenant,
            SharingMode::Shared => Self::Shared,
        }
    }
}

impl From<SharingModeDto> for SharingMode {
    fn from(m: SharingModeDto) -> Self {
        match m {
            SharingModeDto::Private => Self::Private,
            SharingModeDto::Tenant => Self::Tenant,
            SharingModeDto::Shared => Self::Shared,
        }
    }
}

/// Request body for `PUT /api/credstore/v1/secrets/{ref}`.
#[derive(Debug)]
#[modkit_macros::api_dto(request)]
pub struct PutSecretRequest {
    /// Secret value as a UTF-8 string.
    pub value: String,
    /// Sharing mode. Defaults to `"tenant"` if omitted.
    #[serde(default)]
    pub sharing: SharingModeDto,
}

/// Response body for `GET /api/credstore/v1/secrets/{ref}`.
#[derive(Debug)]
#[modkit_macros::api_dto(response)]
pub struct GetSecretResponseDto {
    /// Secret value as a UTF-8 string.
    pub value: String,
    /// Tenant that owns this secret (may differ from the requesting tenant
    /// when the secret is inherited via hierarchical resolution).
    #[schema(value_type = String)]
    pub owner_tenant_id: Uuid,
    /// Sharing mode of the secret.
    pub sharing: SharingModeDto,
    /// `true` if the secret was resolved from an ancestor tenant.
    pub is_inherited: bool,
}

impl TryFrom<GetSecretResponse> for GetSecretResponseDto {
    type Error = Problem;

    fn try_from(r: GetSecretResponse) -> Result<Self, Self::Error> {
        let value = String::from_utf8(r.value.as_bytes().to_vec()).map_err(|_| {
            Problem::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Encoding Error",
                "Secret value contains non-UTF-8 bytes and cannot be returned as a string",
            )
        })?;
        Ok(Self {
            value,
            owner_tenant_id: r.owner_tenant_id,
            sharing: r.sharing.into(),
            is_inherited: r.is_inherited,
        })
    }
}
