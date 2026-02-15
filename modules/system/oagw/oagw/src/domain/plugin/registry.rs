use std::collections::HashMap;
use std::sync::Arc;

use crate::domain::credential::CredentialResolver;
use crate::domain::plugin::{AuthPlugin, PluginError};

use crate::infra::plugin::apikey_auth::{APIKEY_AUTH_PLUGIN_ID, ApiKeyAuthPlugin};
use crate::infra::plugin::noop_auth::{NOOP_AUTH_PLUGIN_ID, NoopAuthPlugin};

/// Registry that resolves auth plugin GTS identifiers to plugin implementations.
pub struct AuthPluginRegistry {
    plugins: HashMap<String, Arc<dyn AuthPlugin>>,
}

impl AuthPluginRegistry {
    /// Create a registry with the built-in plugins (apikey, noop).
    #[must_use]
    pub fn with_builtins(credential_resolver: Arc<dyn CredentialResolver>) -> Self {
        let mut plugins: HashMap<String, Arc<dyn AuthPlugin>> = HashMap::new();
        plugins.insert(
            APIKEY_AUTH_PLUGIN_ID.to_string(),
            Arc::new(ApiKeyAuthPlugin::new(credential_resolver)),
        );
        plugins.insert(NOOP_AUTH_PLUGIN_ID.to_string(), Arc::new(NoopAuthPlugin));
        Self { plugins }
    }

    /// Resolve a plugin by its GTS identifier.
    ///
    /// # Errors
    /// Returns `PluginError::Internal` if the plugin is not registered.
    pub fn resolve(&self, plugin_id: &str) -> Result<Arc<dyn AuthPlugin>, PluginError> {
        self.plugins
            .get(plugin_id)
            .cloned()
            .ok_or_else(|| PluginError::Internal(format!("unknown auth plugin: {plugin_id}")))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::infra::storage::credential_repo::InMemoryCredentialResolver;

    use super::*;

    fn make_registry() -> AuthPluginRegistry {
        let creds = Arc::new(InMemoryCredentialResolver::new());
        AuthPluginRegistry::with_builtins(creds)
    }

    #[test]
    fn resolves_apikey_plugin() {
        let registry = make_registry();
        assert!(registry.resolve(APIKEY_AUTH_PLUGIN_ID).is_ok());
    }

    #[test]
    fn resolves_noop_plugin() {
        let registry = make_registry();
        assert!(registry.resolve(NOOP_AUTH_PLUGIN_ID).is_ok());
    }

    #[test]
    fn unknown_plugin_returns_error() {
        let registry = make_registry();
        let err = registry.resolve("gts.x.core.oagw.auth_plugin.v1~x.core.oagw.unknown.v1");
        assert!(err.is_err());
    }
}
