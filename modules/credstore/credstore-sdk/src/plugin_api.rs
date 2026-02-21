use async_trait::async_trait;

use crate::error::CredStoreError;
use crate::models::{OwnerId, SecretMetadata, SecretRef, SecretValue, SharingMode, TenantId};

/// Backend storage adapter trait implemented by credential store plugins.
///
/// Plugins operate at the single-tenant level with explicit parameters
/// decomposed by the gateway. No `SecurityContext` — authorization is
/// the gateway's responsibility.
#[async_trait]
pub trait CredStorePluginClientV1: Send + Sync {
    /// Retrieves a secret with full metadata from the backend.
    ///
    /// When `owner_id` is `Some`, looks up the private secret for that owner.
    /// When `None`, looks up the tenant/shared secret.
    async fn get(
        &self,
        tenant_id: &TenantId,
        key: &SecretRef,
        owner_id: Option<&OwnerId>,
    ) -> Result<Option<SecretMetadata>, CredStoreError>;

    /// Stores a secret in the backend.
    async fn put(
        &self,
        tenant_id: &TenantId,
        key: &SecretRef,
        value: SecretValue,
        sharing: SharingMode,
        owner_id: OwnerId,
    ) -> Result<(), CredStoreError>;

    /// Deletes a secret from the backend.
    ///
    /// When `owner_id` is `Some`, deletes the private secret for that owner.
    /// When `None`, deletes the tenant/shared secret.
    ///
    /// # Errors
    ///
    /// Returns `CredStoreError::NotFound` if no matching secret exists.
    async fn delete(
        &self,
        tenant_id: &TenantId,
        key: &SecretRef,
        owner_id: Option<&OwnerId>,
    ) -> Result<(), CredStoreError>;
}
