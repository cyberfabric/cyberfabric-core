use serde::{Deserialize, Serialize};

fn default_require_auth_by_default() -> bool {
    true
}

fn default_prefix_path() -> String {
    "/cf".to_owned()
}

fn default_body_limit_bytes() -> usize {
    16 * 1024 * 1024
}

/// API gateway configuration - reused from `api_gateway` module
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct ApiGatewayConfig {
    pub bind_addr: String,
    #[serde(default)]
    pub enable_docs: bool,
    #[serde(default)]
    pub cors_enabled: bool,
    /// Optional detailed CORS configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cors: Option<CorsConfig>,

    /// `OpenAPI` document metadata
    #[serde(default)]
    pub openapi: OpenApiConfig,

    /// Global defaults
    #[serde(default)]
    pub defaults: Defaults,

    /// Disable authentication and authorization completely.
    /// When true, middleware automatically injects a default `SecurityContext` for all requests,
    /// providing access with no tenant filtering.
    /// This bypasses all tenant isolation and should only be used for single-user on-premise installations.
    /// Default: false (authentication required via `AuthN` Resolver).
    #[serde(default)]
    pub auth_disabled: bool,

    /// If true, routes without explicit security requirement still require authentication (AuthN-only).
    #[serde(default = "default_require_auth_by_default")]
    pub require_auth_by_default: bool,

    #[serde(default = "default_prefix_path")]
    pub prefix_path: String,
}

impl Default for ApiGatewayConfig {
    fn default() -> Self {
        Self {
            bind_addr: String::new(),
            enable_docs: false,
            cors_enabled: false,
            cors: None,
            openapi: OpenApiConfig::default(),
            defaults: Defaults::default(),
            auth_disabled: false,
            require_auth_by_default: default_require_auth_by_default(),
            prefix_path: default_prefix_path(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Defaults {
    /// Fallback rate limit when operation does not specify one
    pub rate_limit: RateLimitDefaults,
    /// Global request body size limit in bytes
    pub body_limit_bytes: usize,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            rate_limit: RateLimitDefaults::default(),
            body_limit_bytes: default_body_limit_bytes(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct RateLimitDefaults {
    pub rps: u32,
    pub burst: u32,
    pub in_flight: u32,
}

impl Default for RateLimitDefaults {
    fn default() -> Self {
        Self {
            rps: 50,
            burst: 100,
            in_flight: 64,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct CorsConfig {
    /// Allowed origins: `["*"]` means any
    pub allowed_origins: Vec<String>,
    /// Allowed HTTP methods, e.g. `["GET","POST","OPTIONS","PUT","DELETE","PATCH"]`
    pub allowed_methods: Vec<String>,
    /// Allowed request headers; `["*"]` means any
    pub allowed_headers: Vec<String>,
    /// Whether to allow credentials
    pub allow_credentials: bool,
    /// Max age for preflight caching in seconds
    pub max_age_seconds: u64,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: vec!["*".to_owned()],
            allowed_methods: vec![
                "GET".to_owned(),
                "POST".to_owned(),
                "PUT".to_owned(),
                "PATCH".to_owned(),
                "DELETE".to_owned(),
                "OPTIONS".to_owned(),
            ],
            allowed_headers: vec!["*".to_owned()],
            allow_credentials: false,
            max_age_seconds: 600,
        }
    }
}

/// `OpenAPI` document metadata configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct OpenApiConfig {
    /// API title shown in `OpenAPI` documentation
    pub title: String,
    /// API version
    pub version: String,
    /// API description (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl Default for OpenApiConfig {
    fn default() -> Self {
        Self {
            title: "API Documentation".to_owned(),
            version: "0.1.0".to_owned(),
            description: None,
        }
    }
}
