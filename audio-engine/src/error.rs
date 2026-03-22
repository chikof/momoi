//! Audio subsystem error types.
use thiserror::Error;

/// Errors from the audio capture / analysis pipeline.
#[derive(Debug, Error)]
pub enum AudioError {
    /// `PipeWire` could not be initialised.
    #[error("PipeWire init failed: {0}")]
    PipeWireInit(String),

    /// The audio stream disconnected unexpectedly.
    #[error("audio stream disconnected")]
    StreamDisconnected,

    /// FFT configuration is invalid.
    #[error("FFT configuration error: {0}")]
    FftConfig(String),
}
