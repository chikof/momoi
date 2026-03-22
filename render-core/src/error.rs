//! Rendering error types.

use thiserror::Error;

/// Errors that can occur during rendering operations.
#[allow(missing_docs)]
#[derive(Debug, Error)]
pub enum RenderError {
    /// Backend initialisation failed.
    #[error("backend initialisation failed: {0}")]
    InitFailed(String),

    /// A surface could not be created for the given output.
    #[error("surface creation failed for output '{output}': {reason}")]
    SurfaceCreation { output: String, reason: String },

    /// Frame submission to the compositor failed.
    #[error("frame submission failed: {0}")]
    FrameSubmission(String),

    /// Shader compilation or linking error.
    #[error("shader error in '{name}': {detail}")]
    Shader { name: String, detail: String },

    /// A texture operation failed.
    #[error("texture error: {0}")]
    Texture(String),

    /// The requested output is not available.
    #[error("output not found: {0}")]
    OutputNotFound(String),

    /// Any other backend-specific error.
    #[error("backend error: {0}")]
    Backend(#[from] Box<dyn std::error::Error + Send + Sync>),
}
