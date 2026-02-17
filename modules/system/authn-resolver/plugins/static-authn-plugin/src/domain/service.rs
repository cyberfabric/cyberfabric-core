//! Service implementation for the static `AuthN` resolver plugin.

use std::collections::HashMap;

use modkit_macros::domain_model;
use modkit_security::SecurityContext;

use crate::config::{AuthNMode, IdentityConfig, StaticAuthNPluginConfig};
use authn_resolver_sdk::AuthenticationResult;

/// Static `AuthN` resolver service.
///
/// Provides token-to-identity mapping based on configuration mode:
/// - `accept_all`: Any non-empty token maps to the default identity
/// - `static_tokens`: Specific tokens map to specific identities
#[domain_model]
pub struct Service {
    mode: AuthNMode,
    default_identity: IdentityConfig,
    token_map: HashMap<String, IdentityConfig>,
}

impl Service {
    /// Create a service from plugin configuration.
    #[must_use]
    pub fn from_config(cfg: &StaticAuthNPluginConfig) -> Self {
        let token_map: HashMap<String, IdentityConfig> = cfg
            .tokens
            .iter()
            .map(|m| (m.token.clone(), m.identity.clone()))
            .collect();

        Self {
            mode: cfg.mode.clone(),
            default_identity: cfg.default_identity.clone(),
            token_map,
        }
    }

    /// Authenticate a bearer token and return the identity.
    ///
    /// Returns `None` if the token is not recognized (in `static_tokens` mode)
    /// or empty.
    #[must_use]
    pub fn authenticate(&self, bearer_token: &str) -> Option<AuthenticationResult> {
        if bearer_token.is_empty() {
            return None;
        }

        let identity = match &self.mode {
            AuthNMode::AcceptAll => &self.default_identity,
            AuthNMode::StaticTokens => self.token_map.get(bearer_token)?,
        };

        build_result(identity, bearer_token)
    }
}

fn build_result(identity: &IdentityConfig, bearer_token: &str) -> Option<AuthenticationResult> {
    let ctx = SecurityContext::builder()
        .subject_id(identity.subject_id)
        .subject_tenant_id(identity.subject_tenant_id)
        .token_scopes(identity.token_scopes.clone())
        .bearer_token(bearer_token.to_owned())
        .build()
        .map_err(|e| tracing::error!("Failed to build SecurityContext from config: {e}"))
        .ok()?;

    Some(AuthenticationResult {
        security_context: ctx,
    })
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use secrecy::ExposeSecret;

    use super::*;
    use crate::config::TokenMapping;
    use uuid::Uuid;

    fn default_config() -> StaticAuthNPluginConfig {
        StaticAuthNPluginConfig::default()
    }

    #[test]
    fn accept_all_mode_returns_default_identity() {
        let service = Service::from_config(&default_config());

        let result = service.authenticate("any-token-value");
        assert!(result.is_some());

        let auth = result.unwrap();
        let ctx = &auth.security_context;
        assert_eq!(
            ctx.subject_id(),
            modkit_security::constants::DEFAULT_SUBJECT_ID
        );
        assert_eq!(
            ctx.subject_tenant_id(),
            modkit_security::constants::DEFAULT_TENANT_ID
        );
        assert_eq!(ctx.token_scopes(), &["*"]);
        assert_eq!(
            ctx.bearer_token().map(ExposeSecret::expose_secret),
            Some("any-token-value"),
        );
    }

    #[test]
    fn accept_all_mode_rejects_empty_token() {
        let service = Service::from_config(&default_config());

        let result = service.authenticate("");
        assert!(result.is_none());
    }

    #[test]
    fn static_tokens_mode_returns_mapped_identity() {
        let user_a_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let tenant_a = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();

        let cfg = StaticAuthNPluginConfig {
            mode: AuthNMode::StaticTokens,
            tokens: vec![TokenMapping {
                token: "token-user-a".to_owned(),
                identity: IdentityConfig {
                    subject_id: user_a_id,
                    subject_tenant_id: tenant_a,
                    token_scopes: vec!["read:data".to_owned()],
                },
            }],
            ..default_config()
        };

        let service = Service::from_config(&cfg);

        let result = service.authenticate("token-user-a");
        assert!(result.is_some());

        let auth = result.unwrap();
        let ctx = &auth.security_context;
        assert_eq!(ctx.subject_id(), user_a_id);
        assert_eq!(ctx.subject_tenant_id(), tenant_a);
        assert_eq!(ctx.token_scopes(), &["read:data"]);
        assert_eq!(
            ctx.bearer_token().map(ExposeSecret::expose_secret),
            Some("token-user-a"),
        );
    }

    #[test]
    fn static_tokens_mode_rejects_unknown_token() {
        let cfg = StaticAuthNPluginConfig {
            mode: AuthNMode::StaticTokens,
            tokens: vec![TokenMapping {
                token: "known-token".to_owned(),
                identity: IdentityConfig::default(),
            }],
            ..default_config()
        };

        let service = Service::from_config(&cfg);

        let result = service.authenticate("unknown-token");
        assert!(result.is_none());
    }

    #[test]
    fn static_tokens_mode_rejects_empty_token() {
        let cfg = StaticAuthNPluginConfig {
            mode: AuthNMode::StaticTokens,
            tokens: vec![],
            ..default_config()
        };

        let service = Service::from_config(&cfg);

        let result = service.authenticate("");
        assert!(result.is_none());
    }
}
