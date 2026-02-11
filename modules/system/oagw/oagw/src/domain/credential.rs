/// The resolved secret material.
#[derive(Debug, Clone)]
pub(crate) struct SecretValue {
    value: String,
}

impl SecretValue {
    #[must_use]
    pub(crate) fn new(value: String) -> Self {
        Self { value }
    }

    #[must_use]
    pub(crate) fn as_str(&self) -> &str {
        &self.value
    }

}

/// Intentionally does not display the secret value.
impl std::fmt::Display for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[REDACTED]")
    }
}

/// Errors from credential resolution.
#[derive(Debug, thiserror::Error)]
pub(crate) enum CredentialError {
    #[error("credential not found: {0}")]
    NotFound(String),
    #[error("credential error: {0}")]
    #[allow(dead_code)]
    Internal(String),
}

/// Trait for resolving secret references to their actual values.
#[async_trait::async_trait]
pub(crate) trait CredentialResolver: Send + Sync {
    /// Resolve a secret reference (e.g. `cred://openai-key`) to its value.
    ///
    /// # Errors
    /// Returns `CredentialError::NotFound` if the reference does not exist.
    async fn resolve(&self, secret_ref: &str) -> Result<SecretValue, CredentialError>;
}
