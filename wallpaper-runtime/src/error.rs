//! Runtime error types.
use thiserror::Error;

/// Errors that can arise during wallpaper execution.
#[derive(Debug, Error)]
pub enum RuntimeError {
    /// Renderer returned an error.
    #[error("render error: {0}")]
    Render(#[from] render_core::RenderError),

    /// Audio source disconnected or failed.
    #[error("audio error: {0}")]
    Audio(#[from] audio_engine::AudioError),

    /// Overlay compositing failed.
    #[error("overlay error: {0}")]
    Overlay(#[from] overlay_system::OverlayError),
}
