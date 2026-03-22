//! # config-system
//!
//! TOML-based configuration with live reload via `notify`.

pub mod error;
pub mod loader;
pub mod schema;
pub mod watcher;

pub use error::ConfigError;
pub use loader::ConfigLoader;
pub use schema::{AudioConfig, DaemonConfig, OutputConfig, OverlayConfig, WallpaperConfig};
pub use watcher::ConfigWatcher;
