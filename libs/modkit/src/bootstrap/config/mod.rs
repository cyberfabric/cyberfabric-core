//! Configuration module for modkit-bootstrap
//!
//! This module provides configuration types and utilities for both host and `OoP` modules.

mod dump;

use anyhow::{Context, Result, ensure};
// Use DB config types from modkit-db
pub use modkit_db::{DbConnConfig, GlobalDatabaseConfig, PoolCfg};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::Level;

use crate::ConfigProvider;
use crate::telemetry::TracingConfig;
use url::Url;

// Re-export dump functions
pub use dump::{
    dump_effective_modules_config_json, dump_effective_modules_config_yaml, list_module_names,
    redact_dsn_password, render_effective_modules_config,
};

/// Small typed view to parse each module entry.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleConfig {
    #[serde(default)]
    pub database: Option<DbConnConfig>,
    #[serde(default)]
    pub config: serde_json::Value,
    #[serde(default)]
    pub runtime: Option<ModuleRuntime>,
    #[serde(default)] // Used by the CLI
    pub metadata: serde_json::Value,
}

/// Runtime configuration for a module (local vs out-of-process).
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ModuleRuntime {
    #[serde(default, rename = "type")]
    pub mod_type: RuntimeKind,
    /// Execution configuration for `OoP` modules.
    #[serde(default)]
    pub execution: Option<ExecutionConfig>,
}

/// Execution configuration for out-of-process modules.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ExecutionConfig {
    /// Path to the executable. Supports absolute paths or `~` expansion.
    pub executable_path: String,
    /// Command-line arguments to pass to the executable.
    #[serde(default)]
    pub args: Vec<String>,
    /// Working directory for the process (optional, defaults to current dir).
    #[serde(default)]
    pub working_directory: Option<String>,
    /// Environment variables to set for the process.
    #[serde(default)]
    pub environment: HashMap<String, String>,
}

/// Module runtime kind.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeKind {
    #[default]
    Local,
    Oop,
}

/// Main application configuration with strongly-typed global sections
/// and a flexible per-module configuration bag.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    /// Core server configuration.
    pub server: ServerConfig,
    /// New typed database configuration (optional).
    pub database: Option<GlobalDatabaseConfig>,
    /// Logging configuration (optional, uses defaults if None).
    pub logging: Option<LoggingConfig>,
    /// Tracing configuration (optional, disabled if None).
    pub tracing: Option<TracingConfig>,
    /// Directory containing per-module YAML files (optional).
    #[serde(default)]
    pub modules_dir: Option<String>,
    /// Per-module configuration bag: `module_name` → arbitrary JSON/YAML value.
    #[serde(default)]
    pub modules: HashMap<String, serde_json::Value>,
}

impl ConfigProvider for AppConfig {
    fn get_module_config(&self, module_name: &str) -> Option<&serde_json::Value> {
        self.modules.get(module_name)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    pub home_dir: PathBuf, // will be normalized to absolute path
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            home_dir: super::host::paths::default_home_dir().join(".hyperspot"),
        }
    }
}

impl ServerConfig {
    fn normalize_home_dir_inplace(&mut self) -> Result<()> {
        self.home_dir = super::host::normalize_path(
            self.home_dir
                .to_str()
                .context("home directory configuration is not a valid path")?,
        )
        .context("home_dir normalization failed")?;

        std::fs::create_dir_all(&self.home_dir).context("Failed to create home_dir")?;

        Ok(())
    }
}

/// Logging configuration - maps subsystem names to their logging settings.
/// Key "default" is the catch-all for logs that don't match explicit subsystems.
pub type LoggingConfig = HashMap<String, Section>;

// ================= Custom serde module for optional Level (supports "off") =================
mod optional_level_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use tracing::Level;

    #[allow(clippy::ref_option, clippy::trivially_copy_pass_by_ref)]
    pub fn serialize<S>(level: &Option<Level>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match level {
            Some(l) => serializer.serialize_str(l.as_str()),
            None => serializer.serialize_str("off"),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Level>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "trace" => Ok(Some(Level::TRACE)),
            "debug" => Ok(Some(Level::DEBUG)),
            "info" => Ok(Some(Level::INFO)),
            "warn" => Ok(Some(Level::WARN)),
            "error" => Ok(Some(Level::ERROR)),
            "off" | "none" => Ok(None),
            _ => Err(serde::de::Error::custom(format!("invalid level: {s}"))),
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    pub fn default() -> Option<Level> {
        Some(Level::INFO)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Section {
    #[serde(
        default = "optional_level_serde::default",
        with = "optional_level_serde"
    )]
    pub console_level: Option<Level>,
    pub file: String, // "logs/api.log"
    #[serde(
        default = "optional_level_serde::default",
        with = "optional_level_serde"
    )]
    pub file_level: Option<Level>,
    pub max_age_days: Option<u32>, // Not implemented yet
    #[serde(default)]
    pub max_backups: Option<usize>, // How many files to keep
    #[serde(default)]
    pub max_size_mb: Option<u64>, // Max size of the file in MB
}

/// Create a default logging configuration.
#[must_use]
pub fn default_logging_config() -> LoggingConfig {
    let mut logging = HashMap::new();
    logging.insert(
        "default".to_owned(),
        Section {
            console_level: Some(Level::INFO),
            file: "logs/hyperspot.log".to_owned(),
            file_level: Some(Level::DEBUG),
            max_age_days: Some(7),
            max_backups: Some(3),
            max_size_mb: Some(100),
        },
    );
    logging
}

impl Default for AppConfig {
    fn default() -> Self {
        let server = ServerConfig::default();
        Self {
            server,
            database: Some(GlobalDatabaseConfig {
                servers: HashMap::new(),
                auto_provision: None,
            }),
            logging: Some(default_logging_config()),
            tracing: None, // Disabled by default
            modules_dir: None,
            modules: HashMap::new(),
        }
    }
}

impl AppConfig {
    /// Load configuration with layered loading: defaults → YAML file → environment variables.
    /// Also normalizes `server.home_dir` into an absolute path and creates the directory.
    ///
    /// # Errors
    /// Returns an error if configuration loading or `home_dir` resolution fails.
    pub fn load_layered(config_path: &PathBuf) -> Result<Self> {
        use figment::{
            Figment,
            providers::{Env, Format, Serialized, Yaml},
        };

        // For layered loading, start from a minimal base where optional sections are None,
        // so they remain None unless explicitly provided by YAML/ENV.
        let base = AppConfig {
            server: ServerConfig::default(),
            database: None,
            logging: None,
            tracing: None,
            modules_dir: None,
            modules: HashMap::new(),
        };

        let figment = Figment::new()
            .merge(Serialized::defaults(base))
            .merge(Yaml::file(config_path))
            // Example: APP__SERVER__PORT=8087 maps to server.port
            .merge(Env::prefixed("APP__").split("__"));

        let mut config: AppConfig = figment
            .extract()
            .with_context(|| "Failed to extract config from figment".to_owned())?;

        // Normalize + create home_dir immediately.
        config
            .server
            .normalize_home_dir_inplace()
            .context("Failed to resolve server.home_dir")?;

        // Merge module files if modules_dir is specified.
        if let Some(dir) = config.modules_dir.as_ref() {
            merge_module_files(&mut config.modules, dir)?;
        }

        Ok(config)
    }

    /// Load configuration from file or create with default values.
    /// Also normalizes `server.home_dir` into an absolute path and creates the directory.
    ///
    /// # Errors
    /// Returns an error if configuration loading or `home_dir` resolution fails.
    pub fn load_or_default(config_path: &Option<PathBuf>) -> Result<Self> {
        if let Some(path) = config_path {
            ensure!(
                path.is_file(),
                "config file does not exist: {}",
                path.to_string_lossy()
            );
            Self::load_layered(path)
        } else {
            let mut c = Self::default();
            c.server
                .normalize_home_dir_inplace()
                .context("Failed to resolve server.home_dir (defaults)")?;
            Ok(c)
        }
    }

    /// Serialize configuration to YAML.
    ///
    /// # Errors
    /// Returns an error if serialization fails.
    pub fn to_yaml(&self) -> Result<String> {
        serde_saphyr::to_string(self).context("Failed to serialize config to YAML")
    }

    /// Apply overrides from command line arguments.
    pub fn apply_cli_overrides(&mut self, verbose: u8) {
        // Set logging level based on verbose flags for "default" section.
        let logging = self.logging.get_or_insert_with(default_logging_config);
        if let Some(default_section) = logging.get_mut("default") {
            default_section.console_level = match verbose {
                0 => default_section.console_level, // keep
                1 => Some(Level::DEBUG),
                _ => Some(Level::TRACE),
            };
        }
    }
}

/// Command line arguments structure.
#[derive(Debug, Clone)]
pub struct CliArgs {
    pub config: Option<String>,
    pub print_config: bool,
    pub verbose: u8,
    pub mock: bool,
}

fn merge_module_files(
    bag: &mut HashMap<String, serde_json::Value>,
    dir: impl AsRef<Path>,
) -> Result<()> {
    use std::fs;
    let dir = dir.as_ref();
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext != "yml" && ext != "yaml" {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_owned();
        let raw = fs::read_to_string(&path)?;
        let json: serde_json::Value = serde_saphyr::from_str(&raw)?;
        bag.insert(name, json);
    }
    Ok(())
}

// ---- New ModKit DB Handling Functions ----

/// Expands environment variables in a DSN string.
/// Replaces `${VARNAME}` with the actual environment variable value.
///
/// # Errors
/// Returns an error if any referenced env var is missing.
pub fn expand_env_in_dsn(dsn: &str) -> Result<String> {
    use std::env;

    let mut result = dsn.to_owned();
    // TODO: Think about other way
    let re = regex::Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)}")?;

    for cap in re.captures_iter(dsn) {
        let full_match = &cap[0];
        let var_name = &cap[1];

        let value = env::var(var_name)
            .with_context(|| format!("Environment variable '{var_name}' not found in DSN"))?;

        result = result.replace(full_match, &value);
    }

    Ok(result)
}

/// Resolves password: if it contains ${VAR}, expands from environment variable; otherwise returns as-is.
///
/// # Errors
/// Returns an error if the referenced environment variable is not found.
pub fn resolve_password(password: Option<&str>) -> Result<Option<String>> {
    if let Some(pwd) = password {
        if pwd.starts_with("${") && pwd.ends_with('}') {
            // Extract variable name from ${VAR_NAME}
            let var_name = &pwd[2..pwd.len() - 1];
            let resolved = std::env::var(var_name).with_context(|| {
                format!("Environment variable '{var_name}' not found for password")
            })?;
            Ok(Some(resolved))
        } else {
            // Return literal password as-is
            Ok(Some(pwd.to_owned()))
        }
    } else {
        Ok(None)
    }
}

/// Validates that a DSN string is parseable by the dsn crate.
/// Note: `SQLite` DSNs have special formats that dsn crate doesn't recognize, so we skip validation for them.
///
/// # Errors
/// Returns an error if the DSN is invalid.
pub fn validate_dsn(dsn: &str) -> Result<()> {
    // Skip validation for SQLite DSNs as they use special syntax not recognized by dsn crate
    if dsn.starts_with("sqlite:") {
        return Ok(());
    }

    let _parsed = dsn::parse(dsn).map_err(|e| anyhow::anyhow!("Invalid DSN '{dsn}': {e}"))?;

    Ok(())
}

/// Resolves `SQLite` @`file()` syntax in DSN to actual file paths.
/// - `sqlite://@file(users.sqlite)` → `$HOME/.hyperspot/<module>/users.sqlite`
/// - `sqlite://@file(/abs/path/file.db)` → use absolute path
/// - `sqlite://` or `sqlite:///` → `$HOME/.hyperspot/<module>/<module>.sqlite`
fn resolve_sqlite_dsn(
    dsn: &str,
    home_dir: &Path,
    module_name: &str,
    dry_run: bool,
) -> Result<String> {
    if dsn.contains("@file(") {
        // Extract the file path from @file(...)
        if let Some(start) = dsn.find("@file(")
            && let Some(end) = dsn[start..].find(')')
        {
            let file_path = &dsn[start + 6..start + end]; // +6 for "@file("

            let resolved_path = if file_path.starts_with('/')
                || (file_path.len() > 1 && file_path.chars().nth(1) == Some(':'))
            {
                // Absolute path (Unix or Windows)
                PathBuf::from(file_path)
            } else {
                // Relative path - resolve under module directory
                let module_dir = home_dir.join(module_name);
                if !dry_run {
                    std::fs::create_dir_all(&module_dir).with_context(|| {
                        format!(
                            "Failed to create module directory: {}",
                            module_dir.display()
                        )
                    })?;
                }
                module_dir.join(file_path)
            };

            let normalized_path = resolved_path.to_string_lossy().replace('\\', "/");
            // For Windows absolute paths (C:/...), use sqlite:path format
            // For Unix absolute paths (/...), use sqlite://path format
            if normalized_path.len() > 1 && normalized_path.chars().nth(1) == Some(':') {
                // Windows absolute path like C:/...
                return Ok(format!("sqlite:{normalized_path}"));
            }
            // Unix absolute path or relative path
            return Ok(format!("sqlite://{normalized_path}"));
        }
        return Err(anyhow::anyhow!(
            "Invalid @file() syntax in SQLite DSN: {dsn}"
        ));
    }

    // Handle empty DSN or just sqlite:// - default to module.sqlite
    if dsn == "sqlite://" || dsn == "sqlite:///" || dsn == "sqlite:" {
        let module_dir = home_dir.join(module_name);
        if !dry_run {
            std::fs::create_dir_all(&module_dir).with_context(|| {
                format!(
                    "Failed to create module directory: {}",
                    module_dir.display()
                )
            })?;
        }
        let db_path = module_dir.join(format!("{module_name}.sqlite"));
        let normalized_path = db_path.to_string_lossy().replace('\\', "/");
        // For Windows absolute paths (C:/...), use sqlite:path format
        // For Unix absolute paths (/...), use sqlite://path format
        if normalized_path.len() > 1 && normalized_path.chars().nth(1) == Some(':') {
            // Windows absolute path like C:/...
            return Ok(format!("sqlite:{normalized_path}"));
        }
        // Unix absolute path or relative path
        return Ok(format!("sqlite://{normalized_path}"));
    }

    // Return DSN as-is for normal cases
    Ok(dsn.to_owned())
}

/// Builds a server-based DSN from individual fields.
/// Used when no base DSN is provided or when overriding DSN components.
/// Uses `url::Url` to properly handle percent-encoding of special characters.
fn build_server_dsn(
    scheme: &str,
    host: Option<&str>,
    port: Option<u16>,
    user: Option<&str>,
    password: Option<&str>,
    dbname: Option<&str>,
    params: &HashMap<String, String>,
) -> Result<String> {
    let host = host.unwrap_or("localhost");
    let user = user.unwrap_or("postgres"); // reasonable default for server-based DBs

    // Start with base URL
    let mut url = Url::parse(&format!("{scheme}://dummy/"))
        .with_context(|| format!("Invalid scheme: {scheme}"))?;

    // Set host (required)
    url.set_host(Some(host))
        .with_context(|| format!("Invalid host: {host}"))?;

    // Set port if provided
    if let Some(port) = port {
        url.set_port(Some(port))
            .map_err(|()| anyhow::anyhow!("Invalid port: {port}"))?;
    }

    // Set username
    url.set_username(user)
        .map_err(|()| anyhow::anyhow!("Failed to set username: {user}"))?;

    // Set password if provided
    if let Some(password) = password {
        url.set_password(Some(password))
            .map_err(|()| anyhow::anyhow!("Failed to set password"))?;
    }

    // Set database name as path (with leading slash)
    if let Some(dbname) = dbname {
        // Manually encode the dbname to handle special characters
        let encoded_dbname = urlencoding::encode(dbname);
        url.set_path(&format!("/{encoded_dbname}"));
    } else {
        url.set_path("/");
    }

    // Set query parameters
    if !params.is_empty() {
        // Use url::Url::query_pairs_mut() to properly handle encoding
        let mut query_pairs = url.query_pairs_mut();
        for (key, value) in params {
            query_pairs.append_pair(key, value);
        }
    }

    Ok(url.to_string())
}

/// Builds a `SQLite` DSN by replacing the database file path while preserving query parameters.
fn build_sqlite_dsn_with_dbname_override(
    original_dsn: &str,
    dbname: &str,
    module_name: &str,
    home_dir: &Path,
    dry_run: bool,
) -> Result<String> {
    // Parse the original DSN to extract query parameters
    let query_params = if let Some(query_start) = original_dsn.find('?') {
        &original_dsn[query_start..]
    } else {
        ""
    };

    // Build the correct path for the database file
    let module_dir = home_dir.join(module_name);
    if !dry_run {
        std::fs::create_dir_all(&module_dir).with_context(|| {
            format!(
                "Failed to create module directory: {}",
                module_dir.display()
            )
        })?;
    }
    let db_path = module_dir.join(dbname);
    let normalized_path = db_path.to_string_lossy().replace('\\', "/");

    // Build the new DSN with correct format for the platform
    let dsn_base = if normalized_path.len() > 1 && normalized_path.chars().nth(1) == Some(':') {
        // Windows absolute path like C:/...
        format!("sqlite:{normalized_path}")
    } else {
        // Unix absolute path or relative path
        format!("sqlite://{normalized_path}")
    };

    Ok(format!("{dsn_base}{query_params}"))
}

/// Builds a `SQLite` DSN from file/path or validates existing DSN.
/// If dbname is provided, it overrides the database file in the DSN.
///
/// # Arguments
/// * `dry_run` - If true, skip directory creation (for read-only inspection)
fn build_sqlite_dsn(
    dsn: Option<&str>,
    file: Option<&str>,
    path: Option<&PathBuf>,
    dbname: Option<&str>,
    module_name: &str,
    home_dir: &Path,
    dry_run: bool,
) -> Result<String> {
    // If full DSN provided, resolve @file() syntax and validate
    if let Some(dsn) = dsn {
        let resolved_dsn = resolve_sqlite_dsn(dsn, home_dir, module_name, dry_run)?;

        // If dbname is provided, we need to replace the database file path while preserving query params
        if let Some(dbname) = dbname {
            return build_sqlite_dsn_with_dbname_override(
                &resolved_dsn,
                dbname,
                module_name,
                home_dir,
                dry_run,
            );
        }

        validate_dsn(&resolved_dsn)?;
        return Ok(resolved_dsn);
    }

    // Build from path (absolute)
    if let Some(path) = path {
        let absolute_path = if path.is_absolute() {
            path.clone()
        } else {
            home_dir.join(path)
        };
        let normalized_path = absolute_path.to_string_lossy().replace('\\', "/");
        // For Windows absolute paths (C:/...), use sqlite:path format
        // For Unix absolute paths (/...), use sqlite://path format
        if normalized_path.len() > 1 && normalized_path.chars().nth(1) == Some(':') {
            // Windows absolute path like C:/...
            return Ok(format!("sqlite:{normalized_path}"));
        }
        // Unix absolute path or relative path
        return Ok(format!("sqlite://{normalized_path}"));
    }

    // Build from file (relative under module dir)
    if let Some(file) = file {
        let module_dir = home_dir.join(module_name);
        if !dry_run {
            std::fs::create_dir_all(&module_dir).with_context(|| {
                format!(
                    "Failed to create module directory: {}",
                    module_dir.display()
                )
            })?;
        }
        let db_path = module_dir.join(file);
        let normalized_path = db_path.to_string_lossy().replace('\\', "/");
        // For Windows absolute paths (C:/...), use sqlite:path format
        // For Unix absolute paths (/...), use sqlite://path format
        if normalized_path.len() > 1 && normalized_path.chars().nth(1) == Some(':') {
            // Windows absolute path like C:/...
            return Ok(format!("sqlite:{normalized_path}"));
        }
        // Unix absolute path or relative path
        return Ok(format!("sqlite://{normalized_path}"));
    }

    // Default to module.sqlite
    let module_dir = home_dir.join(module_name);
    if !dry_run {
        std::fs::create_dir_all(&module_dir).with_context(|| {
            format!(
                "Failed to create module directory: {}",
                module_dir.display()
            )
        })?;
    }
    let db_path = module_dir.join(format!("{module_name}.sqlite"));
    let normalized_path = db_path.to_string_lossy().replace('\\', "/");
    // For Windows absolute paths (C:/...), use sqlite:path format
    // For Unix absolute paths (/...), use sqlite://path format
    if normalized_path.len() > 1 && normalized_path.chars().nth(1) == Some(':') {
        // Windows absolute path like C:/...
        Ok(format!("sqlite:{normalized_path}"))
    } else {
        // Unix absolute path or relative path
        Ok(format!("sqlite://{normalized_path}"))
    }
}

/// Type alias for the complex return type of `build_final_db_for_module`
type DbConfigResult = Result<Option<(String /* final_dsn */, PoolCfg)>>;

/// Builder for accumulating database configuration from multiple sources
#[derive(Default)]
struct DbConfigBuilder {
    dsn: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    user: Option<String>,
    password: Option<String>,
    dbname: Option<String>,
    params: HashMap<String, String>,
    pool: PoolCfg,
}

impl DbConfigBuilder {
    fn new() -> Self {
        Self::default()
    }

    /// Apply global server configuration
    fn apply_global_server(
        &mut self,
        global_server: &DbConnConfig,
        home_dir: &Path,
        module_name: &str,
        dry_run: bool,
    ) -> Result<()> {
        // Apply global server DSN
        if let Some(global_dsn) = &global_server.dsn {
            let expanded_dsn = expand_env_in_dsn(global_dsn)?;
            // For SQLite, resolve @file() syntax before validation
            let resolved_dsn = if expanded_dsn.starts_with("sqlite") {
                resolve_sqlite_dsn(&expanded_dsn, home_dir, module_name, dry_run)?
            } else {
                expanded_dsn
            };
            validate_dsn(&resolved_dsn)?;
            self.dsn = Some(resolved_dsn);
        }

        // Apply global server fields (override DSN parts)
        if let Some(host) = &global_server.host {
            self.host = Some(host.clone());
        }
        if let Some(port) = global_server.port {
            self.port = Some(port);
        }
        if let Some(user) = &global_server.user {
            self.user = Some(user.clone());
        }
        if let Some(password) = resolve_password(global_server.password.as_deref())? {
            self.password = Some(password);
        }
        if let Some(dbname) = &global_server.dbname {
            self.dbname = Some(dbname.clone());
        }
        if let Some(params) = &global_server.params {
            self.params.extend(params.clone());
        }
        if let Some(pool) = &global_server.pool {
            self.pool = pool.clone();
        }

        Ok(())
    }

    /// Apply module DSN (overrides global DSN)
    fn apply_module_dsn(
        &mut self,
        module_dsn: &str,
        home_dir: &Path,
        module_name: &str,
        dry_run: bool,
    ) -> Result<()> {
        // For SQLite, resolve @file() syntax before validation
        let resolved_dsn = if module_dsn.starts_with("sqlite") {
            resolve_sqlite_dsn(module_dsn, home_dir, module_name, dry_run)?
        } else {
            module_dsn.to_owned()
        };
        validate_dsn(&resolved_dsn)?;
        self.dsn = Some(resolved_dsn);
        Ok(())
    }

    /// Apply module fields (override everything)
    fn apply_module_fields(&mut self, module_db_config: &DbConnConfig) -> Result<()> {
        if let Some(host) = &module_db_config.host {
            self.host = Some(host.clone());
        }
        if let Some(port) = module_db_config.port {
            self.port = Some(port);
        }
        if let Some(user) = &module_db_config.user {
            self.user = Some(user.clone());
        }
        if let Some(password) = resolve_password(module_db_config.password.as_deref())? {
            self.password = Some(password);
        }
        if let Some(dbname) = &module_db_config.dbname {
            self.dbname = Some(dbname.clone());
        }
        if let Some(params) = &module_db_config.params {
            self.params.extend(params.clone());
        }
        if let Some(pool) = &module_db_config.pool {
            // Module pool settings override global ones
            if let Some(max_conns) = pool.max_conns {
                self.pool.max_conns = Some(max_conns);
            }
            if let Some(acquire_timeout) = pool.acquire_timeout {
                self.pool.acquire_timeout = Some(acquire_timeout);
            }
        }
        Ok(())
    }

    /// Check if we have any field overrides that require rebuilding the DSN
    fn has_field_overrides(&self) -> bool {
        self.host.is_some()
            || self.port.is_some()
            || self.user.is_some()
            || self.password.is_some()
            || !self.params.is_empty()
    }
}

/// Determines the database backend type (`SQLite` or server-based)
fn decide_backend(builder: &DbConfigBuilder, module_db_config: &DbConnConfig) -> bool {
    // Always treat as SQLite if DSN starts with "sqlite", regardless of server reference
    // Also treat as SQLite if no server reference and no explicit DSN (default case)
    module_db_config.file.is_some()
        || module_db_config.path.is_some()
        || builder
            .dsn
            .as_ref()
            .is_some_and(|dsn| dsn.starts_with("sqlite"))
        || (module_db_config.server.is_none() && builder.dsn.is_none())
}

/// Finalize `SQLite` DSN from builder state
fn finalize_sqlite_dsn(
    builder: &DbConfigBuilder,
    module_db_config: &DbConnConfig,
    module_name: &str,
    home_dir: &Path,
    dry_run: bool,
) -> Result<String> {
    build_sqlite_dsn(
        builder.dsn.as_deref(),
        module_db_config.file.as_deref(),
        module_db_config.path.as_ref(),
        builder.dbname.as_deref(),
        module_name,
        home_dir,
        dry_run,
    )
}

/// Finalize server-based DSN from builder state
fn finalize_server_dsn(builder: &DbConfigBuilder, module_name: &str) -> Result<String> {
    // Extract dbname from DSN if not provided separately
    let dbname = if let Some(dbname) = builder.dbname.as_deref() {
        dbname.to_owned()
    } else if let Some(dsn) = builder.dsn.as_ref() {
        // Try to extract dbname from DSN path
        if let Ok(parsed) = url::Url::parse(dsn) {
            let path = parsed.path();
            if path.len() > 1 {
                // Remove leading slash and return the path as dbname
                path[1..].to_string()
            } else {
                return Err(anyhow::anyhow!(
                    "Server-based database config for module '{module_name}' missing required 'dbname'"
                ));
            }
        } else {
            return Err(anyhow::anyhow!(
                "Server-based database config for module '{module_name}' missing required 'dbname'"
            ));
        }
    } else {
        return Err(anyhow::anyhow!(
            "Server-based database config for module '{module_name}' missing required 'dbname'"
        ));
    };

    if builder.has_field_overrides() || builder.dsn.is_none() {
        // Build DSN from fields when we have overrides or no original DSN
        let scheme = if let Some(dsn) = &builder.dsn {
            let parsed = Url::parse(dsn)?;
            parsed.scheme().to_owned()
        } else {
            "postgresql".to_owned() // default
        };

        build_server_dsn(
            &scheme,
            builder.host.as_deref(),
            builder.port,
            builder.user.as_deref(),
            builder.password.as_deref(),
            Some(&dbname),
            &builder.params,
        )
    } else if let Some(original_dsn) = &builder.dsn {
        // Use original DSN when no field overrides (but update dbname if needed)
        if let Ok(mut parsed) = Url::parse(original_dsn) {
            // Update the path with the final dbname if it's different
            let original_dbname = parsed.path().trim_start_matches('/');
            if original_dbname != dbname {
                parsed.set_path(&format!("/{dbname}"));
            }
            Ok(parsed.to_string())
        } else {
            // Fallback to building from fields if URL parsing fails
            build_server_dsn(
                "postgresql",
                builder.host.as_deref(),
                builder.port,
                builder.user.as_deref(),
                builder.password.as_deref(),
                Some(&dbname),
                &builder.params,
            )
        }
    } else {
        // This branch should not be reachable due to the condition above
        unreachable!("final_dsn should not be None when has_field_overrides is false")
    }
}

/// Redacts password from DSN for logging
fn redact_dsn_for_logging(dsn: &str) -> Result<String> {
    if dsn.contains('@') {
        let parsed = Url::parse(dsn)?;
        let mut log_url = parsed;
        if log_url.password().is_some() {
            log_url.set_password(Some("***")).ok();
        }
        Ok(log_url.to_string())
    } else {
        Ok(dsn.to_owned())
    }
}

// ---- OoP Module Configuration Support ----

/// Environment variable name for passing rendered module config to `OoP` modules.
pub const MODKIT_MODULE_CONFIG_ENV: &str = "MODKIT_MODULE_CONFIG";

/// Rendered database configuration for `OoP` modules.
/// Contains both global server templates and module-specific config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderedDbConfig {
    /// Global database configuration with server templates.
    /// `OoP` module can use these servers for reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global: Option<GlobalDatabaseConfig>,
    /// Module-specific database configuration (already merged with server reference in master).
    /// This is the `modules.<name>.database` section after server merge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<DbConnConfig>,
}

impl RenderedDbConfig {
    /// Create a new `RenderedDbConfig` from global and module database configurations.
    #[must_use]
    pub fn new(global: Option<GlobalDatabaseConfig>, module: Option<DbConnConfig>) -> Self {
        Self { global, module }
    }
}

/// Rendered module configuration passed to `OoP` modules via environment variable.
///
/// This struct contains everything an `OoP` module needs to initialize:
/// - Database configuration (structured, for field-by-field merge in `OoP`)
/// - Module config section
/// - Logging configuration (for key-by-key merge in `OoP`)
/// - Tracing configuration for OTEL
///
/// The runtime section is excluded as it's only relevant for the master host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderedModuleConfig {
    /// Rendered database configuration (structured, not resolved DSN).
    /// `OoP` module will merge this with local --config using field-by-field merge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<RenderedDbConfig>,
    /// Module-specific config section (passed as-is)
    #[serde(default)]
    pub config: serde_json::Value,
    /// Logging configuration from master host.
    /// `OoP` module will merge this with local --config (local keys override master keys).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<LoggingConfig>,
    /// Tracing configuration from master host for OTEL initialization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracing: Option<TracingConfig>,
}

impl RenderedModuleConfig {
    /// Deserialize from JSON string (used when reading from env var).
    ///
    /// # Errors
    /// Returns an error if JSON parsing fails.
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).context("Failed to parse RenderedModuleConfig from JSON")
    }

    /// Serialize to JSON string (used when passing to `OoP` modules via env var).
    ///
    /// # Errors
    /// Returns an error if serialization fails.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).context("Failed to serialize RenderedModuleConfig to JSON")
    }
}

/// Render module configuration for passing to `OoP` module via environment variable.
///
/// This function prepares a structured configuration that an `OoP` module can use
/// to initialize itself. The configuration includes:
/// - Database configuration (structured, for field-by-field merge in `OoP`)
/// - Module config section
/// - Logging configuration (for key-by-key merge in `OoP`)
/// - Tracing configuration for OTEL
///
/// The runtime section is excluded as it's only relevant for the master host.
///
/// `OoP` modules receive this via `MODKIT_MODULE_CONFIG` env var and can override
/// any section with their local --config file.
///
/// # Errors
/// Returns an error if module configuration parsing fails.
pub fn render_module_config_for_oop(
    app: &AppConfig,
    module_name: &str,
    _home_dir: &std::path::Path,
) -> Result<RenderedModuleConfig> {
    // Get module's database config (with server reference, but NOT resolved to DSN).
    // OoP module will use DbManager to resolve this with its local overrides.
    let module_db_config = parse_module_config(app, module_name)
        .ok()
        .and_then(|entry| entry.database);

    // Build database config with global servers and module config (structured, not resolved)
    let database = if module_db_config.is_some() || app.database.is_some() {
        Some(RenderedDbConfig::new(
            app.database.clone(),
            module_db_config,
        ))
    } else {
        None
    };

    // Get the module's config section (excluding database and runtime)
    let config = parse_module_config(app, module_name)
        .map(|entry| entry.config)
        .unwrap_or_default();

    // Pass logging config from master host so OoP modules can merge with their local config
    let logging = app.logging.clone();

    // Pass tracing config from master host so OoP modules use the same OTEL settings
    let tracing = app.tracing.clone();

    Ok(RenderedModuleConfig {
        database,
        config,
        logging,
        tracing,
    })
}

/// Parse a module config from the config bag.
///
/// # Errors
/// Returns an error if the module is not found or config parsing fails.
pub fn parse_module_config(app: &AppConfig, module_name: &str) -> Result<ModuleConfig> {
    let module_raw = app
        .modules
        .get(module_name)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Module '{module_name}' not found in config"))?;

    let module_config: ModuleConfig = serde_json::from_value(module_raw)?;
    Ok(module_config)
}

/// Helper to get runtime config for a module (if present).
///
/// # Errors
/// Returns an error if module config parsing fails.
pub fn get_module_runtime_config(
    app: &AppConfig,
    module_name: &str,
) -> Result<Option<ModuleRuntime>> {
    let entry = parse_module_config(app, module_name)?;
    Ok(entry.runtime)
}

/// Merges global + module DB configs into a final, validated DSN and pool config.
/// Precedence: Global DSN -> Global fields -> Module DSN -> Module fields (fields always win).
/// For server-based, returns error if final dbname is missing.
/// For `SQLite`, builds/normalizes sqlite DSN from file/path or uses a full DSN as-is.
///
/// # Arguments
/// * `dry_run` - If true, skip directory creation (for read-only inspection)
///
/// # Errors
/// Returns an error if database configuration is invalid or resolution fails.
pub fn build_final_db_for_module(
    app: &AppConfig,
    module_name: &str,
    home_dir: &Path,
    dry_run: bool,
) -> DbConfigResult {
    // Parse module entry from raw JSON
    let Some(module_raw) = app.modules.get(module_name) else {
        return Ok(None); // No module config
    };

    let module_entry: ModuleConfig = serde_json::from_value(module_raw.clone())
        .with_context(|| format!("Invalid module config structure for '{module_name}'"))?;

    let Some(module_db_config) = module_entry.database else {
        tracing::warn!(
            "Module '{}' has no database configuration; DB capability disabled",
            module_name
        );
        return Ok(None);
    };

    // Global database config
    let global_db_config = app.database.as_ref();

    // Build configuration using the builder pattern
    let mut builder = DbConfigBuilder::new();

    // Step 1: Apply global server config if referenced
    if let Some(server_name) = &module_db_config.server {
        let global_server = global_db_config
            .and_then(|gc| gc.servers.get(server_name))
            .ok_or_else(|| {
                anyhow::anyhow!("Referenced server '{server_name}' not found in global config")
            })?;

        builder.apply_global_server(global_server, home_dir, module_name, dry_run)?;
    }

    // Step 2: Apply module DSN (override global)
    if let Some(module_dsn) = &module_db_config.dsn {
        builder.apply_module_dsn(module_dsn, home_dir, module_name, dry_run)?;
    }

    // Step 3: Apply module fields (override everything)
    builder.apply_module_fields(&module_db_config)?;

    // Determine backend type and finalize DSN
    let is_sqlite = decide_backend(&builder, &module_db_config);

    let result_dsn = if is_sqlite {
        finalize_sqlite_dsn(&builder, &module_db_config, module_name, home_dir, dry_run)?
    } else {
        finalize_server_dsn(&builder, module_name)?
    };

    // Validate final DSN
    validate_dsn(&result_dsn)?;

    // Redact password for logging
    let log_dsn = redact_dsn_for_logging(&result_dsn)?;

    tracing::info!(
        "Built final DB config for module '{}': {}",
        module_name,
        log_dsn
    );

    Ok(Some((result_dsn, builder.pool)))
}

/// Helper function to get module database configuration from `AppConfig`.
/// Returns the `DbConnConfig` for a module, or None if the module has no database config.
#[must_use]
pub fn get_module_db_config(app: &AppConfig, module_name: &str) -> Option<DbConnConfig> {
    let module_raw = app.modules.get(module_name)?;
    let module_entry: ModuleConfig = serde_json::from_value(module_raw.clone()).ok()?;
    module_entry.database
}

/// Helper function to resolve module home directory.
/// Returns the path where module-specific files (like `SQLite` databases) should be stored.
#[must_use]
pub fn module_home(app: &AppConfig, module_name: &str) -> PathBuf {
    PathBuf::from(&app.server.home_dir).join(module_name)
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use std::fs;
    use temp_env::with_var;
    use tempfile::tempdir;

    /// Helper: a normalized `home_dir` should be absolute and not start with '~'.
    fn is_normalized_path(p: &Path) -> bool {
        p.is_absolute() && !p.starts_with("~")
    }

    /// Helper: platform default subdirectory name.
    fn default_subdir() -> &'static str {
        ".hyperspot"
    }

    #[test]
    fn test_default_config_structure() {
        let config = AppConfig::default();

        // Database defaults (simplified structure)
        assert!(config.database.is_some());
        let db = config.database.as_ref().unwrap();
        assert!(db.servers.is_empty()); // Default config has no servers defined
        assert_eq!(db.auto_provision, None);

        // Logging defaults
        assert!(config.logging.is_some());
        let logging = config.logging.as_ref().unwrap();
        assert!(logging.contains_key("default"));

        let default_section = &logging["default"];
        assert_eq!(default_section.console_level, Some(Level::INFO));
        assert_eq!(default_section.file, "logs/hyperspot.log");

        // Modules bag is empty by default
        assert!(config.modules.is_empty());
    }

    #[test]
    fn test_load_layered_normalizes_home_dir() {
        let tmp = tempdir().unwrap();
        let cfg_path = tmp.path().join("cfg.yaml");

        // Provide a user path with "~" to ensure expansion and normalization.
        let yaml = r#"
server:
  home_dir: "~/.test_hyperspot"

database:
  servers:
    test_postgres:
      dsn: "postgres://user:pass@localhost/db"
      pool:
        max_conns: 20

logging:
  default:
    console_level: debug
    file: "logs/default.log"
"#;
        fs::write(&cfg_path, yaml).unwrap();

        let config = AppConfig::load_layered(&cfg_path).unwrap();

        // home_dir should be normalized immediately
        assert!(is_normalized_path(&config.server.home_dir));
        assert!(config.server.home_dir.ends_with(".test_hyperspot"));

        // database parsed (TODO: update test to use new config format)
        // For now, since this test uses old format YAML, we skip DB assertions
        // let db = config.database.as_ref().unwrap();

        // logging parsed
        let logging = config.logging.as_ref().unwrap();
        let def = &logging["default"];
        assert_eq!(def.console_level, Some(Level::DEBUG));
        assert_eq!(def.file, "logs/default.log");
    }

    #[test]
    fn test_load_or_default_normalizes_home_dir_when_none() {
        // No external file => defaults, but home_dir must be normalized.
        // Ensure platform env is present for home resolution in CI.
        let tmp = tempdir().unwrap();
        let env_var = if cfg!(target_os = "windows") {
            "APPDATA"
        } else {
            "HOME"
        };
        with_var(env_var, Some(tmp.path().to_str().unwrap()), || {
            let config = AppConfig::load_or_default(&None).unwrap();
            assert!(is_normalized_path(&config.server.home_dir));
            assert!(config.server.home_dir.ends_with(default_subdir()));
        });
    }

    #[test]
    fn test_minimal_yaml_config() {
        let tmp = tempdir().unwrap();
        let cfg_path = tmp.path().join("cfg.yaml");

        let yaml = r#"
server:
  home_dir: "~/.minimal"
"#;
        fs::write(&cfg_path, yaml).unwrap();

        let config = AppConfig::load_layered(&cfg_path).unwrap();

        // Required fields are parsed; home_dir normalized
        assert!(is_normalized_path(&config.server.home_dir));
        assert!(config.server.home_dir.ends_with(".minimal"));

        // Optional sections default to None
        assert!(config.database.is_none());
        assert!(config.logging.is_none());
        assert!(config.modules.is_empty());
    }

    #[test]
    fn test_cli_overrides() {
        let mut config = AppConfig::default();

        let args = CliArgs {
            config: None,
            print_config: false,
            verbose: 2, // trace
            mock: false,
        };

        config.apply_cli_overrides(args.verbose);

        // Port override

        // Verbose override affects logging
        let logging = config.logging.as_ref().unwrap();
        let default_section = &logging["default"];
        assert_eq!(default_section.console_level, Some(Level::TRACE));
    }

    #[test]
    fn test_cli_verbose_levels_matrix() {
        for (verbose_level, expected_log_level) in [
            (0, Some(Level::INFO)), // unchanged from default
            (1, Some(Level::DEBUG)),
            (2, Some(Level::TRACE)),
            (3, Some(Level::TRACE)), // cap at trace
        ] {
            let mut config = AppConfig::default();
            let args = CliArgs {
                config: None,
                print_config: false,
                verbose: verbose_level,
                mock: false,
            };

            config.apply_cli_overrides(args.verbose);

            let logging = config.logging.as_ref().unwrap();
            let default_section = &logging["default"];

            if verbose_level == 0 {
                assert_eq!(default_section.console_level, Some(Level::INFO));
            } else {
                assert_eq!(default_section.console_level, expected_log_level);
            }
        }
    }

    #[test]
    fn test_layered_config_loading_with_modules_dir() {
        let tmp = tempdir().unwrap();
        let cfg_path = tmp.path().join("modules_dir.yaml");
        let modules_dir = tmp.path().join("modules");

        fs::create_dir_all(&modules_dir).unwrap();
        let module_cfg = modules_dir.join("test_module.yaml");
        fs::write(
            &module_cfg,
            r#"
setting1: "value1"
setting2: 42
"#,
        )
        .unwrap();

        // Convert Windows paths to forward slashes for YAML compatibility
        let modules_dir_str = modules_dir.to_string_lossy().replace('\\', "/");
        let yaml = format!(
            r#"
server:
  home_dir: "~/.modules_test"

modules_dir: "{modules_dir_str}"

modules:
  existing_module:
    key: "value"
"#
        );

        fs::write(&cfg_path, yaml).unwrap();

        let config = AppConfig::load_layered(&cfg_path).unwrap();

        // Should have loaded the existing module from modules section
        assert!(config.modules.contains_key("existing_module"));

        // Should have also loaded the module from modules_dir
        assert!(config.modules.contains_key("test_module"));

        // Check the loaded module config
        let test_module = &config.modules["test_module"];
        assert_eq!(test_module["setting1"], "value1");
        assert_eq!(test_module["setting2"], 42);
    }

    #[test]
    fn test_load_and_init_logging_smoke() {
        // Just verifies structure is acceptable for logging init path.
        let tmp = tempdir().unwrap();
        let cfg_path = tmp.path().join("logging.yaml");
        let yaml = r#"
server:
  home_dir: "~/.logging_test"

logging:
  default:
    console_level: debug
    file: ""
    file_level: info
"#;
        fs::write(&cfg_path, yaml).unwrap();

        let config = AppConfig::load_layered(&cfg_path).unwrap();
        assert!(config.logging.is_some());
        let logging = config.logging.as_ref().unwrap();
        assert!(logging.contains_key("default"));

        let default_section = &logging["default"];
        assert_eq!(default_section.console_level, Some(Level::DEBUG));
        assert_eq!(default_section.file_level, Some(Level::INFO));
        // not calling init to avoid side effects in tests
    }

    // ===================== DB Configuration Precedence Tests =====================

    /// Helper function to create `AppConfig` with database server configuration
    fn create_app_with_server(server_name: &str, db_config: DbConnConfig) -> AppConfig {
        let mut servers = HashMap::new();
        servers.insert(server_name.to_owned(), db_config);

        AppConfig {
            database: Some(GlobalDatabaseConfig {
                servers,
                auto_provision: None,
            }),
            ..Default::default()
        }
    }

    /// Helper function to add a module to `AppConfig`
    fn add_module_to_app(
        app: &mut AppConfig,
        module_name: &str,
        database_config: &serde_json::Value,
    ) {
        app.modules.insert(
            module_name.to_owned(),
            serde_json::json!({
                "database": database_config,
                "config": {}
            }),
        );
    }

    /// Helper function to add a module with custom config to `AppConfig`
    fn add_module_with_config(app: &mut AppConfig, module_name: &str, config: &serde_json::Value) {
        app.modules.insert(
            module_name.to_owned(),
            serde_json::json!({
                "database": {},
                "config": config
            }),
        );
    }

    /// Helper function to create a minimal `AppConfig` for testing
    fn create_minimal_app() -> AppConfig {
        AppConfig {
            database: None,
            modules: HashMap::new(),
            ..Default::default()
        }
    }

    #[test]
    fn test_precedence_global_dsn_only() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        let mut app = create_app_with_server(
            "test_server",
            DbConnConfig {
                dsn: Some(
                    "postgresql://global_user:global_pass@global_host:5432/global_db".to_owned(),
                ),
                ..Default::default()
            },
        );

        // Module references global server
        add_module_to_app(
            &mut app,
            "test_module",
            &serde_json::json!({
                "server": "test_server"
            }),
        );

        let result = build_final_db_for_module(&app, "test_module", home_dir, false).unwrap();
        assert!(result.is_some());

        let (dsn, _pool) = result.unwrap();
        assert!(dsn.contains("global_user"));
        assert!(dsn.contains("global_host"));
        assert!(dsn.contains("global_db"));
    }

    #[test]
    fn test_precedence_global_fields_only() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        let mut app = create_app_with_server(
            "test_server",
            DbConnConfig {
                host: Some("field_host".to_owned()),
                port: Some(5433),
                user: Some("field_user".to_owned()),
                dbname: Some("field_db".to_owned()),
                ..Default::default()
            },
        );

        // Module references global server
        add_module_to_app(
            &mut app,
            "test_module",
            &serde_json::json!({
                "server": "test_server"
            }),
        );

        let result = build_final_db_for_module(&app, "test_module", home_dir, false).unwrap();
        assert!(result.is_some());

        let (dsn, _pool) = result.unwrap();
        assert!(dsn.contains("field_host"));
        assert!(dsn.contains("5433"));
        assert!(dsn.contains("field_user"));
        assert!(dsn.contains("field_db"));
    }

    #[test]
    fn test_precedence_module_dsn_only() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        let app = AppConfig {
            modules: {
                let mut modules = HashMap::new();
                modules.insert(
                    "test_module".to_owned(),
                    serde_json::json!({
                        "database": {
                            "dsn": "sqlite://module_test.db?wal=true&synchronous=NORMAL"
                        },
                        "config": {}
                    }),
                );
                modules
            },
            ..Default::default()
        };

        let result = build_final_db_for_module(&app, "test_module", home_dir, false).unwrap();
        assert!(result.is_some());

        let (dsn, _pool) = result.unwrap();
        assert!(dsn.contains("module_test.db"));
        assert!(dsn.contains("wal=true"));
    }

    #[test]
    fn test_precedence_module_fields_only() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        let app = AppConfig {
            modules: {
                let mut modules = HashMap::new();
                modules.insert(
                    "test_module".to_owned(),
                    serde_json::json!({
                        "database": {
                            "file": "module_fields.db"
                        },
                        "config": {}
                    }),
                );
                modules
            },
            ..Default::default()
        };

        let result = build_final_db_for_module(&app, "test_module", home_dir, false).unwrap();
        assert!(result.is_some());

        let (dsn, _pool) = result.unwrap();
        assert!(dsn.contains("module_fields.db"));
        // Platform-specific DSN format check
        #[cfg(windows)]
        assert!(dsn.starts_with("sqlite:") && !dsn.starts_with("sqlite://"));
        #[cfg(unix)]
        assert!(dsn.starts_with("sqlite://"));
    }

    #[test]
    fn test_precedence_fields_override_dsn() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        let mut app = create_app_with_server(
            "test_server",
            DbConnConfig {
                dsn: Some("postgresql://old_user:old_pass@old_host:5432/old_db".to_owned()),
                host: Some("new_host".to_owned()), // This should override DSN host
                port: Some(5433),                  // This should override DSN port
                user: Some("new_user".to_owned()), // This should override DSN user
                dbname: Some("new_db".to_owned()), // This should override DSN dbname
                ..Default::default()
            },
        );

        // Module also overrides some fields
        add_module_to_app(
            &mut app,
            "test_module",
            &serde_json::json!({
                "server": "test_server",
                "port": 5434  // Module field should override global field
            }),
        );

        let result = build_final_db_for_module(&app, "test_module", home_dir, false).unwrap();
        assert!(result.is_some());

        let (dsn, _pool) = result.unwrap();
        // Fields should override DSN parts
        assert!(dsn.contains("new_host"));
        assert!(dsn.contains("5434")); // Module override should win
        assert!(dsn.contains("new_user"));
        assert!(dsn.contains("new_db"));
        // Old DSN values should not appear
        assert!(!dsn.contains("old_host"));
        assert!(!dsn.contains("5432"));
        assert!(!dsn.contains("old_user"));
        assert!(!dsn.contains("old_db"));
    }

    #[test]
    fn test_env_expansion_password() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        with_var("TEST_DB_PASSWORD", Some("secret123"), || {
            let mut app = create_app_with_server(
                "test_server",
                DbConnConfig {
                    host: Some("localhost".to_owned()),
                    port: Some(5432),
                    user: Some("testuser".to_owned()),
                    password: Some("${TEST_DB_PASSWORD}".to_owned()), // Should expand to "secret123"
                    dbname: Some("testdb".to_owned()),
                    ..Default::default()
                },
            );

            add_module_to_app(
                &mut app,
                "test_module",
                &serde_json::json!({
                    "server": "test_server"
                }),
            );

            let result = build_final_db_for_module(&app, "test_module", home_dir, false).unwrap();
            assert!(result.is_some());

            let (dsn, _pool) = result.unwrap();
            assert!(dsn.contains("secret123"));
        });
    }

    #[test]
    fn test_env_expansion_in_dsn() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        temp_env::with_vars(
            [
                ("DB_HOST", Some("test-server")),
                ("DB_PASSWORD", Some("env_secret")),
            ],
            || {
                let mut app = create_app_with_server(
                    "test_server",
                    DbConnConfig {
                        dsn: Some(
                            "postgresql://user:${DB_PASSWORD}@${DB_HOST}:5432/mydb".to_owned(),
                        ),
                        ..Default::default()
                    },
                );

                add_module_to_app(
                    &mut app,
                    "test_module",
                    &serde_json::json!({
                        "server": "test_server"
                    }),
                );

                let result =
                    build_final_db_for_module(&app, "test_module", home_dir, false).unwrap();
                assert!(result.is_some());

                let (dsn, _pool) = result.unwrap();
                assert!(dsn.contains("test-server"));
                assert!(dsn.contains("env_secret"));
                // ${} placeholders should be replaced
                assert!(!dsn.contains("${DB_HOST}"));
                assert!(!dsn.contains("${DB_PASSWORD}"));
            },
        );
    }

    #[test]
    fn test_sqlite_file_path_resolution() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        // Test 1: file (relative to home_dir/module_name/)
        let app1 = AppConfig {
            modules: {
                let mut modules = HashMap::new();
                modules.insert(
                    "test_module".to_owned(),
                    serde_json::json!({
                        "database": {
                            "file": "test.db"
                        },
                        "config": {}
                    }),
                );
                modules
            },
            ..Default::default()
        };

        let result1 = build_final_db_for_module(&app1, "test_module", home_dir, false).unwrap();
        assert!(result1.is_some());
        let (dsn1, _) = result1.unwrap();
        assert!(dsn1.contains("test_module"));
        assert!(dsn1.contains("test.db"));

        // Test 2: path (absolute path)
        let abs_path = tmp.path().join("absolute.db");
        let app2 = AppConfig {
            modules: {
                let mut modules = HashMap::new();
                modules.insert(
                    "test_module".to_owned(),
                    serde_json::json!({
                        "database": {
                            "path": abs_path.to_string_lossy()
                        },
                        "config": {}
                    }),
                );
                modules
            },
            ..Default::default()
        };

        let result2 = build_final_db_for_module(&app2, "test_module", home_dir, false).unwrap();
        assert!(result2.is_some());
        let (dsn2, _) = result2.unwrap();
        assert!(dsn2.contains("absolute.db"));

        // Test 3: no file or path (should default to module_name.sqlite)
        let app3 = AppConfig {
            modules: {
                let mut modules = HashMap::new();
                modules.insert(
                    "test_module".to_owned(),
                    serde_json::json!({
                        "database": {},
                        "config": {}
                    }),
                );
                modules
            },
            ..Default::default()
        };

        let result3 = build_final_db_for_module(&app3, "test_module", home_dir, false).unwrap();
        assert!(result3.is_some());
        let (dsn3, _) = result3.unwrap();
        assert!(dsn3.contains("test_module.sqlite"));
    }

    #[cfg(windows)]
    #[test]
    fn test_sqlite_path_resolution_windows() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        let app = AppConfig {
            modules: {
                let mut modules = HashMap::new();
                modules.insert(
                    "test_module".to_owned(),
                    serde_json::json!({
                        "database": {
                            "file": "test.db"
                        },
                        "config": {}
                    }),
                );
                modules
            },
            ..Default::default()
        };

        let result = build_final_db_for_module(&app, "test_module", home_dir, false).unwrap();
        assert!(result.is_some());
        let (dsn, _) = result.unwrap();

        // On Windows, paths should be normalized to forward slashes in DSN
        assert!(!dsn.contains('\\'));
        assert!(dsn.contains('/'));
    }

    #[test]
    fn test_sqlite_dsn_with_server_reference_and_dbname_override() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        let mut app = AppConfig::default();

        // Global server with SQLite DSN and query params
        let mut servers = HashMap::new();
        servers.insert(
            "sqlite_users".to_owned(),
            DbConnConfig {
                engine: None,
                dsn: Some(
                    "sqlite://users_info.db?WAL=true&synchronous=NORMAL&busy_timeout=5000"
                        .to_owned(),
                ),
                host: None,
                port: None,
                user: None,
                password: None,
                dbname: None,
                params: None,
                pool: None,
                file: None,
                path: None,
                server: None,
            },
        );

        app.database = Some(GlobalDatabaseConfig {
            servers,
            auto_provision: None,
        });

        // Module that references the server but overrides the dbname
        app.modules.insert(
            "users_info".to_owned(),
            serde_json::json!({
                "database": {
                    "server": "sqlite_users",
                    "dbname": "users_info.db"
                },
                "config": {}
            }),
        );

        let result = build_final_db_for_module(&app, "users_info", home_dir, false).unwrap();
        assert!(result.is_some());
        let (dsn, _) = result.unwrap();

        // Should be an absolute path with preserved query parameters
        assert!(dsn.contains("?WAL=true&synchronous=NORMAL&busy_timeout=5000"));
        assert!(dsn.contains("users_info/users_info.db"));

        // Platform-specific path format
        #[cfg(windows)]
        {
            // Windows should use sqlite:C:/path format
            assert!(dsn.starts_with("sqlite:"));
            assert!(!dsn.starts_with("sqlite://"));
        }

        #[cfg(unix)]
        {
            // Unix should use sqlite://path format
            assert!(dsn.starts_with("sqlite://"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_sqlite_path_resolution_unix() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        let app = AppConfig {
            modules: {
                let mut modules = HashMap::new();
                modules.insert(
                    "test_module".to_owned(),
                    serde_json::json!({
                        "database": {
                            "file": "test.db"
                        },
                        "config": {}
                    }),
                );
                modules
            },
            ..Default::default()
        };

        let result = build_final_db_for_module(&app, "test_module", home_dir, false).unwrap();
        assert!(result.is_some());
        let (dsn, _) = result.unwrap();

        // On Unix, paths should be absolute
        assert!(dsn.starts_with("sqlite://"));
        assert!(dsn.contains("/test_module/test.db"));
    }

    #[test]
    fn test_server_based_db_missing_dbname_error() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        let mut app = create_app_with_server(
            "test_server",
            DbConnConfig {
                host: Some("localhost".to_owned()),
                port: Some(5432),
                user: Some("testuser".to_owned()),
                // Missing dbname for server-based DB
                ..Default::default()
            },
        );

        add_module_to_app(
            &mut app,
            "test_module",
            &serde_json::json!({
                "server": "test_server"
            }),
        );

        let result = build_final_db_for_module(&app, "test_module", home_dir, false);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("missing required 'dbname'"));
    }

    #[test]
    fn test_module_no_database_config() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        // Module with no database section
        let app = AppConfig {
            modules: {
                let mut modules = HashMap::new();
                modules.insert(
                    "no_db_module".to_owned(),
                    serde_json::json!({
                        "config": {
                            "some_setting": "value"
                        }
                    }),
                );
                modules
            },
            ..Default::default()
        };

        let result = build_final_db_for_module(&app, "no_db_module", home_dir, false).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_module_empty_database_config() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        // Module with empty database section
        let app = AppConfig {
            modules: {
                let mut modules = HashMap::new();
                modules.insert(
                    "empty_db_module".to_owned(),
                    serde_json::json!({
                        "database": null,
                        "config": {}
                    }),
                );
                modules
            },
            ..Default::default()
        };

        let result = build_final_db_for_module(&app, "empty_db_module", home_dir, false).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_referenced_server_not_found() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        let app = AppConfig {
            modules: {
                let mut modules = HashMap::new();
                modules.insert(
                    "test_module".to_owned(),
                    serde_json::json!({
                        "database": {
                            "server": "nonexistent_server"
                        },
                        "config": {}
                    }),
                );
                modules
            },
            ..Default::default()
        };

        let result = build_final_db_for_module(&app, "test_module", home_dir, false);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Referenced server 'nonexistent_server' not found"));
    }

    #[test]
    fn test_dsn_validation_invalid_url() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        let app = AppConfig {
            modules: {
                let mut modules = HashMap::new();
                modules.insert(
                    "test_module".to_owned(),
                    serde_json::json!({
                        "database": {
                            "dsn": "invalid://not-a-valid[url"
                        },
                        "config": {}
                    }),
                );
                modules
            },
            ..Default::default()
        };

        let result = build_final_db_for_module(&app, "test_module", home_dir, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_env_variable_not_found() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        // Use with_var with None to ensure the env var doesn't exist
        with_var("NONEXISTENT_PASSWORD", None::<&str>, || {
            let mut app = create_app_with_server(
                "test_server",
                DbConnConfig {
                    host: Some("localhost".to_owned()),
                    password: Some("${NONEXISTENT_PASSWORD}".to_owned()),
                    dbname: Some("testdb".to_owned()),
                    ..Default::default()
                },
            );

            add_module_to_app(
                &mut app,
                "test_module",
                &serde_json::json!({
                    "server": "test_server"
                }),
            );

            let result = build_final_db_for_module(&app, "test_module", home_dir, false);
            assert!(result.is_err());
            let error_msg = result.unwrap_err().to_string();
            assert!(error_msg.contains("NONEXISTENT_PASSWORD"));
        });
    }

    #[test]
    fn test_sqlite_at_file_relative_path() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        let app = AppConfig {
            modules: {
                let mut modules = HashMap::new();
                modules.insert(
                    "test_module".to_owned(),
                    serde_json::json!({
                        "database": {
                            "dsn": "sqlite://@file(users.db)"
                        },
                        "config": {}
                    }),
                );
                modules
            },
            ..Default::default()
        };

        let result = build_final_db_for_module(&app, "test_module", home_dir, false).unwrap();
        assert!(result.is_some());

        let (dsn, _pool) = result.unwrap();
        assert!(dsn.contains("test_module"));
        assert!(dsn.contains("users.db"));
        // Platform-specific DSN format check
        #[cfg(windows)]
        assert!(dsn.starts_with("sqlite:") && !dsn.starts_with("sqlite://"));
        #[cfg(unix)]
        assert!(dsn.starts_with("sqlite:///"));
    }

    #[test]
    fn test_sqlite_at_file_absolute_path() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();
        let abs_path = tmp.path().join("absolute_db.sqlite");

        let app = AppConfig {
            modules: {
                let mut modules = HashMap::new();
                modules.insert(
                    "test_module".to_owned(),
                    serde_json::json!({
                        "database": {
                            "dsn": format!("sqlite://@file({})", abs_path.to_string_lossy())
                        },
                        "config": {}
                    }),
                );
                modules
            },
            ..Default::default()
        };

        let result = build_final_db_for_module(&app, "test_module", home_dir, false).unwrap();
        assert!(result.is_some());

        let (dsn, _pool) = result.unwrap();
        assert!(dsn.contains("absolute_db.sqlite"));
        // Platform-specific DSN format check
        #[cfg(windows)]
        assert!(dsn.starts_with("sqlite:") && !dsn.starts_with("sqlite://"));
        #[cfg(unix)]
        assert!(dsn.starts_with("sqlite:///"));
    }

    #[test]
    fn test_sqlite_empty_dsn_default() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        let app = AppConfig {
            modules: {
                let mut modules = HashMap::new();
                modules.insert(
                    "test_module".to_owned(),
                    serde_json::json!({
                        "database": {
                            "dsn": "sqlite://"
                        },
                        "config": {}
                    }),
                );
                modules
            },
            ..Default::default()
        };

        let result = build_final_db_for_module(&app, "test_module", home_dir, false).unwrap();
        assert!(result.is_some());

        let (dsn, _pool) = result.unwrap();
        assert!(dsn.contains("test_module"));
        assert!(dsn.contains("test_module.sqlite"));
        // Platform-specific DSN format check
        #[cfg(windows)]
        assert!(dsn.starts_with("sqlite:") && !dsn.starts_with("sqlite://"));
        #[cfg(unix)]
        assert!(dsn.starts_with("sqlite:///"));
    }

    #[test]
    fn test_sqlite_at_file_invalid_syntax() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        let app = AppConfig {
            modules: {
                let mut modules = HashMap::new();
                modules.insert(
                    "test_module".to_owned(),
                    serde_json::json!({
                        "database": {
                            "dsn": "sqlite://@file(missing_closing_paren"
                        },
                        "config": {}
                    }),
                );
                modules
            },
            ..Default::default()
        };

        let result = build_final_db_for_module(&app, "test_module", home_dir, false);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Invalid @file() syntax"));
    }

    #[test]
    fn test_dsn_special_characters_in_credentials() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        // Test with special characters in username and password
        let mut app = create_app_with_server(
            "test_server",
            DbConnConfig {
                host: Some("localhost".to_owned()),
                port: Some(5432),
                user: Some("user@domain".to_owned()),
                password: Some("pa@ss:w0rd/with%special&chars".to_owned()),
                dbname: Some("test/db".to_owned()),
                ..Default::default()
            },
        );

        add_module_to_app(
            &mut app,
            "test_module",
            &serde_json::json!({
                "server": "test_server"
            }),
        );

        let result = build_final_db_for_module(&app, "test_module", home_dir, false).unwrap();
        assert!(result.is_some());

        let (dsn, _pool) = result.unwrap();

        // Verify DSN is properly encoded
        assert!(dsn.starts_with("postgresql://"));
        assert!(dsn.contains("user%40domain")); // @ encoded as %40
        assert!(dsn.contains("/test%2Fdb")); // / in dbname encoded as %2F

        // Verify DSN is parseable and contains expected user
        validate_dsn(&dsn).expect("DSN with special characters should be valid");

        // Parse the DSN to verify it contains the correct components
        let parsed_dsn = dsn::parse(&dsn).expect("DSN should be parseable");
        assert_eq!(parsed_dsn.username.as_deref(), Some("user@domain"));
        assert_eq!(
            parsed_dsn.password.as_deref(),
            Some("pa@ss:w0rd/with%special&chars")
        );
        // Note: dsn crate may have limitations with path parsing - just verify the main DSN works
        // The important thing is that the DSN is valid and contains the right components
    }

    #[test]
    #[allow(clippy::non_ascii_literal)]
    fn test_dsn_unicode_characters() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        // Test with Unicode characters
        let mut app = create_app_with_server(
            "test_server",
            DbConnConfig {
                host: Some("localhost".to_owned()),
                user: Some("ユーザー".to_owned()), // Japanese characters
                dbname: Some("unicode_db".to_owned()),
                ..Default::default()
            },
        );

        add_module_to_app(
            &mut app,
            "test_module",
            &serde_json::json!({
                "server": "test_server"
            }),
        );

        let result = build_final_db_for_module(&app, "test_module", home_dir, false).unwrap();
        assert!(result.is_some());

        let (dsn, _pool) = result.unwrap();

        // Verify DSN is properly encoded with Unicode
        assert!(dsn.starts_with("postgresql://"));
        // Unicode characters should be percent-encoded
        assert!(dsn.contains('%')); // Should contain encoded characters

        // Verify DSN is parseable
        validate_dsn(&dsn).expect("DSN with Unicode characters should be valid");
    }

    #[test]
    fn test_dsn_query_parameters_encoding() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        let mut params = HashMap::new();
        params.insert("ssl mode".to_owned(), "require & verify".to_owned());
        params.insert("application_name".to_owned(), "my-app/v1.0".to_owned());

        let mut app = create_app_with_server(
            "test_server",
            DbConnConfig {
                host: Some("localhost".to_owned()),
                user: Some("testuser".to_owned()),
                dbname: Some("testdb".to_owned()),
                params: Some(params),
                ..Default::default()
            },
        );

        add_module_to_app(
            &mut app,
            "test_module",
            &serde_json::json!({
                "server": "test_server"
            }),
        );

        let result = build_final_db_for_module(&app, "test_module", home_dir, false).unwrap();
        assert!(result.is_some());

        let (dsn, _pool) = result.unwrap();

        // Verify query parameters are properly encoded (spaces become +, & becomes %26)
        assert!(dsn.contains("ssl+mode=require+%26+verify"));
        assert!(dsn.contains("application_name=my-app%2Fv1.0"));

        // Verify DSN is parseable
        validate_dsn(&dsn).expect("DSN with encoded query parameters should be valid");
    }

    #[test]
    fn test_pool_config_merging() {
        use std::time::Duration;

        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        // Global server with pool config
        let mut app = create_app_with_server(
            "test_server",
            DbConnConfig {
                host: Some("localhost".to_owned()),
                dbname: Some("testdb".to_owned()),
                pool: Some(PoolCfg {
                    max_conns: Some(10),
                    min_conns: None,
                    acquire_timeout: Some(Duration::from_secs(5)),
                    idle_timeout: None,
                    max_lifetime: None,
                    test_before_acquire: None,
                }),
                ..Default::default()
            },
        );

        // Module overrides only max_conns
        add_module_to_app(
            &mut app,
            "test_module",
            &serde_json::json!({
                "server": "test_server",
                "pool": {
                    "max_conns": 20
                }
            }),
        );

        let result = build_final_db_for_module(&app, "test_module", home_dir, false).unwrap();
        assert!(result.is_some());

        let (_dsn, pool) = result.unwrap();
        assert_eq!(pool.max_conns, Some(20)); // Module override wins
        assert_eq!(pool.acquire_timeout, Some(Duration::from_secs(5))); // Global value preserved
    }

    #[test]
    fn test_pool_config_module_overrides_all() {
        use std::time::Duration;

        let tmp = tempdir().unwrap();
        let home_dir = tmp.path();

        // Global server with pool config
        let mut app = create_app_with_server(
            "test_server",
            DbConnConfig {
                host: Some("localhost".to_owned()),
                dbname: Some("testdb".to_owned()),
                pool: Some(PoolCfg {
                    max_conns: Some(10),
                    min_conns: None,
                    acquire_timeout: Some(Duration::from_secs(5)),
                    idle_timeout: None,
                    max_lifetime: None,
                    test_before_acquire: None,
                }),
                ..Default::default()
            },
        );

        // Module overrides both pool settings
        add_module_to_app(
            &mut app,
            "test_module",
            &serde_json::json!({
                "server": "test_server",
                "pool": {
                    "max_conns": 30,
                    "acquire_timeout": "10s"
                }
            }),
        );

        let result = build_final_db_for_module(&app, "test_module", home_dir, false).unwrap();
        assert!(result.is_some());

        let (_dsn, pool) = result.unwrap();
        assert_eq!(pool.max_conns, Some(30));
        assert_eq!(pool.acquire_timeout, Some(Duration::from_secs(10)));
    }

    #[test]
    fn test_list_module_names() {
        let mut app = create_minimal_app();
        add_module_with_config(&mut app, "zebra_module", &serde_json::json!({}));
        add_module_with_config(&mut app, "alpha_module", &serde_json::json!({}));
        add_module_with_config(&mut app, "beta_module", &serde_json::json!({}));

        let module_names = list_module_names(&app);

        // Should be sorted alphabetically
        assert_eq!(module_names.len(), 3);
        assert_eq!(module_names[0], "alpha_module");
        assert_eq!(module_names[1], "beta_module");
        assert_eq!(module_names[2], "zebra_module");
    }

    #[test]
    fn test_list_module_names_empty() {
        let app = create_minimal_app();
        let module_names = list_module_names(&app);
        assert_eq!(module_names.len(), 0);
    }

    #[test]
    fn test_redact_dsn_password_postgres() {
        let dsn = "postgres://user:secretpass@localhost:5432/mydb";
        let redacted = redact_dsn_password(dsn).unwrap();
        assert_eq!(
            redacted,
            "postgres://user:***REDACTED***@localhost:5432/mydb"
        );
    }

    #[test]
    fn test_redact_dsn_password_no_password() {
        let dsn = "postgres://user@localhost:5432/mydb";
        let redacted = redact_dsn_password(dsn).unwrap();
        // No password means no redaction needed
        assert_eq!(redacted, "postgres://user@localhost:5432/mydb");
    }

    #[test]
    fn test_redact_dsn_password_special_chars() {
        let dsn = "postgres://user:p@ss%40word@localhost:5432/mydb";
        let redacted = redact_dsn_password(dsn).unwrap();
        assert_eq!(
            redacted,
            "postgres://user:***REDACTED***@localhost:5432/mydb"
        );
    }

    #[test]
    fn test_render_effective_modules_config() {
        let mut app = create_minimal_app();
        add_module_with_config(
            &mut app,
            "test_module",
            &serde_json::json!({
                "my_setting": "my_value",
                "enabled": true
            }),
        );

        let result = render_effective_modules_config(&app).unwrap();

        // Check structure
        assert!(result.is_object());
        let modules = result.as_object().unwrap();
        assert!(modules.contains_key("test_module"));

        let test_module = modules.get("test_module").unwrap();
        assert!(test_module.is_object());
        let test_module_obj = test_module.as_object().unwrap();

        // Should have config section
        assert!(test_module_obj.contains_key("config"));

        // Check config section
        let config = test_module_obj.get("config").unwrap();
        assert_eq!(config.get("my_setting").unwrap(), "my_value");
        assert_eq!(config.get("enabled").unwrap(), true);
    }

    #[test]
    fn test_render_effective_modules_config_with_database() {
        let mut app = create_app_with_server(
            "test_server",
            DbConnConfig {
                host: Some("localhost".to_owned()),
                port: Some(5432),
                user: Some("user".to_owned()),
                password: Some("pass".to_owned()),
                dbname: Some("db".to_owned()),
                ..Default::default()
            },
        );

        // Module with database config
        add_module_to_app(
            &mut app,
            "test_module",
            &serde_json::json!({
                "server": "test_server"
            }),
        );

        let result = render_effective_modules_config(&app).unwrap();
        let modules = result.as_object().unwrap();
        let test_module = modules.get("test_module").unwrap().as_object().unwrap();

        // Should have database section
        assert!(test_module.contains_key("database"));
        let database = test_module.get("database").unwrap().as_object().unwrap();
        assert!(database.contains_key("dsn"));

        // DSN should be redacted
        let dsn = database.get("dsn").unwrap().as_str().unwrap();
        assert!(dsn.contains("***REDACTED***"));
        assert!(!dsn.contains("pass"));
    }

    #[test]
    fn test_render_effective_modules_config_minimal() {
        // Test that modules with minimal/no config can be rendered
        let mut app = create_minimal_app();

        // Manually add a module with no database or config sections
        app.modules
            .insert("minimal_module".to_owned(), serde_json::json!({}));

        let result = render_effective_modules_config(&app).unwrap();

        // Module should be present in output (or excluded if truly empty)
        // Either way, rendering should succeed
        assert!(result.is_object());
    }

    #[test]
    fn test_dump_effective_modules_config_yaml() {
        let mut app = create_minimal_app();
        add_module_with_config(
            &mut app,
            "test_module",
            &serde_json::json!({
                "setting": "value"
            }),
        );

        let yaml = dump_effective_modules_config_yaml(&app).unwrap();

        // Should be valid YAML
        assert!(yaml.contains("test_module:"));
        assert!(yaml.contains("config:"));
        assert!(yaml.contains("setting: value"));
    }

    #[test]
    fn test_dump_effective_modules_config_json() {
        let mut app = create_minimal_app();
        add_module_with_config(
            &mut app,
            "test_module",
            &serde_json::json!({
                "setting": "value"
            }),
        );

        let json = dump_effective_modules_config_json(&app).unwrap();

        // Should be valid JSON
        assert!(json.contains("\"test_module\""));
        assert!(json.contains("\"config\""));
        assert!(json.contains("\"setting\""));
        assert!(json.contains("\"value\""));

        // Verify it's parseable
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_object());
    }

    #[test]
    fn test_render_multiple_modules() {
        let mut app = create_minimal_app();
        add_module_with_config(&mut app, "module_a", &serde_json::json!({"a": 1}));
        add_module_with_config(&mut app, "module_b", &serde_json::json!({"b": 2}));
        add_module_with_config(&mut app, "module_c", &serde_json::json!({"c": 3}));

        let result = render_effective_modules_config(&app).unwrap();
        let modules = result.as_object().unwrap();

        assert_eq!(modules.len(), 3);
        assert!(modules.contains_key("module_a"));
        assert!(modules.contains_key("module_b"));
        assert!(modules.contains_key("module_c"));
    }
}

// Note: DB trait implementations and helper functions removed since we now use DbManager
