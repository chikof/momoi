//! GPU renderer errors.
use thiserror::Error;

/// Errors specific to the GPU rendering backend.
#[derive(Debug, Error)]
pub enum GpuError {
    /// No suitable wgpu adapter was found.
    #[error("no suitable GPU adapter found")]
    NoAdapter,

    /// Device creation failed.
    #[error("GPU device creation failed: {0}")]
    DeviceCreation(String),

    /// Buffer readback failed.
    #[error("pixel readback failed: {0}")]
    Readback(String),
}
