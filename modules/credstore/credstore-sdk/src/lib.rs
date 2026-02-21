//! `CredStore` SDK
//!
//! This crate provides the public API for the `credstore` module:
//!
//! - [`CredStoreClient`] — Consumer API trait for storing/retrieving secrets
//! - [`CredStorePluginClientV1`] — Plugin API trait for backend storage adapters
//! - [`SecretRef`], [`SecretValue`], [`SharingMode`], [`GetSecretResponse`], [`SecretMetadata`] — Domain models
//! - [`CredStoreError`] — Error types
//! - [`CredStorePluginSpecV1`] — GTS schema for plugin discovery
//!
//! # Usage
//!
//! ```rust,ignore
//! use credstore_sdk::{CredStoreClient, SecretRef, SecretValue, SharingMode};
//!
//! async fn store_secret(client: &dyn CredStoreClient, ctx: &SecurityContext) {
//!     let key = SecretRef::new("partner-openai-key").unwrap();
//!     let value = SecretValue::from("sk-abc123");
//!
//!     client.put(ctx, &key, value, SharingMode::Tenant).await.unwrap();
//!
//!     if let Some(resp) = client.get(ctx, &key).await.unwrap() {
//!         // Use resp.value.as_bytes()
//!         // Check resp.is_inherited, resp.sharing, resp.owner_tenant_id
//!     }
//! }
//! ```

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

pub mod api;
pub mod error;
pub mod gts;
pub mod models;
pub mod plugin_api;

// Re-export main types at crate root
pub use api::CredStoreClientV1;
pub use error::CredStoreError;
pub use gts::CredStorePluginSpecV1;
pub use models::{
    GetSecretResponse, OwnerId, SecretMetadata, SecretRef, SecretValue, SharingMode, TenantId,
};
pub use plugin_api::CredStorePluginClientV1;
