//! Wayland backend error types.

use thiserror::Error;

/// All errors that can arise from the Wayland integration layer.
#[derive(Debug, Error)]
pub enum WaylandError {
    /// Failed to connect to the Wayland display socket.
    #[error("failed to connect to Wayland display: {0}")]
    Connect(String),

    /// A required Wayland global (protocol extension) was not advertised.
    #[error("required Wayland global missing: {0}")]
    GlobalMissing(String),

    /// Layer-shell surface creation or configuration failed.
    #[error("layer surface error on output '{output}': {detail}")]
    LayerSurface {
        /// Name of the affected output.
        output: String,
        /// Description of the failure.
        detail: String,
    },

    /// `wl_shm` pool or buffer allocation failed.
    #[error("shm allocation error: {0}")]
    ShmAlloc(String),

    /// linux-dmabuf negotiation or import failed.
    #[error("dmabuf error: {0}")]
    Dmabuf(String),

    /// The Wayland event loop returned an error.
    #[error("event loop error: {0}")]
    EventLoop(String),

    /// I/O error (e.g. creating the shm memfd).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Trying to convert a number
    #[error("num error: {0}")]
    FromInt(#[from] std::num::TryFromIntError),
}
