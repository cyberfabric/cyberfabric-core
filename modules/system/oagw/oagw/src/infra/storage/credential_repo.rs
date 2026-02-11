use dashmap::DashMap;
use modkit_macros::domain_model;
use crate::domain::credential::{CredentialError, CredentialResolver, SecretValue};

/// In-memory credential resolver for development and testing.
#[domain_model]
pub struct InMemoryCredentialResolver {
    store: DashMap<String, String>,
}

impl InMemoryCredentialResolver {
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: DashMap::new(),
        }
    }

    /// Create a resolver pre-loaded with credentials.
    #[cfg(any(test, feature = "test-utils"))]
    #[must_use]
    pub fn with_credentials(creds: Vec<(String, String)>) -> Self {
        let resolver = Self::new();
        for (key, value) in creds {
            resolver.store.insert(key, value);
        }
        resolver
    }

    /// Add or update a credential.
    pub fn set(&self, secret_ref: String, value: String) {
        self.store.insert(secret_ref, value);
    }
}

impl Default for InMemoryCredentialResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl CredentialResolver for InMemoryCredentialResolver {
    async fn resolve(&self, secret_ref: &str) -> Result<SecretValue, CredentialError> {
        self.store
            .get(secret_ref)
            .map(|v| SecretValue::new(v.value().clone()))
            .ok_or_else(|| CredentialError::NotFound(secret_ref.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn resolve_existing_key() {
        let resolver = InMemoryCredentialResolver::with_credentials(vec![(
            "cred://openai-key".into(),
            "sk-abc123".into(),
        )]);

        let secret = resolver.resolve("cred://openai-key").await.unwrap();
        assert_eq!(secret.as_str(), "sk-abc123");
    }

    #[tokio::test]
    async fn resolve_missing_key_returns_not_found() {
        let resolver = InMemoryCredentialResolver::new();
        let result = resolver.resolve("cred://nonexistent").await;
        assert!(matches!(result, Err(CredentialError::NotFound(_))));
    }

    #[tokio::test]
    async fn set_and_resolve() {
        let resolver = InMemoryCredentialResolver::new();
        resolver.set("cred://key".into(), "secret-value".into());
        let secret = resolver.resolve("cred://key").await.unwrap();
        assert_eq!(secret.as_str(), "secret-value");
    }

    #[test]
    fn secret_value_display_is_redacted() {
        let sv = SecretValue::new("super-secret".into());
        assert_eq!(format!("{sv}"), "[REDACTED]");
    }
}
