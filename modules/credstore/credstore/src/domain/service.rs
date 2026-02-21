//! Domain service for the credstore module.
//!
//! Plugin discovery is lazy: resolved on first API call after
//! types-registry is ready.

use std::sync::Arc;
use std::time::Duration;

use credstore_sdk::{
    CredStorePluginClientV1, CredStorePluginSpecV1, GetSecretResponse, OwnerId, SecretRef,
    SecretValue, SharingMode, TenantId,
};
use modkit::client_hub::{ClientHub, ClientScope};
use modkit::plugins::{GtsPluginSelector, choose_plugin_instance};
use modkit::telemetry::ThrottledLog;
use modkit_macros::domain_model;
use modkit_security::SecurityContext;
use tracing::info;
use types_registry_sdk::{ListQuery, TypesRegistryClient};

use super::error::DomainError;

/// Throttle interval for plugin unavailable warnings.
const UNAVAILABLE_LOG_THROTTLE: Duration = Duration::from_secs(10);

/// `CredStore` domain service.
///
/// Discovers plugins via types-registry and delegates storage operations.
#[domain_model]
pub struct Service {
    hub: Arc<ClientHub>,
    vendor: String,
    selector: GtsPluginSelector,
    unavailable_log_throttle: ThrottledLog,
}

impl Service {
    /// Creates a new service with lazy plugin resolution.
    #[must_use]
    pub fn new(hub: Arc<ClientHub>, vendor: String) -> Self {
        Self {
            hub,
            vendor,
            selector: GtsPluginSelector::new(),
            unavailable_log_throttle: ThrottledLog::new(UNAVAILABLE_LOG_THROTTLE),
        }
    }

    /// Lazily resolves and returns the plugin client.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::PluginNotFound` if no plugin is registered for the configured vendor.
    /// Returns `DomainError::PluginUnavailable` if the plugin client is not yet registered.
    async fn get_plugin(&self) -> Result<Arc<dyn CredStorePluginClientV1>, DomainError> {
        let instance_id = self.selector.get_or_init(|| self.resolve_plugin()).await?;
        let scope = ClientScope::gts_id(instance_id.as_ref());

        if let Some(client) = self
            .hub
            .try_get_scoped::<dyn CredStorePluginClientV1>(&scope)
        {
            Ok(client)
        } else {
            if self.unavailable_log_throttle.should_log() {
                tracing::warn!(
                    plugin_gts_id = %instance_id,
                    vendor = %self.vendor,
                    "CredStore plugin client not registered yet"
                );
            }
            Err(DomainError::PluginUnavailable {
                gts_id: instance_id.to_string(),
                reason: "client not registered yet".into(),
            })
        }
    }

    /// Resolves the plugin instance from types-registry.
    #[tracing::instrument(skip_all, fields(vendor = %self.vendor))]
    async fn resolve_plugin(&self) -> Result<String, DomainError> {
        info!("Resolving credstore plugin");

        let registry = self
            .hub
            .get::<dyn TypesRegistryClient>()
            .map_err(|e| DomainError::TypesRegistryUnavailable(e.to_string()))?;

        let plugin_type_id = CredStorePluginSpecV1::gts_schema_id().clone();

        let instances = registry
            .list(
                ListQuery::new()
                    .with_pattern(format!("{plugin_type_id}*"))
                    .with_is_type(false),
            )
            .await?;

        let gts_id = choose_plugin_instance::<CredStorePluginSpecV1>(
            &self.vendor,
            instances.iter().map(|e| (e.gts_id.as_str(), &e.content)),
        )?;
        info!(plugin_gts_id = %gts_id, "Selected credstore plugin instance");

        Ok(gts_id)
    }

    fn extract_context(ctx: &SecurityContext) -> (TenantId, OwnerId) {
        (ctx.subject_tenant_id(), ctx.subject_id())
    }

    /// Retrieves a secret from the plugin.
    ///
    /// Returns `Ok(None)` if the secret is not found (anti-enumeration).
    ///
    /// # Errors
    ///
    /// Returns a `DomainError` for plugin resolution or backend failures.
    #[tracing::instrument(skip_all, fields(key = %key))]
    pub async fn get(
        &self,
        ctx: &SecurityContext,
        key: &SecretRef,
    ) -> Result<Option<GetSecretResponse>, DomainError> {
        let (tenant_id, _owner_id) = Self::extract_context(ctx);
        let plugin = self.get_plugin().await?;

        let result = plugin.get(&tenant_id, key, None).await?;
        Ok(result.map(|meta| GetSecretResponse {
            value: meta.value,
            owner_tenant_id: meta.owner_tenant_id,
            sharing: meta.sharing,
            is_inherited: false,
        }))
    }

    /// Stores a secret in the caller's tenant.
    ///
    /// # Errors
    ///
    /// Returns a `DomainError` for plugin resolution or backend failures.
    #[tracing::instrument(skip_all, fields(key = %key))]
    pub async fn put(
        &self,
        ctx: &SecurityContext,
        key: &SecretRef,
        value: SecretValue,
        sharing: SharingMode,
    ) -> Result<(), DomainError> {
        let (tenant_id, owner_id) = Self::extract_context(ctx);
        let plugin = self.get_plugin().await?;
        plugin
            .put(&tenant_id, key, value, sharing, owner_id)
            .await?;
        Ok(())
    }

    /// Deletes a secret from the caller's tenant.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::NotFound` if no matching secret exists.
    #[tracing::instrument(skip_all, fields(key = %key))]
    pub async fn delete(&self, ctx: &SecurityContext, key: &SecretRef) -> Result<(), DomainError> {
        let (tenant_id, owner_id) = Self::extract_context(ctx);
        let plugin = self.get_plugin().await?;
        plugin
            .delete(&tenant_id, key, Some(&owner_id))
            .await
            .map_err(DomainError::from)
    }
}
