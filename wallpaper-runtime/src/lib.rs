//! # wallpaper-runtime
//!
//! Per-output render loop plus specialised wallpaper renderer types.

pub mod context;
pub mod error;
pub mod image_wallpaper;
pub mod runner;
pub mod time;
pub mod time_wallpaper;

pub use context::WallpaperContext;
pub use error::RuntimeError;
pub use image_wallpaper::ImageRenderer;
pub use runner::{RenderedFrame, WallpaperRunner};
pub use time_wallpaper::{TimeBasedRenderer, TimeConfig};
