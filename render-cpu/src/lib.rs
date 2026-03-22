//! # render-cpu
//!
//! Pure software renderer. Used as a fallback when no GPU is available.
//! Renders an animated gradient driven by `time` and `audio_bands[0]`
//! (bass energy).

#![deny(missing_docs)]

pub mod error;
pub mod renderer;

pub use error::CpuError;
pub use renderer::CpuRenderer;
