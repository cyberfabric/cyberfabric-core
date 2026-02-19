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
///
/// Uses [`Builder::build`] to obtain a nameable [`ConsoleLayer`] and spawns the
/// gRPC [`Server`] on a dedicated background thread (same strategy as
/// [`Builder::spawn`], but avoids the opaque `impl Layer<S>` return type).
#[cfg(feature = "tokio-console")]
#[must_use]
pub fn build_console_layer(config: Option<&TokioConsoleConfig>) -> Option<TokioConsoleLayer> {
    let Some(tokio_config) = config else {
        tracing::warn!(
            "tokio_console config must not be None when `tokio-console` feature is enabled"
        );
        return None;
    };

    let addr = match tokio_config.server_addr.parse::<std::net::SocketAddr>() {
        Ok(addr) => addr,
        Err(err) => {
            tracing::error!(
                server_addr = %tokio_config.server_addr,
                error = %err,
                "invalid tokio_console.server_addr, tokio-console layer disabled"
            );
            return None;
        }
    };

    let (layer, server) = console_subscriber::ConsoleLayer::builder()
        .server_addr(addr)
        .build();

    if let Err(err) = std::thread::Builder::new()
        .name("console_subscriber".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .enable_time()
                .build()
            {
                Ok(rt) => rt,
                Err(err) => {
                    eprintln!("console subscriber runtime initialization failed: {err}");
                    return;
                }
            };
            runtime.block_on(async move {
                if let Err(err) = server.serve().await {
                    eprintln!("console subscriber server failed: {err}");
                }
            });
        })
    {
        tracing::error!(
            error = %err,
            "console subscriber could not spawn thread, tokio-console layer disabled"
        );
        return None;
    }

    Some(layer)
}

/// No-op fallback when the `tokio-console` feature is disabled.
#[cfg(not(feature = "tokio-console"))]
#[must_use]
pub fn build_console_layer(config: Option<&TokioConsoleConfig>) -> Option<TokioConsoleLayer> {
    if config.is_some() {
        tracing::warn!(
            "tokio_console section present in config but the `tokio-console` feature is disabled"
        );
    }
    None
}

#[cfg(test)]
#[path = "observability_tests.rs"]
mod tests;
