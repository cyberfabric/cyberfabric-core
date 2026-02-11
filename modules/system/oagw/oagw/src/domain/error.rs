use modkit_macros::domain_model;
use uuid::Uuid;

use super::repo::RepositoryError;

/// Domain-layer errors for OAGW control-plane and data-plane operations.
#[domain_model]
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("{entity} not found: {id}")]
    NotFound { entity: &'static str, id: Uuid },

    #[error("conflict: {detail}")]
    Conflict { detail: String },

    #[error("validation: {detail}")]
    Validation { detail: String, instance: String },

    #[error("upstream '{alias}' is disabled")]
    UpstreamDisabled { alias: String },

    #[error("internal: {message}")]
    Internal { message: String },

    #[error("target host header required for multi-endpoint upstream")]
    MissingTargetHost { instance: String },

    #[error("invalid target host header format")]
    InvalidTargetHost { instance: String },

    #[error("{detail}")]
    UnknownTargetHost { detail: String, instance: String },

    #[error("{detail}")]
    AuthenticationFailed { detail: String, instance: String },

    #[error("{detail}")]
    PayloadTooLarge { detail: String, instance: String },

    #[error("{detail}")]
    RateLimitExceeded {
        detail: String,
        instance: String,
        retry_after_secs: Option<u64>,
    },

    #[error("{detail}")]
    SecretNotFound { detail: String, instance: String },

    #[error("{detail}")]
    DownstreamError { detail: String, instance: String },

    #[error("{detail}")]
    ProtocolError { detail: String, instance: String },

    #[error("{detail}")]
    ConnectionTimeout { detail: String, instance: String },

    #[error("{detail}")]
    RequestTimeout { detail: String, instance: String },
}

impl DomainError {
    #[must_use]
    pub fn not_found(entity: &'static str, id: Uuid) -> Self {
        Self::NotFound { entity, id }
    }

    #[must_use]
    pub fn conflict(detail: impl Into<String>) -> Self {
        Self::Conflict {
            detail: detail.into(),
        }
    }

    #[must_use]
    pub fn validation(detail: impl Into<String>) -> Self {
        Self::Validation {
            detail: detail.into(),
            instance: String::new(),
        }
    }

    #[must_use]
    pub fn upstream_disabled(alias: impl Into<String>) -> Self {
        Self::UpstreamDisabled {
            alias: alias.into(),
        }
    }

    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// From<RepositoryError>
// ---------------------------------------------------------------------------

impl From<RepositoryError> for DomainError {
    fn from(e: RepositoryError) -> Self {
        match e {
            RepositoryError::NotFound => Self::NotFound {
                entity: "entity",
                id: Uuid::nil(),
            },
            RepositoryError::Conflict(detail) => Self::Conflict { detail },
            RepositoryError::Internal(message) => Self::Internal { message },
        }
    }
}
