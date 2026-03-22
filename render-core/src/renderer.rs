//! Core renderer trait definitions.

use crate::{Frame, FrameStats, RenderError, ShaderUniforms, SurfaceDescriptor};
use parking_lot::Mutex;
use std::sync::Arc;

/// Identifies which rendering backend to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderBackend {
    /// Hardware-accelerated rendering via `wgpu`.
    Gpu,
    /// Pure software fallback renderer.
    Cpu,
}

/// The primary trait every rendering backend must implement.
pub trait Renderer: Send + Sync {
    /// One-time initialisation. Called once per output surface.
    ///
    /// # Errors
    /// Returns [`RenderError::InitFailed`] if the backend cannot initialise.
    fn init(&mut self, surface: &SurfaceDescriptor) -> Result<(), RenderError>;

    /// Render a single frame using the supplied uniforms.
    ///
    /// # Errors
    /// Returns [`RenderError::FrameSubmission`] if the frame cannot be presented.
    fn render_frame(
        &mut self,
        uniforms: &ShaderUniforms,
        stats: &mut FrameStats,
    ) -> Result<Frame, RenderError>;

    /// Called when the surface is resized.
    ///
    /// # Errors
    /// Returns [`RenderError::SurfaceCreation`] if the resize fails.
    fn resize(&mut self, width: u32, height: u32) -> Result<(), RenderError>;

    /// Human-readable backend identifier for logging.
    fn backend_name(&self) -> &'static str;
}

/// A type-erased, heap-allocated renderer shared across threads.
pub type DynRenderer = Arc<Mutex<Box<dyn Renderer>>>;
