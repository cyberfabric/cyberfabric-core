//! Configuration for the credstore module.

use serde::Deserialize;

/// Module configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CredStoreConfig {
    /// Vendor selector used to pick a plugin implementation.
    ///
    /// The module queries types-registry for plugin instances matching
    /// this vendor and selects the one with lowest priority number.
    pub vendor: String,
}

impl Default for CredStoreConfig {
    fn default() -> Self {
        Self {
            vendor: "hyperspot".to_owned(),
        }
    }
}
