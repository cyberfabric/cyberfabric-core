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

    /// The request was denied by the authorization policy.
    #[error("access forbidden: {detail}")]
    Forbidden { detail: String },
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

    /// Construct a [`DomainError::Forbidden`] with the given detail message.
    #[must_use]
    pub fn forbidden(detail: impl Into<String>) -> Self {
        Self::Forbidden {
            detail: detail.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// From<RepositoryError>
// ---------------------------------------------------------------------------

impl From<RepositoryError> for DomainError {
    fn from(e: RepositoryError) -> Self {
        match e {
            RepositoryError::NotFound { entity, id } => Self::NotFound { entity, id },
            RepositoryError::Conflict(detail) => Self::Conflict { detail },
            RepositoryError::Internal(message) => Self::Internal { message },
        }
    }
}

// ---------------------------------------------------------------------------
// From<EnforcerError>
// ---------------------------------------------------------------------------

/// Convert an authorization enforcer error into a domain error.
impl From<authz_resolver_sdk::EnforcerError> for DomainError {
    fn from(e: authz_resolver_sdk::EnforcerError) -> Self {
        use authz_resolver_sdk::EnforcerError;

        tracing::error!(error = %e, "OAGW authorization check failed");
        match e {
            EnforcerError::Denied { deny_reason } => {
                let detail = deny_reason
                    .map(|r| format!("{}: {}", r.error_code, r.details.unwrap_or_default()))
                    .unwrap_or_else(|| "access denied by policy".to_string());
                Self::Forbidden { detail }
            }
            EnforcerError::CompileFailed(_) => Self::Internal {
                message: "authorization constraint compilation failed".to_string(),
            },
            EnforcerError::EvaluationFailed(_) => Self::Internal {
                message: "authorization evaluation failed".to_string(),
            },
        }
    }
}
