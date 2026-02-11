use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Configuration for the OAGW module.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OagwConfig {
    #[serde(default = "default_proxy_timeout_secs")]
    pub proxy_timeout_secs: u64,
    #[serde(default = "default_max_body_size_bytes")]
    pub max_body_size_bytes: usize,
    /// Optional credentials to pre-load into the in-memory credential resolver.
    /// Keys are secret references (e.g., `cred://openai-key`), values are secrets.
    /// Intended for development and testing only.
    #[serde(default)]
    pub credentials: HashMap<String, String>,
}

impl Default for OagwConfig {
    fn default() -> Self {
        Self {
            proxy_timeout_secs: default_proxy_timeout_secs(),
            max_body_size_bytes: default_max_body_size_bytes(),
            credentials: HashMap::new(),
        }
    }
}

fn default_proxy_timeout_secs() -> u64 {
    30
}

fn default_max_body_size_bytes() -> usize {
    10 * 1024 * 1024 // 10 MB
}
