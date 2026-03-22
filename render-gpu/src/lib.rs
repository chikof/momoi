//! # render-gpu
//!
//! `wgpu`-based GPU renderer for momoi.
//! Renders each frame into an offscreen RGBA texture and reads pixels back
//! for submission to the Wayland surface buffer.

pub mod error;
pub mod renderer;
pub mod uniform_buffer;

pub use error::GpuError;
pub use renderer::GpuRenderer;
