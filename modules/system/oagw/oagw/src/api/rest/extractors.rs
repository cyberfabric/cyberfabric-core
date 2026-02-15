use crate::domain::gts_helpers;
use crate::domain::dto::ListQuery;
use uuid::Uuid;

use crate::api::rest::error::error_response;

/// Parse a GTS identifier to extract the UUID.
///
/// # Errors
/// Returns an error response if the GTS string is invalid.
#[allow(clippy::result_large_err)]
pub fn parse_gts_id(gts_str: &str, _instance: &str) -> Result<Uuid, axum::response::Response> {
    let (_, uuid) = gts_helpers::parse_resource_gts(gts_str).map_err(error_response)?;
    Ok(uuid)
}

/// Pagination query parameters.
#[derive(Debug, serde::Deserialize)]
pub struct PaginationQuery {
    #[serde(default = "default_top")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

fn default_top() -> u32 {
    50
}

impl PaginationQuery {
    pub fn to_list_query(&self) -> ListQuery {
        ListQuery {
            top: self.limit.min(100),
            skip: self.offset,
        }
    }
}
