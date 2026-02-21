//! `CredStore` Gateway Module
//!
//! Implements the `CredStore` gateway module that:
//! 1. Registers the `CredStorePluginSpecV1` schema in types-registry
//! 2. Discovers plugin instances via types-registry (lazy, first-use)
//! 3. Routes `get`/`put`/`delete` calls through the selected plugin
//! 4. Implements hierarchical tenant secret resolution via tenant-resolver
//! 5. Registers `Arc<dyn CredStoreClient>` in `ClientHub` for consumers
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

pub mod api;
pub mod config;
pub mod domain;
pub mod module;
