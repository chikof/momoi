//! GPU rendering operations modules.
//!
//! This module contains specialized rendering operations for different wallpaper types:
//! - **shaders**: Procedural shader effects (plasma, waves, etc.)
//! - **images**: Image scaling and texture operations
//! - **blending**: Frame blending for transitions

pub mod blending;
pub mod images;
pub mod shaders;

// Re-export commonly used functions
pub use blending::{blend_frames, blend_frames_cached};
pub use images::{render_image, render_image_argb};
pub use shaders::{render_shader, render_shader_cached};
