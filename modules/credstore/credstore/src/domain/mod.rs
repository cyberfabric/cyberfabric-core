//! Domain layer for the credstore module.

pub mod error;
pub mod local_client;
pub mod service;

pub use error::DomainError;
pub use local_client::CredStoreLocalClient;
pub use service::Service;
