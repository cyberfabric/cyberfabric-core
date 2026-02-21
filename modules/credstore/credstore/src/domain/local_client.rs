//! Local (in-process) client for the credstore module.

use std::sync::Arc;

use async_trait::async_trait;
use credstore_sdk::{
    CredStoreClientV1, CredStoreError, GetSecretResponse, SecretRef, SecretValue, SharingMode,
};
use modkit_macros::domain_model;
use modkit_security::SecurityContext;

use super::{DomainError, Service};

/// Local client wrapping the credstore service.
///
/// Registered in `ClientHub` by the credstore module during `init()`.
#[domain_model]
pub struct CredStoreLocalClient {
    svc: Arc<Service>,
}

impl CredStoreLocalClient {
    /// Creates a new local client wrapping the given service.
    #[must_use]
    pub fn new(svc: Arc<Service>) -> Self {
        Self { svc }
    }
}

fn log_and_convert(op: &str, e: DomainError) -> CredStoreError {
    tracing::error!(operation = op, error = ?e, "credstore call failed");
    e.into()
}

#[async_trait]
impl CredStoreClientV1 for CredStoreLocalClient {
    async fn get(
        &self,
        ctx: &SecurityContext,
        key: &SecretRef,
    ) -> Result<Option<GetSecretResponse>, CredStoreError> {
        self.svc
            .get(ctx, key)
            .await
            .map_err(|e| log_and_convert("get", e))
    }

    async fn put(
        &self,
        ctx: &SecurityContext,
        key: &SecretRef,
        value: SecretValue,
        sharing: SharingMode,
    ) -> Result<(), CredStoreError> {
        self.svc
            .put(ctx, key, value, sharing)
            .await
            .map_err(|e| log_and_convert("put", e))
    }

    async fn delete(&self, ctx: &SecurityContext, key: &SecretRef) -> Result<(), CredStoreError> {
        self.svc
            .delete(ctx, key)
            .await
            .map_err(|e| log_and_convert("delete", e))
    }
}
