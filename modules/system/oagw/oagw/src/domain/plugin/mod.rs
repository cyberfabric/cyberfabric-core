pub(crate) mod registry;

pub(crate) use registry::AuthPluginRegistry;

use http::HeaderMap;

// ---------------------------------------------------------------------------
// Plugin errors
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("secret not found: {0}")]
    SecretNotFound(String),
    #[error("authentication failed: {0}")]
    AuthFailed(String),
    #[error("request rejected: {0}")]
    Rejected(String),
    #[error("plugin error: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// Auth plugin
// ---------------------------------------------------------------------------

pub struct AuthContext {
    pub headers: HeaderMap,
    pub config: serde_json::Value,
}

#[async_trait::async_trait]
pub trait AuthPlugin: Send + Sync {
    async fn authenticate(&self, ctx: &mut AuthContext) -> Result<(), PluginError>;
}
