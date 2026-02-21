use thiserror::Error;

/// Errors that can occur during credential store operations.
#[derive(Debug, Error)]
pub enum CredStoreError {
    #[error("invalid secret reference: {reason}")]
    InvalidSecretRef { reason: String },

    #[error("secret not found")]
    NotFound,

    #[error("no plugin available")]
    NoPluginAvailable,

    #[error("service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl CredStoreError {
    #[must_use]
    pub fn invalid_ref(reason: impl Into<String>) -> Self {
        Self::InvalidSecretRef {
            reason: reason.into(),
        }
    }

    #[must_use]
    pub fn service_unavailable(msg: impl Into<String>) -> Self {
        Self::ServiceUnavailable(msg.into())
    }

    #[must_use]
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}
