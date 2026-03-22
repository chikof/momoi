//! CPU renderer errors.
use thiserror::Error;

/// Errors specific to the CPU rendering backend.
#[derive(Debug, Error)]
pub enum CpuError {
    /// Image decoding failed.
    #[error("image decode error: {0}")]
    ImageDecode(#[from] image::ImageError),
}
