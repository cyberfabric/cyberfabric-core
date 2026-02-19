//! Host/in-process bootstrap module
//!
//! This module provides logging initialization, signal handling,
//! and path utilities for host processes.
//!
//! Configuration types are now in the top-level `config` module.

pub mod logging;
pub mod observability;
pub mod paths;
pub mod signals;

pub use logging::*;
pub use observability::*;
pub use paths::{HomeDirError, expand_tilde, normalize_path};
pub use signals::*;
