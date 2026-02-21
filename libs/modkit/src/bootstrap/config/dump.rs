//! Effective module configuration dump support.
//!
//! This module provides utilities for inspecting and dumping the effective
//! runtime configuration of modules, including resolved database DSNs and pool settings.

use super::{AppConfig, RuntimeKind, build_final_db_for_module, parse_module_config};
use anyhow::{Context, Result};
use std::path::PathBuf;
use url::Url;

/// List all module names present in the configuration.
///
/// Returns a sorted vector of module names that are configured in the `AppConfig`.
/// This is useful for discovering available modules before dumping their configuration.
///
/// # Example
/// ```no_run
/// use modkit::bootstrap::AppConfig;
/// # fn example(config: &AppConfig) {
/// let modules = modkit::bootstrap::config::list_module_names(config);
/// for module in modules {
///     println!("Module: {}", module);
/// }
/// # }
/// ```
#[must_use]
pub fn list_module_names(app: &AppConfig) -> Vec<String> {
    let mut names: Vec<String> = app.modules.keys().cloned().collect();
    names.sort();
    names
}

/// Render effective configuration for all loaded modules.
///
/// This function builds a complete view of the effective runtime configuration
/// for each module that is successfully loaded in the `ModuleRegistry`.
///
/// For each module, it includes:
/// - `runtime`: Module runtime type (local/oop) if configured
/// - `config`: Module-specific configuration section (as-is from config file)
/// - `database`: Final resolved database configuration with redacted DSN (if applicable)
///
/// This is a read-only inspection operation that does not create any directories
/// or modify the filesystem.
///
/// Modules with configuration errors are logged as warnings and skipped,
/// allowing inspection of valid modules even when some are misconfigured.
///
/// # Errors
/// This function does not return errors in practice - all module-level failures
/// are logged as warnings and the problematic modules are skipped. The `Result`
/// return type is kept for API consistency with other dump functions.
pub fn render_effective_modules_config(app: &AppConfig) -> Result<serde_json::Value> {
    use serde_json::json;

    let home_dir = PathBuf::from(&app.server.home_dir);
    // Prevent path traversal attacks by rejecting paths containing '..'
    if home_dir.components().any(|c| c == std::path::Component::ParentDir) {
        return Err(anyhow::anyhow!("Invalid input: {}", home_dir.display()));
    }
    let mut modules_config = serde_json::Map::new();

    // Iterate over all modules in the configuration
    for module_name in app.modules.keys() {
        let mut module_entry = serde_json::Map::new();

        // Parse module config once for efficiency
        let parsed_config = match parse_module_config(app, module_name) {
            Ok(config) => config,
            Err(e) => {
                tracing::warn!(
                    module = %module_name,
                    error = %e,
                    "Failed to parse module config, skipping"
                );
                continue;
            }
        };

        // Get runtime configuration if present
        if let Some(runtime_config) = parsed_config.runtime {
            module_entry.insert(
                "runtime".to_owned(),
                json!({
                    "type": match runtime_config.mod_type {
                        RuntimeKind::Local => "local",
                        RuntimeKind::Oop => "oop",
                    }
                }),
            );
        }

        // Get module config section (the "config" field)
        if !parsed_config.config.is_null() {
            module_entry.insert("config".to_owned(), parsed_config.config);
        }

        // Get database configuration (resolved DSN + pool) - use dry_run=true
        match build_final_db_for_module(app, module_name, &home_dir, true) {
            Ok(Some((dsn, pool))) => {
                // Redact password in DSN (warn and skip DB section if this fails, but keep module)
                let redacted_dsn = match redact_dsn_password(&dsn) {
                    Ok(redacted) => redacted,
                    Err(e) => {
                        tracing::warn!(
                            module = %module_name,
                            error = %e,
                            "Failed to redact DSN password, skipping database config for this module"
                        );
                        // Continue processing this module, just skip the DB section
                        // Add module entry even without DB config
                        if !module_entry.is_empty() {
                            modules_config.insert(module_name.clone(), json!(module_entry));
                        }
                        continue;
                    }
                };

                let mut db_config = serde_json::Map::new();
                db_config.insert("dsn".to_owned(), json!(redacted_dsn));

                // Add pool configuration if present
                let mut pool_map = serde_json::Map::new();
                if let Some(max_conns) = pool.max_conns {
                    pool_map.insert("max_conns".to_owned(), json!(max_conns));
                }
                if let Some(min_conns) = pool.min_conns {
                    pool_map.insert("min_conns".to_owned(), json!(min_conns));
                }
                if let Some(acquire_timeout) = pool.acquire_timeout {
                    pool_map.insert(
                        "acquire_timeout".to_owned(),
                        json!(format!("{}s", acquire_timeout.as_secs())),
                    );
                }
                if let Some(idle_timeout) = pool.idle_timeout {
                    pool_map.insert(
                        "idle_timeout".to_owned(),
                        json!(format!("{}s", idle_timeout.as_secs())),
                    );
                }
                if let Some(max_lifetime) = pool.max_lifetime {
                    pool_map.insert(
                        "max_lifetime".to_owned(),
                        json!(format!("{}s", max_lifetime.as_secs())),
                    );
                }
                if let Some(test_before_acquire) = pool.test_before_acquire {
                    pool_map.insert("test_before_acquire".to_owned(), json!(test_before_acquire));
                }

                if !pool_map.is_empty() {
                    db_config.insert("pool".to_owned(), json!(pool_map));
                }

                module_entry.insert("database".to_owned(), json!(db_config));
            }
            Ok(None) => {
                // Module has no database config, skip
            }
            Err(e) => {
                tracing::warn!(
                    module = %module_name,
                    error = %e,
                    "Failed to build database config, skipping"
                );
            }
        }

        // Only add module to output if it has any configuration
        if !module_entry.is_empty() {
            modules_config.insert(module_name.clone(), json!(module_entry));
        }
    }

    Ok(json!(modules_config))
}

/// Redacts password from a DSN for safe logging.
///
/// Replaces the password portion with `***REDACTED***` while preserving the rest of the DSN.
///
/// # Errors
/// Returns an error if DSN parsing fails.
pub fn redact_dsn_password(dsn: &str) -> Result<String> {
    if dsn.contains('@') {
        let parsed = Url::parse(dsn)?;
        let mut redacted_url = parsed;
        if redacted_url.password().is_some() {
            redacted_url.set_password(Some("***REDACTED***")).ok();
        }
        Ok(redacted_url.to_string())
    } else {
        Ok(dsn.to_owned())
    }
}

/// Dump effective modules configuration as YAML string.
///
/// This function renders the effective configuration for all modules and
/// serializes it to a human-readable YAML format.
///
/// # Errors
/// Returns an error if configuration rendering or YAML serialization fails.
pub fn dump_effective_modules_config_yaml(app: &AppConfig) -> Result<String> {
    let config = render_effective_modules_config(app)?;
    let yaml = serde_saphyr::to_string(&config)
        .context("Failed to serialize modules configuration to YAML")?;
    Ok(yaml)
}

/// Dump effective modules configuration as JSON string.
///
/// This function renders the effective configuration for all modules and
/// serializes it to a pretty-printed JSON format.
///
/// # Errors
/// Returns an error if configuration rendering or JSON serialization fails.
pub fn dump_effective_modules_config_json(app: &AppConfig) -> Result<String> {
    let config = render_effective_modules_config(app)?;
    let json = serde_json::to_string_pretty(&config)
        .context("Failed to serialize modules configuration to JSON")?;
    Ok(json)
}
