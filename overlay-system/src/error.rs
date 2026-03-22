//! Overlay error types.
use thiserror::Error;

/// Errors from the overlay subsystem.
#[derive(Debug, Error)]
pub enum OverlayError {
    /// Rendering a widget failed.
    #[error("widget render error: {0}")]
    Render(String),
}
