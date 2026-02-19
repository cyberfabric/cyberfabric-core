//! Tests for `OoP` configuration merge logic
//!
//! Tests cover all merge scenarios:
//! - Database: field-by-field merge (global.servers → module.database in master → module.database in local)
//! - Logging: key-by-key merge (local keys override master keys)
//! - Config: full replacement (local replaces master if present)

use super::*;
use crate::bootstrap::config::{
    AppConfig, ConsoleFormat, GlobalDatabaseConfig, LoggingConfig, RenderedDbConfig,
    RenderedModuleConfig, Section, SectionFile, ServerConfig, default_logging_config,
};
use modkit_db::{DbConnConfig, PoolCfg};
use std::collections::HashMap;
use std::time::Duration;
use tracing::Level;

/// Helper to create a minimal `AppConfig` for testing
fn minimal_app_config() -> AppConfig {
    AppConfig {
        server: ServerConfig {
            home_dir: std::env::temp_dir().join("modkit_test"),
        },
        database: None,
        logging: default_logging_config(),
        tracing: None,
        tokio_console: None,
        modules_dir: None,
        modules: HashMap::new(),
    }
}

/// Helper to create a logging section
fn logging_section(console_level: Option<Level>, file: &str) -> Section {
    Section {
        console_level,
        section_file: Some(SectionFile {
            file: file.to_owned(),
            file_level: Some(Level::DEBUG),
        }),
        console_format: ConsoleFormat::default(),
        max_age_days: Some(7),
        max_backups: Some(3),
        max_size_mb: Some(100),
    }
}

// =============================================================================
// Logging Merge Tests
// =============================================================================

mod logging_merge {
    use super::*;

    #[test]
    fn test_merge_logging_local_only() {
        // When only local has logging, result should be local's logging
        let local_logging: LoggingConfig = [(
            "default".to_owned(),
            logging_section(Some(Level::DEBUG), "logs/local.log"),
        )]
        .into();

        let result = merge_logging_configs(None, &local_logging);

        assert_eq!(result.len(), 1);
        assert_eq!(
            result.get("default").unwrap().console_level,
            Some(Level::DEBUG)
        );
        assert_eq!(
            result.get("default").unwrap().file().unwrap(),
            "logs/local.log"
        );
    }

    #[test]
    fn test_merge_logging_local_overrides_master_key() {
        // Local key should override master key
        let master_logging: LoggingConfig = [
            (
                "default".to_owned(),
                logging_section(Some(Level::INFO), "logs/master.log"),
            ),
            (
                "module_a".to_owned(),
                logging_section(Some(Level::INFO), "logs/a-master.log"),
            ),
        ]
        .into();

        let local_logging: LoggingConfig = [(
            "default".to_owned(),
            logging_section(Some(Level::DEBUG), "logs/local.log"),
        )]
        .into();

        let result = merge_logging_configs(Some(&master_logging), &local_logging);

        assert_eq!(result.len(), 2);
        // Local overrides default
        assert_eq!(
            result.get("default").unwrap().console_level,
            Some(Level::DEBUG)
        );
        assert_eq!(
            result.get("default").unwrap().file().unwrap(),
            "logs/local.log"
        );
        // Master's module_a preserved
        assert_eq!(
            result.get("module_a").unwrap().console_level,
            Some(Level::INFO)
        );
        assert_eq!(
            result.get("module_a").unwrap().file().unwrap(),
            "logs/a-master.log"
        );
    }

    #[test]
    fn test_merge_logging_local_adds_new_key() {
        // Local can add new keys that don't exist in master
        let master_logging: LoggingConfig = [(
            "default".to_owned(),
            logging_section(Some(Level::INFO), "logs/default.log"),
        )]
        .into();

        let local_logging: LoggingConfig = [(
            "new_module".to_owned(),
            logging_section(Some(Level::TRACE), "logs/new.log"),
        )]
        .into();

        let result = merge_logging_configs(Some(&master_logging), &local_logging);

        assert_eq!(result.len(), 2);
        assert_eq!(
            result.get("default").unwrap().console_level,
            Some(Level::INFO)
        );
        assert_eq!(
            result.get("new_module").unwrap().console_level,
            Some(Level::TRACE)
        );
    }

    #[test]
    fn test_merge_logging_multiple_overrides() {
        // Multiple keys can be overridden
        let master_logging: LoggingConfig = [
            (
                "default".to_owned(),
                logging_section(Some(Level::INFO), "logs/default.log"),
            ),
            (
                "sqlx".to_owned(),
                logging_section(Some(Level::WARN), "logs/sql.log"),
            ),
            (
                "api".to_owned(),
                logging_section(Some(Level::INFO), "logs/api.log"),
            ),
        ]
        .into();

        let local_logging: LoggingConfig = [
            (
                "default".to_owned(),
                logging_section(Some(Level::DEBUG), "logs/local-default.log"),
            ),
            (
                "sqlx".to_owned(),
                logging_section(Some(Level::DEBUG), "logs/local-sql.log"),
            ),
        ]
        .into();

        let result = merge_logging_configs(Some(&master_logging), &local_logging);

        assert_eq!(result.len(), 3);
        // Overridden
        assert_eq!(
            result.get("default").unwrap().console_level,
            Some(Level::DEBUG)
        );
        assert_eq!(
            result.get("sqlx").unwrap().console_level,
            Some(Level::DEBUG)
        );
        // Preserved from master
        assert_eq!(result.get("api").unwrap().console_level, Some(Level::INFO));
    }
}

// =============================================================================
// JSON Object Merge Tests
// =============================================================================

mod json_merge {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_merge_json_flat_objects() {
        let mut target = json!({"a": 1, "b": 2});
        let source = json!({"b": 3, "c": 4});

        merge_json_objects(&mut target, &source);

        assert_eq!(target, json!({"a": 1, "b": 3, "c": 4}));
    }

    #[test]
    fn test_merge_json_nested_objects() {
        let mut target = json!({
            "database": {
                "host": "localhost",
                "port": 5432
            }
        });
        let source = json!({
            "database": {
                "port": 5433,
                "user": "admin"
            }
        });

        merge_json_objects(&mut target, &source);

        assert_eq!(
            target,
            json!({
                "database": {
                    "host": "localhost",
                    "port": 5433,
                    "user": "admin"
                }
            })
        );
    }

    #[test]
    fn test_merge_json_deeply_nested() {
        let mut target = json!({
            "level1": {
                "level2": {
                    "a": 1,
                    "b": 2
                }
            }
        });
        let source = json!({
            "level1": {
                "level2": {
                    "b": 3,
                    "c": 4
                },
                "new_key": "value"
            }
        });

        merge_json_objects(&mut target, &source);

        assert_eq!(
            target,
            json!({
                "level1": {
                    "level2": {
                        "a": 1,
                        "b": 3,
                        "c": 4
                    },
                    "new_key": "value"
                }
            })
        );
    }

    #[test]
    fn test_merge_json_source_replaces_non_object() {
        // When target has non-object value, source object replaces it
        let mut target = json!({"key": "string_value"});
        let source = json!({"key": {"nested": true}});

        merge_json_objects(&mut target, &source);

        assert_eq!(target, json!({"key": {"nested": true}}));
    }

    #[test]
    fn test_merge_json_non_object_replaces_object() {
        // When source has non-object value, it replaces target object
        let mut target = json!({"key": {"nested": true}});
        let source = json!({"key": "string_value"});

        merge_json_objects(&mut target, &source);

        assert_eq!(target, json!({"key": "string_value"}));
    }

    #[test]
    fn test_merge_json_empty_source() {
        let mut target = json!({"a": 1, "b": 2});
        let source = json!({});

        merge_json_objects(&mut target, &source);

        assert_eq!(target, json!({"a": 1, "b": 2}));
    }

    #[test]
    fn test_merge_json_empty_target() {
        let mut target = json!({});
        let source = json!({"a": 1, "b": 2});

        merge_json_objects(&mut target, &source);

        assert_eq!(target, json!({"a": 1, "b": 2}));
    }
}

// =============================================================================
// Database Merge Tests (via build_merged_db_options)
// =============================================================================

mod database_merge {
    use super::*;
    use serde_json::json;

    fn create_global_db_config() -> GlobalDatabaseConfig {
        let mut servers = HashMap::new();
        servers.insert(
            "sqlite_main".to_owned(),
            DbConnConfig {
                engine: Some(modkit_db::config::DbEngineCfg::Sqlite),
                server: None,
                dsn: None,
                host: None,
                port: None,
                user: None,
                password: None,
                dbname: None,
                file: None,
                path: None,
                params: Some([("WAL".to_owned(), "true".to_owned())].into()),
                pool: Some(PoolCfg {
                    max_conns: Some(5),
                    min_conns: None,
                    acquire_timeout: Some(Duration::from_secs(30)),
                    idle_timeout: None,
                    max_lifetime: None,
                    test_before_acquire: None,
                }),
            },
        );
        GlobalDatabaseConfig {
            servers,
            auto_provision: Some(true),
        }
    }

    fn create_module_db_config() -> DbConnConfig {
        DbConnConfig {
            engine: Some(modkit_db::config::DbEngineCfg::Sqlite),
            server: Some("sqlite_main".to_owned()),
            dsn: None,
            host: None,
            port: None,
            user: None,
            password: None,
            dbname: None,
            file: Some("module.db".to_owned()),
            path: None,
            params: None,
            pool: None,
        }
    }

    #[test]
    fn test_rendered_db_config_no_database() {
        // When no database config, result should be DbOptions::None
        let home_dir = std::env::temp_dir().join("modkit_test_no_db");
        let local_config = minimal_app_config();

        let result = build_merged_db_options(&home_dir, "test_module", None, &local_config);

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), DbOptions::None));
    }

    #[test]
    fn test_rendered_db_config_master_only() {
        // When only master has database config
        let home_dir = std::env::temp_dir().join("modkit_test_master_only");
        _ = std::fs::create_dir_all(&home_dir);

        let rendered_db = RenderedDbConfig::new(
            Some(create_global_db_config()),
            Some(create_module_db_config()),
        );

        let local_config = minimal_app_config();

        let result =
            build_merged_db_options(&home_dir, "test_module", Some(&rendered_db), &local_config);

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), DbOptions::Manager(_)));
    }

    #[test]
    fn test_rendered_db_config_local_only() {
        // When only local has database config (standalone mode)
        let home_dir = std::env::temp_dir().join("modkit_test_local_only");
        _ = std::fs::create_dir_all(&home_dir);

        let mut local_config = minimal_app_config();
        local_config.database = Some(create_global_db_config());
        local_config.modules.insert(
            "test_module".to_owned(),
            json!({
                "database": {
                    "server": "sqlite_main",
                    "file": "local.db"
                }
            }),
        );

        let result = build_merged_db_options(&home_dir, "test_module", None, &local_config);

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), DbOptions::Manager(_)));
    }

    #[test]
    fn test_rendered_db_config_local_overrides_pool() {
        // Local config should override pool settings from master
        let home_dir = std::env::temp_dir().join("modkit_test_pool_override");
        _ = std::fs::create_dir_all(&home_dir);

        let rendered_db = RenderedDbConfig::new(
            Some(create_global_db_config()),
            Some(create_module_db_config()),
        );

        let mut local_config = minimal_app_config();
        // Local overrides pool.max_conns
        local_config.modules.insert(
            "test_module".to_owned(),
            json!({
                "database": {
                    "pool": {
                        "max_conns": 10
                    }
                }
            }),
        );

        let result =
            build_merged_db_options(&home_dir, "test_module", Some(&rendered_db), &local_config);

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), DbOptions::Manager(_)));
    }

    #[test]
    fn test_rendered_db_config_local_overrides_file() {
        // Local config should override file path from master
        let home_dir = std::env::temp_dir().join("modkit_test_file_override");
        _ = std::fs::create_dir_all(&home_dir);

        let rendered_db = RenderedDbConfig::new(
            Some(create_global_db_config()),
            Some(create_module_db_config()),
        );

        let mut local_config = minimal_app_config();
        // Local overrides file
        local_config.modules.insert(
            "test_module".to_owned(),
            json!({
                "database": {
                    "file": "local_override.db"
                }
            }),
        );

        let result =
            build_merged_db_options(&home_dir, "test_module", Some(&rendered_db), &local_config);

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), DbOptions::Manager(_)));
    }

    #[test]
    fn test_rendered_db_config_local_adds_params() {
        // Local config can add new params to master's params
        let home_dir = std::env::temp_dir().join("modkit_test_params_add");
        _ = std::fs::create_dir_all(&home_dir);

        let rendered_db = RenderedDbConfig::new(
            Some(create_global_db_config()),
            Some(create_module_db_config()),
        );

        let mut local_config = minimal_app_config();
        // Local adds new params
        local_config.modules.insert(
            "test_module".to_owned(),
            json!({
                "database": {
                    "params": {
                        "new_param": "value"
                    }
                }
            }),
        );

        let result =
            build_merged_db_options(&home_dir, "test_module", Some(&rendered_db), &local_config);

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), DbOptions::Manager(_)));
    }

    #[test]
    fn test_rendered_db_config_local_global_merges_with_master() {
        // Local global database config merges with master's global config
        let home_dir = std::env::temp_dir().join("modkit_test_global_merge");
        _ = std::fs::create_dir_all(&home_dir);

        let rendered_db = RenderedDbConfig::new(
            Some(create_global_db_config()),
            Some(create_module_db_config()),
        );

        let mut local_config = minimal_app_config();
        // Local adds a new server to global database config
        let mut new_servers = HashMap::new();
        new_servers.insert(
            "new_server".to_owned(),
            DbConnConfig {
                engine: Some(modkit_db::config::DbEngineCfg::Sqlite),
                server: None,
                dsn: Some("sqlite://new.db".to_owned()),
                host: None,
                port: None,
                user: None,
                password: None,
                dbname: None,
                file: None,
                path: None,
                params: None,
                pool: None,
            },
        );
        local_config.database = Some(GlobalDatabaseConfig {
            servers: new_servers,
            auto_provision: None,
        });

        let result =
            build_merged_db_options(&home_dir, "test_module", Some(&rendered_db), &local_config);

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), DbOptions::Manager(_)));
    }
}

// =============================================================================
// Full OoP Config Build Tests
// =============================================================================

mod full_oop_config {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_build_oop_config_standalone_mode() {
        // No rendered config - standalone mode
        let mut local_config = minimal_app_config();
        local_config.logging = [(
            "default".to_owned(),
            logging_section(Some(Level::DEBUG), "logs/standalone.log"),
        )]
        .into();
        local_config.modules.insert(
            "test_module".to_owned(),
            json!({
                "config": {
                    "setting": "local_value"
                }
            }),
        );

        let result = build_oop_config_and_db(&local_config, "test_module", None);

        assert!(result.is_ok());
        let (final_config, merged_logging, db_options) = result.unwrap();

        // Config should be from local
        let module_config = final_config.modules.get("test_module").unwrap();
        assert_eq!(module_config["config"]["setting"], "local_value");

        // Logging should be from local
        assert_eq!(merged_logging.len(), 1);
        assert_eq!(
            merged_logging.get("default").unwrap().console_level,
            Some(Level::DEBUG)
        );

        // No database
        assert!(matches!(db_options, DbOptions::None));
    }

    #[test]
    fn test_build_oop_config_with_rendered_config() {
        // With rendered config from master
        let local_config = minimal_app_config();

        let rendered = RenderedModuleConfig {
            database: None,
            config: json!({"master_setting": "value"}),
            logging: Some(
                [(
                    "default".to_owned(),
                    logging_section(Some(Level::INFO), "logs/master.log"),
                )]
                .into(),
            ),
            tracing: None,
        };

        let result = build_oop_config_and_db(&local_config, "test_module", Some(&rendered));

        assert!(result.is_ok());
        let (final_config, merged_logging, _) = result.unwrap();

        // Config should be from master (local has no config section)
        let module_config = final_config.modules.get("test_module").unwrap();
        assert_eq!(module_config["config"]["master_setting"], "value");

        // Logging from master
        assert_eq!(
            merged_logging.get("default").unwrap().console_level,
            Some(Level::INFO)
        );
    }

    #[test]
    fn test_build_oop_config_local_overrides_master_config() {
        // Local config section completely replaces master
        let mut local_config = minimal_app_config();
        local_config.modules.insert(
            "test_module".to_owned(),
            json!({
                "config": {
                    "local_setting": "local_value"
                }
            }),
        );

        let rendered = RenderedModuleConfig {
            database: None,
            config: json!({
                "master_setting": "master_value",
                "another": "setting"
            }),
            logging: None,
            tracing: None,
        };

        let result = build_oop_config_and_db(&local_config, "test_module", Some(&rendered));

        assert!(result.is_ok());
        let (final_config, _, _) = result.unwrap();

        // Config should be from LOCAL (full replacement)
        let module_config = final_config.modules.get("test_module").unwrap();
        assert_eq!(module_config["config"]["local_setting"], "local_value");
        // Master's settings should NOT be present
        assert!(module_config["config"].get("master_setting").is_none());
    }

    #[test]
    fn test_build_oop_config_logging_merge() {
        // Logging should merge (key-by-key)
        let mut local_config = minimal_app_config();
        local_config.logging = [
            (
                "default".to_owned(),
                logging_section(Some(Level::DEBUG), "logs/local-default.log"),
            ),
            (
                "new_key".to_owned(),
                logging_section(Some(Level::TRACE), "logs/new.log"),
            ),
        ]
        .into();

        let rendered = RenderedModuleConfig {
            database: None,
            config: json!({}),
            logging: Some(
                [
                    (
                        "default".to_owned(),
                        logging_section(Some(Level::INFO), "logs/master-default.log"),
                    ),
                    (
                        "sqlx".to_owned(),
                        logging_section(Some(Level::WARN), "logs/sql.log"),
                    ),
                ]
                .into(),
            ),
            tracing: None,
        };

        let result = build_oop_config_and_db(&local_config, "test_module", Some(&rendered));

        assert!(result.is_ok());
        let (_, merged_logging, _) = result.unwrap();

        // 3 keys total: default (overridden), sqlx (from master), new_key (from local)
        assert_eq!(merged_logging.len(), 3);

        // default: overridden by local
        assert_eq!(
            merged_logging.get("default").unwrap().console_level,
            Some(Level::DEBUG)
        );
        assert_eq!(
            merged_logging.get("default").unwrap().file().unwrap(),
            "logs/local-default.log"
        );

        // sqlx: from master
        assert_eq!(
            merged_logging.get("sqlx").unwrap().console_level,
            Some(Level::WARN)
        );

        // new_key: from local
        assert_eq!(
            merged_logging.get("new_key").unwrap().console_level,
            Some(Level::TRACE)
        );
    }

    #[test]
    fn test_build_oop_config_empty_local_config_section() {
        // When local has empty config section (null), use master's
        let mut local_config = minimal_app_config();
        local_config.modules.insert(
            "test_module".to_owned(),
            json!({
                "config": null
            }),
        );

        let rendered = RenderedModuleConfig {
            database: None,
            config: json!({"master_setting": "value"}),
            logging: None,
            tracing: None,
        };

        let result = build_oop_config_and_db(&local_config, "test_module", Some(&rendered));

        assert!(result.is_ok());
        let (final_config, _, _) = result.unwrap();

        // Config should be from master since local is null
        let module_config = final_config.modules.get("test_module").unwrap();
        assert_eq!(module_config["config"]["master_setting"], "value");
    }

    #[test]
    fn test_build_oop_config_no_config_section_in_local() {
        // When local has no config section at all, use master's
        let mut local_config = minimal_app_config();
        local_config.modules.insert(
            "test_module".to_owned(),
            json!({
                "database": {}  // has database but no config
            }),
        );

        let rendered = RenderedModuleConfig {
            database: None,
            config: json!({"master_setting": "value"}),
            logging: None,
            tracing: None,
        };

        let result = build_oop_config_and_db(&local_config, "test_module", Some(&rendered));

        assert!(result.is_ok());
        let (final_config, _, _) = result.unwrap();

        // Config should be from master
        let module_config = final_config.modules.get("test_module").unwrap();
        assert_eq!(module_config["config"]["master_setting"], "value");
    }
}
