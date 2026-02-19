//! Out-of-process module bootstrap library
//!
//! This module provides reusable functionality for bootstrapping `OoP` (out-of-process)
//! modkit modules in local (non-k8s) environments.
//!
//! ## Features
//!
//! - Configuration loading using `modkit-bootstrap`
//! - Logging initialization with tracing
//! - gRPC connection to `DirectoryService`
//! - Module instance registration
//! - Heartbeat management
//! - Module lifecycle execution
//!
//! ## Shutdown Model
//!
//! Shutdown is driven by a single root `CancellationToken` per process:
//! - OS signals (SIGTERM, SIGINT, Ctrl+C) are hooked at bootstrap level
//! - The root token is passed to `RunOptions::Token` for module runtime shutdown
//! - Background tasks (like heartbeat) use child tokens derived from the root
//!
//! On shutdown, the module deregisters itself from the `DirectoryService` before exiting.
//!
//! ## Example
//!
//! ```rust,no_run
//! use modkit::bootstrap::oop::{OopRunOptions, run_oop_with_options};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let opts = OopRunOptions {
//!         module_name: "my_module".to_string(),
//!         instance_id: None,
//!         directory_endpoint: "http://127.0.0.1:50051".to_string(),
//!         config_path: None,
//!         verbose: 0,
//!         print_config: false,
//!         heartbeat_interval_secs: 5,
//!     };
//!
//!     run_oop_with_options(opts).await
//! }
//! ```

use anyhow::{Context, Result};
use figment::{Figment, providers::Serialized};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::config::{
    AppConfig, CliArgs, LoggingConfig, MODKIT_MODULE_CONFIG_ENV, RenderedDbConfig,
    RenderedModuleConfig,
};
use crate::bootstrap::host::init_logging_unified;
use crate::runtime::{
    ClientRegistration, DbOptions, MODKIT_DIRECTORY_ENDPOINT_ENV, RunOptions, ShutdownOptions, run,
    shutdown,
};
use cf_system_sdks::directory::{DirectoryClient, DirectoryGrpcClient};

/// Configuration options for `OoP` module bootstrap
#[derive(Debug, Clone)]
pub struct OopRunOptions {
    /// Logical module name (e.g., "`file-parser`")
    pub module_name: String,

    /// Instance ID (defaults to a random UUID if None)
    pub instance_id: Option<Uuid>,

    /// Directory service gRPC endpoint (e.g., "<http://127.0.0.1:50051>")
    pub directory_endpoint: String,

    /// Path to configuration file
    pub config_path: Option<PathBuf>,

    /// Log verbosity level (0=default, 1=info, 2=debug, 3=trace)
    pub verbose: u8,

    /// Print effective configuration and exit
    pub print_config: bool,

    /// Heartbeat interval in seconds (default: 5)
    pub heartbeat_interval_secs: u64,
}

impl Default for OopRunOptions {
    fn default() -> Self {
        // Check for config path in environment variable as fallback
        let config_path = std::env::var("MODKIT_CONFIG_PATH").ok().map(PathBuf::from);

        // Check for directory endpoint in environment variable (set by master host)
        // This is the preferred way to get the endpoint when spawned by master host
        let directory_endpoint = std::env::var(MODKIT_DIRECTORY_ENDPOINT_ENV)
            .unwrap_or_else(|_| "http://127.0.0.1:50051".to_owned());

        Self {
            module_name: String::new(),
            instance_id: None,
            directory_endpoint,
            config_path,
            verbose: 0,
            print_config: false,
            heartbeat_interval_secs: 5,
        }
    }
}

/// Builds the final configuration and `DbOptions` for an `OoP` module.
///
/// Configuration merge strategy (for each section):
/// - **Database**: field-by-field merge using `DbManager` (master as base, local as override)
/// - **Logging**: key-by-key merge (each subsystem key is overridden by local)
/// - **Config**: local completely replaces master if present
///
/// The local config file (--config) can override any settings from master's `MODKIT_MODULE_CONFIG`.
///
/// For database, the merge happens at 3 levels:
/// 1. Global database.servers.* from master
/// 2. Module's database section from master (modules.<name>.database)
/// 3. Module's database section from local --config (overrides master)
#[tracing::instrument(
    level = "debug",
    skip(local_config, rendered_config),
    fields(
        has_rendered = rendered_config.is_some(),
        has_local_db = local_config.database.is_some()
    )
)]
fn build_oop_config_and_db(
    local_config: &AppConfig,
    module_name: &str,
    rendered_config: Option<&RenderedModuleConfig>,
) -> Result<(AppConfig, LoggingConfig, DbOptions)> {
    let home_dir = PathBuf::from(&local_config.server.home_dir);

    // Build final_config for module's "config" section
    let final_config = if let Some(rendered) = rendered_config {
        // MODKIT_MODULE_CONFIG exists: use rendered config as BASE, local config as OVERRIDE
        let mut config = local_config.clone();

        // Get or create the module entry
        let module_entry = config
            .modules
            .entry(module_name.to_owned())
            .or_insert_with(|| serde_json::json!({}));

        // Merge rendered.config as base, local module config as override
        if let Some(obj) = module_entry.as_object_mut() {
            // If local doesn't have "config" section, use rendered entirely
            // If local has "config" section, it takes precedence (local overrides master)
            if !obj.contains_key("config") || obj["config"].is_null() {
                obj.insert("config".to_owned(), rendered.config.clone());
            }
            // If local has "config", it already overrides - no action needed
        }

        debug!(
            module = %module_name,
            has_rendered_db = %rendered.database.is_some(),
            has_rendered_logging = %rendered.logging.is_some(),
            "Using rendered config from master as base, local config as override"
        );

        config
    } else {
        // No MODKIT_MODULE_CONFIG: use local config entirely (standalone mode)
        debug!(
            module = %module_name,
            "No rendered config from master, using local config entirely (standalone mode)"
        );
        local_config.clone()
    };

    // Merge logging: master logging (base) + local logging (override by key)
    let final_logging = merge_logging_configs(
        rendered_config.as_ref().and_then(|r| r.logging.as_ref()),
        &local_config.logging,
    );

    // Build DbOptions using Figment merge + DbManager
    // This allows field-by-field merge: master db config (base) -> local db config (override)
    let db_options = build_merged_db_options(
        &home_dir,
        module_name,
        rendered_config.as_ref().and_then(|r| r.database.as_ref()),
        local_config,
    )?;

    Ok((final_config, final_logging, db_options))
}

/// Merges logging configurations: master as base, local as override (by key).
///
/// Each key in the logging `HashMap` (e.g., "default", "calculator", "sqlx")
/// is overridden by local if present.
fn merge_logging_configs(master: Option<&LoggingConfig>, local: &LoggingConfig) -> LoggingConfig {
    master
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .chain(local.clone())
        .collect()
}

/// Builds `DbOptions` by merging rendered config from master with local config.
///
/// Uses Figment to merge configurations and `DbManager` to handle the actual
/// database connection setup with field-by-field merge logic.
fn build_merged_db_options(
    home_dir: &Path,
    module_name: &str,
    rendered_db: Option<&RenderedDbConfig>,
    local_config: &AppConfig,
) -> Result<DbOptions> {
    // Check if we have any database configuration
    let has_rendered_db = rendered_db.is_some_and(|db| db.module.is_some() || db.global.is_some());
    let has_local_db = local_config.database.is_some()
        || local_config
            .modules
            .get(module_name)
            .and_then(|m| m.get("database"))
            .is_some();

    if !has_rendered_db && !has_local_db {
        debug!(
            module = %module_name,
            "No database config available"
        );
        return Ok(DbOptions::None);
    }

    // Build a merged configuration for DbManager:
    // 1. Start with rendered config from master (global servers + module db)
    // 2. Overlay local config (local can override any field)

    let mut merged_config = serde_json::Map::new();

    // Step 1: Add rendered database config from master as base
    if let Some(rendered) = rendered_db {
        // Add global servers from master
        if let Some(ref global) = rendered.global {
            let global_json = serde_json::to_value(global)
                .context("Failed to serialize rendered global db config")?;
            merged_config.insert("database".to_owned(), global_json);
        }

        // Add module's database config from master
        if let Some(ref module_db) = rendered.module {
            let module_db_json = serde_json::to_value(module_db)
                .context("Failed to serialize rendered module db config")?;

            let mut modules = serde_json::Map::new();
            let mut module_entry = serde_json::Map::new();
            module_entry.insert("database".to_owned(), module_db_json);
            modules.insert(
                module_name.to_owned(),
                serde_json::Value::Object(module_entry),
            );
            merged_config.insert("modules".to_owned(), serde_json::Value::Object(modules));
        }
    }

    // Step 2: Overlay local config (local overrides master)
    // Local global database config
    if let Some(ref local_db) = local_config.database {
        let local_db_json =
            serde_json::to_value(local_db).context("Failed to serialize local global db config")?;

        // Merge with existing or replace
        if let Some(existing) = merged_config.get_mut("database") {
            merge_json_objects(existing, &local_db_json);
        } else {
            merged_config.insert("database".to_owned(), local_db_json);
        }
    }

    // Local module database config
    if let Some(local_module) = local_config.modules.get(module_name)
        && let Some(local_module_db) = local_module.get("database")
    {
        let modules = merged_config
            .entry("modules".to_owned())
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));

        if let Some(modules_obj) = modules.as_object_mut() {
            let module_entry = modules_obj
                .entry(module_name.to_owned())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));

            if let Some(module_obj) = module_entry.as_object_mut() {
                if let Some(existing_db) = module_obj.get_mut("database") {
                    merge_json_objects(existing_db, local_module_db);
                } else {
                    module_obj.insert("database".to_owned(), local_module_db.clone());
                }
            }
        }
    }

    debug!(
        module = %module_name,
        has_rendered = %rendered_db.is_some(),
        has_local_global = %local_config.database.is_some(),
        "Building DbManager with merged config"
    );

    // Create DbManager from merged Figment
    let figment = Figment::new().merge(Serialized::defaults(serde_json::Value::Object(
        merged_config,
    )));
    let db_manager = Arc::new(
        modkit_db::DbManager::from_figment(figment, home_dir.to_path_buf())
            .context("Failed to create DbManager from merged config")?,
    );

    Ok(DbOptions::Manager(db_manager))
}

/// Recursively merges source JSON object into target.
/// Source values override target values for matching keys.
fn merge_json_objects(target: &mut serde_json::Value, source: &serde_json::Value) {
    if let (Some(target_obj), Some(source_obj)) = (target.as_object_mut(), source.as_object()) {
        for (key, value) in source_obj {
            if let Some(target_value) = target_obj.get_mut(key) {
                // Recursively merge objects, otherwise replace
                if target_value.is_object() && value.is_object() {
                    merge_json_objects(target_value, value);
                } else {
                    *target_value = value.clone();
                }
            } else {
                target_obj.insert(key.clone(), value.clone());
            }
        }
    } else {
        // If target is not an object, replace entirely
        *target = source.clone();
    }
}

/// Run an out-of-process module with the given options
///
/// This function:
/// 1. Creates a root `CancellationToken` for the process
/// 2. Hooks OS signals (SIGTERM, SIGINT, Ctrl+C) to trigger cancellation
/// 3. Loads configuration and initializes logging
/// 4. Connects to the `DirectoryService`
/// 5. Registers the module instance
/// 6. Starts a background heartbeat loop (using a child token)
/// 7. Runs the module lifecycle with `ShutdownOptions::Token`
/// 8. Deregisters from `DirectoryService` on shutdown
///
/// ## Shutdown Model
///
/// A single root cancellation token drives shutdown for the entire process.
/// OS signals are hooked at this bootstrap level (not via `ShutdownOptions::Signals`).
/// The heartbeat loop and module runtime both observe this token tree.
///
/// # Arguments
///
/// * `opts` - Bootstrap configuration options
///
/// # Returns
///
/// * `Ok(())` - If the module lifecycle completed successfully
/// * `Err(e)` - If any step failed
///
/// # Example
///
/// ```rust,no_run
/// use modkit::bootstrap::oop::{OopRunOptions, run_oop_with_options};
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let opts = OopRunOptions {
///         module_name: "file-parser".to_string(),
///         instance_id: None,
///         directory_endpoint: "http://127.0.0.1:50051".to_string(),
///         config_path: None,
///         verbose: 1,
///         print_config: false,
///         heartbeat_interval_secs: 5,
///     };
///
///     run_oop_with_options(opts).await
/// }
/// ```
///
/// # Errors
/// Returns an error if the `OoP` module fails to start or run.
#[tracing::instrument(
    level = "info",
    name = "oop_bootstrap",
    skip(opts),
    fields(
        module = %opts.module_name,
        directory = %opts.directory_endpoint
    )
)]
pub async fn run_oop_with_options(opts: OopRunOptions) -> Result<()> {
    // Generate instance ID if not provided
    let instance_id = opts.instance_id.unwrap_or_else(Uuid::new_v4);

    // Create root cancellation token for the entire process.
    // This token drives shutdown for the module runtime and all background tasks.
    let cancel = CancellationToken::new();

    // Hook OS signals to the root token at bootstrap level.
    // This replaces the use of ShutdownOptions::Signals inside the runtime.
    let cancel_for_signals = cancel.clone();
    tokio::spawn(async move {
        match shutdown::wait_for_shutdown().await {
            Ok(()) => {
                info!(target: "", "------------------");
                info!("shutdown: signal received in OoP bootstrap");
            }
            Err(e) => {
                warn!(
                    error = %e,
                    "shutdown: primary waiter failed in OoP bootstrap, falling back to ctrl_c()"
                );
                _ = tokio::signal::ctrl_c().await;
            }
        }
        cancel_for_signals.cancel();
    });

    // Prepare CLI args for AppConfig loading
    let args = CliArgs {
        config: opts
            .config_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string()),
        print_config: opts.print_config,
        verbose: opts.verbose,
        mock: false,
    };

    // Load configuration
    let mut config = AppConfig::load_or_default(&opts.config_path)?;
    config.apply_cli_overrides(args.verbose);

    // Try to read rendered module config from master host via env var BEFORE logging init
    // so we can use the tracing config from master for OTEL
    let rendered_config = match std::env::var(MODKIT_MODULE_CONFIG_ENV) {
        Ok(json) => RenderedModuleConfig::from_json(&json).ok(),
        Err(_) => None,
    };

    // Build final config by merging:
    // 1. Rendered config from master host (base)
    // 2. Local config file (override)
    // This also merges logging configuration for proper initialization
    let (final_config, merged_logging, db_options) =
        build_oop_config_and_db(&config, &opts.module_name, rendered_config.as_ref())?;

    // Initialize OTEL layer from rendered config's tracing settings (if available)
    // OoP modules use master's tracing config, not their own local config
    #[cfg(feature = "otel")]
    let otel_layer = rendered_config
        .as_ref()
        .and_then(|rc| rc.tracing.as_ref())
        .map(crate::telemetry::init_tracing)
        .transpose()?;
    #[cfg(not(feature = "otel"))]
    let otel_layer = None;

    // Build tokio-console layer for OoP module (when feature is enabled)
    let console_layer =
        super::host::observability::build_console_layer(config.tokio_console.as_ref());

    // Initialize logging with MERGED config (master base + local override)
    init_logging_unified(
        &merged_logging,
        &config.server.home_dir,
        otel_layer,
        console_layer,
    );

    // Now we can log - report what we received from master
    if let Some(ref rc) = rendered_config {
        info!(
            env_var = MODKIT_MODULE_CONFIG_ENV,
            has_database = rc.database.is_some(),
            has_config = !rc.config.is_null(),
            has_logging = rc.logging.is_some(),
            has_tracing = rc.tracing.is_some(),
            "Received rendered config from master host"
        );
    } else if std::env::var(MODKIT_MODULE_CONFIG_ENV).is_ok() {
        warn!(
            env_var = MODKIT_MODULE_CONFIG_ENV,
            "Failed to parse rendered config from master host, using local config only"
        );
    } else {
        debug!(
            env_var = MODKIT_MODULE_CONFIG_ENV,
            "No rendered config from master host, using local config only"
        );
    }

    info!(
        module = %opts.module_name,
        instance_id = %instance_id,
        directory_endpoint = %opts.directory_endpoint,
        "OoP module bootstrap starting"
    );

    // Print config and exit if requested
    if opts.print_config {
        print_config(&config);
        return Ok(());
    }

    // Connect to DirectoryService
    info!(
        "Connecting to directory service at {}",
        opts.directory_endpoint
    );
    let directory_client = DirectoryGrpcClient::connect(&opts.directory_endpoint).await?;
    let directory_api: Arc<dyn DirectoryClient> = Arc::new(directory_client);

    info!("Successfully connected to directory service");

    // Start heartbeat loop in background using a child token from the root.
    // This allows the heartbeat to be cancelled when the root token is cancelled.
    let heartbeat_directory = Arc::clone(&directory_api);
    let heartbeat_module = opts.module_name.clone();
    let heartbeat_instance_id_str = instance_id.to_string();
    let heartbeat_interval = Duration::from_secs(opts.heartbeat_interval_secs);
    let heartbeat_cancel = cancel.child_token();

    tokio::spawn(async move {
        info!(
            interval_secs = opts.heartbeat_interval_secs,
            "Starting heartbeat loop"
        );

        loop {
            tokio::select! {
                () = heartbeat_cancel.cancelled() => {
                    info!("Heartbeat loop stopping due to cancellation");
                    break;
                }
                () = sleep(heartbeat_interval) => {
                    match heartbeat_directory
                        .send_heartbeat(&heartbeat_module, &heartbeat_instance_id_str)
                        .await
                    {
                        Ok(()) => {
                            tracing::debug!("Heartbeat sent successfully");
                        }
                        Err(e) => {
                            warn!(error = %e, "Failed to send heartbeat, will retry");
                        }
                    }
                }
            }
        }
    });

    // Build config provider for modules
    let config_provider = Arc::new(final_config);

    // Keep a reference to directory_api for deregistration after shutdown
    // Run the module lifecycle with the root cancellation token.
    // Shutdown is driven by the signal handler spawned above, not by ShutdownOptions::Signals.
    // The DirectoryClient (gRPC client) is injected into the ClientHub so modules can access it.
    info!("Starting module lifecycle");
    let run_options = RunOptions {
        modules_cfg: config_provider,
        db: db_options,
        shutdown: ShutdownOptions::Token(cancel.clone()),
        clients: vec![ClientRegistration::new::<dyn DirectoryClient>(
            directory_api,
        )],
        instance_id,
        oop: None, // OoP modules don't spawn other OoP modules
    };

    let result = run(run_options).await;

    if let Err(ref e) = result {
        error!(error = %e, "Module runtime failed");
    } else {
        info!("Module runtime completed successfully");
    }

    result
}

#[allow(unknown_lints, de1301_no_print_macros)] // direct stdout config print before exit
fn print_config(config: &AppConfig) {
    match config.to_yaml() {
        Ok(yaml) => {
            println!("{yaml}");
        }
        Err(e) => {
            eprintln!("Failed to render config as YAML: {e}");
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
#[path = "oop_tests.rs"]
mod tests;
