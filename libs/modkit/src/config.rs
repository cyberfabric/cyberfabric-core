//! Configuration module for typed module configuration access.
//!
//! This module provides two distinct mechanisms for loading module configuration:
//!
//! 1. **Lenient loading** (default): Falls back to `T::default()` when configuration is missing.
//!    - Used by `module_config_or_default`
//!    - Allows modules to exist without configuration sections in the main config file
//!
//! 2. **Strict loading**: Requires configuration to be present and valid.
//!    - Used by `module_config_required`
//!    - Returns errors when configuration is missing or invalid

use serde::de::DeserializeOwned;

/// Configuration error for typed config operations
#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("module '{module}' not found")]
    ModuleNotFound { module: String },
    #[error("module '{module}' config must be an object")]
    InvalidModuleStructure { module: String },
    #[error("missing 'config' section in module '{module}'")]
    MissingConfigSection { module: String },
    #[error("invalid config for module '{module}': {source}")]
    InvalidConfig {
        module: String,
        #[source]
        source: serde_json::Error,
    },
}

/// Provider of module-specific configuration (raw JSON sections only).
pub trait ConfigProvider: Send + Sync {
    /// Returns raw JSON section for the module, if any.
    fn get_module_config(&self, module_name: &str) -> Option<&serde_json::Value>;

    /// Returns the full app config for accessing server-level configuration.
    /// This is used by system modules (like `api_gateway`) that need access to server-level settings.
    #[cfg(feature = "bootstrap")]
    fn app_config(&self) -> Option<&crate::bootstrap::AppConfig> {
        None
    }
}

/// Lenient configuration loader that falls back to defaults.
///
/// This function provides forgiving behavior for modules that don't require configuration:
/// - If the module is not present in config → returns `Ok(T::default())`
/// - If the module value is not an object → returns `Ok(T::default())`
/// - If the module has no "config" field → returns `Ok(T::default())`
/// - If "config" is present but invalid → returns `Err(ConfigError::InvalidConfig)`
///
/// Use this for modules that can operate with default configuration.
///
/// # Errors
/// Returns `ConfigError::InvalidConfig` if the config section exists but cannot be deserialized.
pub fn module_config_or_default<T: DeserializeOwned + Default>(
    provider: &dyn ConfigProvider,
    module_name: &str,
) -> Result<T, ConfigError> {
    // If module not found, use defaults
    let Some(module_raw) = provider.get_module_config(module_name) else {
        return Ok(T::default());
    };

    // If module is not an object, use defaults
    let Some(obj) = module_raw.as_object() else {
        return Ok(T::default());
    };

    // If no config section, use defaults
    let Some(config_section) = obj.get("config") else {
        return Ok(T::default());
    };

    // Config section exists, try to parse it
    let config: T =
        serde_json::from_value(config_section.clone()).map_err(|e| ConfigError::InvalidConfig {
            module: module_name.to_owned(),
            source: e,
        })?;

    Ok(config)
}

/// Strict configuration loader that requires configuration to be present.
///
/// This function enforces that configuration must exist and be valid:
/// - If the module is not present → returns `Err(ConfigError::ModuleNotFound)`
/// - If the module value is not an object → returns `Err(ConfigError::InvalidModuleStructure)`
/// - If the module has no "config" field → returns `Err(ConfigError::MissingConfigSection)`
/// - If "config" is present but invalid → returns `Err(ConfigError::InvalidConfig)`
///
/// Use this for modules that cannot operate without explicit configuration.
///
/// # Errors
/// Returns `ConfigError` if the module is not found, has invalid structure, or config is invalid.
pub fn module_config_required<T: DeserializeOwned>(
    provider: &dyn ConfigProvider,
    module_name: &str,
) -> Result<T, ConfigError> {
    let module_raw =
        provider
            .get_module_config(module_name)
            .ok_or_else(|| ConfigError::ModuleNotFound {
                module: module_name.to_owned(),
            })?;

    // Extract config section from: modules.<name> = { database: ..., config: ... }
    let obj = module_raw
        .as_object()
        .ok_or_else(|| ConfigError::InvalidModuleStructure {
            module: module_name.to_owned(),
        })?;

    let config_section = obj
        .get("config")
        .ok_or_else(|| ConfigError::MissingConfigSection {
            module: module_name.to_owned(),
        })?;

    let config: T =
        serde_json::from_value(config_section.clone()).map_err(|e| ConfigError::InvalidConfig {
            module: module_name.to_owned(),
            source: e,
        })?;

    Ok(config)
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;
    use std::collections::HashMap;

    #[derive(Debug, PartialEq, Deserialize, Default)]
    struct TestConfig {
        #[serde(default)]
        api_key: String,
        #[serde(default)]
        timeout_ms: u64,
        #[serde(default)]
        enabled: bool,
    }

    struct MockConfigProvider {
        modules: HashMap<String, serde_json::Value>,
    }

    impl MockConfigProvider {
        fn new() -> Self {
            let mut modules = HashMap::new();

            // Valid module config
            modules.insert(
                "test_module".to_owned(),
                json!({
                    "database": {
                        "url": "postgres://localhost/test"
                    },
                    "config": {
                        "api_key": "secret123",
                        "timeout_ms": 5000,
                        "enabled": true
                    }
                }),
            );

            // Module without config section
            modules.insert(
                "no_config_module".to_owned(),
                json!({
                    "database": {
                        "url": "postgres://localhost/test"
                    }
                }),
            );

            // Module with invalid structure (not an object)
            modules.insert("invalid_module".to_owned(), json!("not an object"));

            Self { modules }
        }
    }

    impl ConfigProvider for MockConfigProvider {
        fn get_module_config(&self, module_name: &str) -> Option<&serde_json::Value> {
            self.modules.get(module_name)
        }
    }

    // ========== Tests for lenient loading (module_config_or_default) ==========

    #[test]
    fn test_lenient_success() {
        let provider = MockConfigProvider::new();
        let result: Result<TestConfig, ConfigError> =
            module_config_or_default(&provider, "test_module");

        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.api_key, "secret123");
        assert_eq!(config.timeout_ms, 5000);
        assert!(config.enabled);
    }

    #[test]
    fn test_lenient_module_not_found_returns_default() {
        let provider = MockConfigProvider::new();
        let result: Result<TestConfig, ConfigError> =
            module_config_or_default(&provider, "nonexistent");

        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config, TestConfig::default());
    }

    #[test]
    fn test_lenient_missing_config_section_returns_default() {
        let provider = MockConfigProvider::new();
        let result: Result<TestConfig, ConfigError> =
            module_config_or_default(&provider, "no_config_module");

        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config, TestConfig::default());
    }

    #[test]
    fn test_lenient_invalid_structure_returns_default() {
        let provider = MockConfigProvider::new();
        let result: Result<TestConfig, ConfigError> =
            module_config_or_default(&provider, "invalid_module");

        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config, TestConfig::default());
    }

    #[test]
    fn test_lenient_invalid_config_returns_error() {
        let mut provider = MockConfigProvider::new();
        // Add module with invalid config structure
        provider.modules.insert(
            "bad_config_module".to_owned(),
            json!({
                "config": {
                    "api_key": "secret123",
                    "timeout_ms": "not_a_number", // Should be u64
                    "enabled": true
                }
            }),
        );

        let result: Result<TestConfig, ConfigError> =
            module_config_or_default(&provider, "bad_config_module");

        assert!(matches!(result, Err(ConfigError::InvalidConfig { .. })));
        if let Err(ConfigError::InvalidConfig { module, .. }) = result {
            assert_eq!(module, "bad_config_module");
        }
    }

    #[test]
    fn test_lenient_helper_with_multiple_scenarios() {
        let provider = MockConfigProvider::new();

        // Module not found should return default
        let result: Result<TestConfig, ConfigError> =
            module_config_or_default(&provider, "nonexistent");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), TestConfig::default());

        // Valid config should parse correctly
        let result: Result<TestConfig, ConfigError> =
            module_config_or_default(&provider, "test_module");
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.api_key, "secret123");
    }

    // ========== Tests for strict loading (module_config_required) ==========

    #[test]
    fn test_strict_success() {
        let provider = MockConfigProvider::new();
        let result: Result<TestConfig, ConfigError> =
            module_config_required(&provider, "test_module");

        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.api_key, "secret123");
        assert_eq!(config.timeout_ms, 5000);
        assert!(config.enabled);
    }

    #[test]
    fn test_strict_module_not_found() {
        let provider = MockConfigProvider::new();
        let result: Result<TestConfig, ConfigError> =
            module_config_required(&provider, "nonexistent");

        assert!(matches!(result, Err(ConfigError::ModuleNotFound { .. })));
        if let Err(ConfigError::ModuleNotFound { module }) = result {
            assert_eq!(module, "nonexistent");
        }
    }

    #[test]
    fn test_strict_missing_config_section() {
        let provider = MockConfigProvider::new();
        let result: Result<TestConfig, ConfigError> =
            module_config_required(&provider, "no_config_module");

        assert!(matches!(
            result,
            Err(ConfigError::MissingConfigSection { .. })
        ));
        if let Err(ConfigError::MissingConfigSection { module }) = result {
            assert_eq!(module, "no_config_module");
        }
    }

    #[test]
    fn test_strict_invalid_structure() {
        let provider = MockConfigProvider::new();
        let result: Result<TestConfig, ConfigError> =
            module_config_required(&provider, "invalid_module");

        assert!(matches!(
            result,
            Err(ConfigError::InvalidModuleStructure { .. })
        ));
        if let Err(ConfigError::InvalidModuleStructure { module }) = result {
            assert_eq!(module, "invalid_module");
        }
    }

    #[test]
    fn test_strict_invalid_config() {
        let mut provider = MockConfigProvider::new();
        // Add module with invalid config structure
        provider.modules.insert(
            "bad_config_module".to_owned(),
            json!({
                "config": {
                    "api_key": "secret123",
                    "timeout_ms": "not_a_number", // Should be u64
                    "enabled": true
                }
            }),
        );

        let result: Result<TestConfig, ConfigError> =
            module_config_required(&provider, "bad_config_module");

        assert!(matches!(result, Err(ConfigError::InvalidConfig { .. })));
        if let Err(ConfigError::InvalidConfig { module, .. }) = result {
            assert_eq!(module, "bad_config_module");
        }
    }

    // ========== Tests for ConfigError display messages ==========

    #[test]
    fn test_config_error_messages() {
        let module_not_found = ConfigError::ModuleNotFound {
            module: "test".to_owned(),
        };
        assert_eq!(module_not_found.to_string(), "module 'test' not found");

        let invalid_structure = ConfigError::InvalidModuleStructure {
            module: "test".to_owned(),
        };
        assert_eq!(
            invalid_structure.to_string(),
            "module 'test' config must be an object"
        );

        let missing_config = ConfigError::MissingConfigSection {
            module: "test".to_owned(),
        };
        assert_eq!(
            missing_config.to_string(),
            "missing 'config' section in module 'test'"
        );
    }
}
