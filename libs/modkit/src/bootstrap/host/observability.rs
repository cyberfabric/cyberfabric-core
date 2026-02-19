//! tokio-console observability support (feature-gated behind `tokio-console`).
//!
//! Provides async runtime inspection via [`console-subscriber`](https://docs.rs/console-subscriber).
//! The layer is only built when the `tokio-console` Cargo feature is enabled
//! **and** a `tokio_console` section is present in the application config.

use super::super::config::TokioConsoleConfig;

// ========== tokio-console-agnostic layer type (compiles with/without the feature) ==========
#[cfg(feature = "tokio-console")]
pub type TokioConsoleLayer = console_subscriber::ConsoleLayer;
#[cfg(not(feature = "tokio-console"))]
pub type TokioConsoleLayer = ();

/// Build a [`TokioConsoleLayer`] from configuration.
///
/// Returns `Some(layer)` when the `tokio-console` feature is compiled in and
/// `config` is `Some`; returns `None` otherwise.
#[cfg(feature = "tokio-console")]
#[must_use]
pub fn build_console_layer(config: Option<&TokioConsoleConfig>) -> Option<TokioConsoleLayer> {
    config.map(|tc| {
        console_subscriber::ConsoleLayer::builder()
            .server_addr(
                tc.server_addr
                    .parse::<std::net::SocketAddr>()
                    .expect("invalid tokio_console.server_addr"),
            )
            .spawn()
    })
}

/// No-op fallback when the `tokio-console` feature is disabled.
#[cfg(not(feature = "tokio-console"))]
#[must_use]
pub fn build_console_layer(_config: Option<&TokioConsoleConfig>) -> Option<TokioConsoleLayer> {
    None
}
