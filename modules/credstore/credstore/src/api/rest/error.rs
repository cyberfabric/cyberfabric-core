//! Error mapping from `DomainError` to RFC-9457 `Problem` responses.

use axum::http::StatusCode;
use modkit::api::problem::Problem;

use crate::domain::DomainError;

/// Log any side effects for an error before producing the `Problem`.
// Tracing macros expand to complex code that inflates clippy's cognitive-complexity
// counter beyond what the logic warrants.
#[allow(clippy::cognitive_complexity)]
fn log_domain_error(e: &DomainError) {
    match e {
        DomainError::PluginNotFound { vendor } => {
            tracing::error!(%vendor, "No credstore plugin found for vendor");
        }
        DomainError::PluginUnavailable { gts_id, reason } => {
            tracing::warn!(%gts_id, %reason, "Credstore plugin not yet available");
        }
        DomainError::InvalidPluginInstance { gts_id, reason } => {
            tracing::error!(%gts_id, %reason, "Credstore plugin instance is misconfigured");
        }
        DomainError::TypesRegistryUnavailable(msg) => {
            tracing::error!(%msg, "Types registry unavailable");
        }
        DomainError::Internal(msg) => {
            tracing::error!(%msg, "Internal credstore error");
        }
        DomainError::NotFound => {}
    }
}

/// Map `DomainError` to an RFC-9457 `Problem` response.
///
/// # Errors
///
/// Always returns a `Problem`; never fails.
pub fn domain_error_to_problem(e: &DomainError) -> Problem {
    log_domain_error(e);
    match e {
        DomainError::NotFound => Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "Secret not found or inaccessible",
        ),
        DomainError::PluginNotFound { .. }
        | DomainError::PluginUnavailable { .. }
        | DomainError::InvalidPluginInstance { .. }
        | DomainError::TypesRegistryUnavailable(_) => Problem::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "Service Unavailable",
            "Storage backend temporarily unavailable",
        ),
        DomainError::Internal(_) => Problem::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error",
            "An internal error occurred",
        ),
    }
}

impl From<DomainError> for Problem {
    fn from(e: DomainError) -> Self {
        domain_error_to_problem(&e)
    }
}
