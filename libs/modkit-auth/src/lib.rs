#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![warn(warnings)]

// Core modules
pub mod claims;
pub mod errors;
pub mod http_error;
pub mod traits;
pub mod types;

pub mod authorizer;

// Plugin system modules
pub mod auth_mode;
pub mod claims_error;
pub mod config;
pub mod config_error;
pub mod dispatcher;
pub mod metrics;
pub mod plugin_traits;
pub mod plugins;
pub mod providers;
pub mod standard_claims;
pub mod validation;

// Outbound OAuth2 client credentials
pub mod oauth2;

// Core exports
pub use claims::Claims;
pub use errors::AuthError;
pub use traits::TokenValidator;
pub use types::{AuthRequirement, RoutePolicy, SecRequirement};

// Plugin system exports
pub use auth_mode::{AuthModeConfig, PluginRegistry};
pub use claims_error::ClaimsError;
pub use config::{AuthConfig, JwksConfig, PluginConfig, build_auth_dispatcher};
pub use config_error::ConfigError;
pub use dispatcher::AuthDispatcher;
pub use metrics::{AuthEvent, AuthMetricLabels, AuthMetrics, LoggingMetrics, NoOpMetrics};
pub use plugin_traits::{ClaimsPlugin, IntrospectionProvider, KeyProvider};
pub use standard_claims::StandardClaim;
pub use validation::ValidationConfig;

// Outbound OAuth2 exports
pub use oauth2::{
    BearerAuthLayer, ClientAuthMethod, HttpClientBuilderExt, OAuthClientConfig, SecretString,
    Token, TokenError,
};
