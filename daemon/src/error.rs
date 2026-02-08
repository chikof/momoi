//! Unified error handling for the Momoi daemon.
//!
//! This module provides structured error types for all daemon operations,
//! using `thiserror` for automatic trait implementations and error chaining.
//!
//! # Error Categories
//!
//! - **Config**: Configuration loading and validation errors
//! - **Ipc**: IPC server and client communication errors
//! - **Wayland**: Wayland protocol and connection errors
//! - **Gpu**: GPU initialization and rendering errors
//! - **Image**: Image loading, decoding, and processing errors
//! - **Video**: Video playback and decoding errors
//! - **Io**: File system and socket I/O errors
//!
//! # Examples
//!
//! ```
//! use momoi::error::{MomoiError, Result};
//!
//! fn load_wallpaper(path: &str) -> Result<()> {
//!     // Returns MomoiError::Image on failure
//!     Ok(())
//! }
//! ```

use std::path::PathBuf;
use thiserror::Error;

/// Unified result type for daemon operations.
pub type Result<T> = std::result::Result<T, MomoiError>;

/// Main error type for the Momoi daemon.
///
/// All daemon errors are variants of this enum, enabling structured error
/// handling and proper error chaining through `source()`.
#[derive(Error, Debug)]
pub enum MomoiError {
    // ==================== Configuration Errors ====================
    /// Configuration file loading or parsing error
    #[error("Configuration error: {message}")]
    Config {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Configuration validation error (invalid values)
    #[error("Invalid configuration: {field} - {reason}")]
    ConfigValidation { field: String, reason: String },

    // ==================== IPC Errors ====================
    /// IPC server initialization error
    #[error("Failed to start IPC server: {0}")]
    IpcServer(String),

    /// IPC client connection error
    #[error("IPC connection error: {0}")]
    IpcConnection(String),

    /// IPC message serialization/deserialization error
    #[error("IPC protocol error: {0}")]
    IpcProtocol(#[from] serde_json::Error),

    // ==================== Wayland Errors ====================
    /// Wayland connection or initialization error
    #[error("Wayland connection error: {0}")]
    WaylandConnection(String),

    /// Wayland protocol error (compositor communication)
    #[error("Wayland protocol error: {0}")]
    WaylandProtocol(String),

    /// Output/display configuration error
    #[error("Output error for '{output}': {message}")]
    Output { output: String, message: String },

    // ==================== GPU/Rendering Errors ====================
    /// GPU initialization or context creation error
    #[error("GPU initialization failed: {0}")]
    GpuInit(String),

    /// Shader compilation or loading error
    #[error("Shader error: {shader} - {message}")]
    Shader { shader: String, message: String },

    /// GPU rendering operation error
    #[error("Rendering error: {0}")]
    Rendering(String),

    // ==================== Image Errors ====================
    /// Image file loading error
    #[error("Failed to load image '{path}': {reason}")]
    ImageLoad {
        path: PathBuf,
        reason: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Image format/decoding error
    #[error("Image format error: {0}")]
    ImageFormat(String),

    /// Image processing error (resize, convert, etc.)
    #[error("Image processing error: {operation} - {reason}")]
    ImageProcessing { operation: String, reason: String },

    // ==================== Video Errors ====================
    /// Video file loading error
    #[error("Failed to load video '{path}': {reason}")]
    VideoLoad {
        path: PathBuf,
        reason: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// GStreamer pipeline error
    #[error("GStreamer error: {0}")]
    GStreamer(String),

    /// Video decoding or playback error
    #[error("Video playback error: {0}")]
    VideoPlayback(String),

    // ==================== I/O Errors ====================
    /// File system I/O error
    #[error("I/O error: {operation} - {path}")]
    Io {
        operation: String,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Unix socket error
    #[error("Socket error: {0}")]
    Socket(#[from] std::io::Error),

    // ==================== General Errors ====================
    /// Resource not found (file, output, preset, etc.)
    #[error("Not found: {resource_type} '{name}'")]
    NotFound { resource_type: String, name: String },

    /// Invalid input or parameter
    #[error("Invalid {parameter}: {reason}")]
    InvalidInput { parameter: String, reason: String },

    /// Operation timeout
    #[error("Operation timed out: {operation} after {timeout_ms}ms")]
    Timeout { operation: String, timeout_ms: u64 },

    /// Generic error with context
    #[error("{0}")]
    Other(String),
}

// ==================== Convenience Constructors ====================

impl MomoiError {
    /// Create a configuration error with source
    pub fn config(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Config {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Create a configuration error without source
    pub fn config_msg(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
            source: None,
        }
    }

    /// Create an image loading error with source
    pub fn image_load(
        path: impl Into<PathBuf>,
        reason: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::ImageLoad {
            path: path.into(),
            reason: reason.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Create an image loading error without source
    pub fn image_load_msg(path: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
        Self::ImageLoad {
            path: path.into(),
            reason: reason.into(),
            source: None,
        }
    }

    /// Create a video loading error with source
    pub fn video_load(
        path: impl Into<PathBuf>,
        reason: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::VideoLoad {
            path: path.into(),
            reason: reason.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Create a video loading error without source
    pub fn video_load_msg(path: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
        Self::VideoLoad {
            path: path.into(),
            reason: reason.into(),
            source: None,
        }
    }

    /// Create an I/O error
    pub fn io(
        operation: impl Into<String>,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        Self::Io {
            operation: operation.into(),
            path: path.into(),
            source,
        }
    }

    /// Create a not found error
    pub fn not_found(resource_type: impl Into<String>, name: impl Into<String>) -> Self {
        Self::NotFound {
            resource_type: resource_type.into(),
            name: name.into(),
        }
    }

    /// Create an invalid input error
    pub fn invalid_input(parameter: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidInput {
            parameter: parameter.into(),
            reason: reason.into(),
        }
    }
}

// ==================== Conversions from anyhow ====================

impl From<anyhow::Error> for MomoiError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err.to_string())
    }
}

// ==================== Conversion to common::WallpaperError for IPC ====================

impl From<MomoiError> for common::WallpaperError {
    fn from(err: MomoiError) -> Self {
        match err {
            MomoiError::Config { message, .. } => Self::Io(message),
            MomoiError::ConfigValidation { field, reason } => {
                Self::Io(format!("Invalid config {}: {}", field, reason))
            }
            MomoiError::IpcServer(msg) | MomoiError::IpcConnection(msg) => Self::Ipc(msg),
            MomoiError::IpcProtocol(e) => Self::Ipc(e.to_string()),
            MomoiError::WaylandConnection(msg) | MomoiError::WaylandProtocol(msg) => {
                Self::Wayland(msg)
            }
            MomoiError::Output { output, message } => {
                Self::Wayland(format!("Output '{}': {}", output, message))
            }
            MomoiError::GpuInit(msg)
            | MomoiError::Shader { message: msg, .. }
            | MomoiError::Rendering(msg) => Self::Wayland(format!("GPU error: {}", msg)),
            MomoiError::ImageLoad { path, reason, .. } => {
                Self::Image(format!("{}: {}", path.display(), reason))
            }
            MomoiError::ImageFormat(reason) => Self::Image(reason),
            MomoiError::ImageProcessing { reason, .. } => Self::Image(reason),
            MomoiError::VideoLoad { path, reason, .. } => {
                Self::Video(format!("{}: {}", path.display(), reason))
            }
            MomoiError::GStreamer(reason) => Self::Video(reason),
            MomoiError::VideoPlayback(reason) => Self::Video(reason),
            MomoiError::Io {
                operation,
                path,
                source,
            } => Self::Io(format!("{} ({}): {}", operation, path.display(), source)),
            MomoiError::Socket(e) => Self::Io(e.to_string()),
            MomoiError::NotFound {
                resource_type,
                name,
            } => Self::NotFound(format!("{}: {}", resource_type, name)),
            MomoiError::InvalidInput { parameter, reason } => {
                Self::Io(format!("Invalid {}: {}", parameter, reason))
            }
            MomoiError::Timeout {
                operation,
                timeout_ms,
            } => Self::Io(format!("Timeout: {} after {}ms", operation, timeout_ms)),
            MomoiError::Other(msg) => Self::Io(msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_error() {
        let err = MomoiError::config_msg("Failed to parse TOML");
        assert!(err.to_string().contains("Configuration error"));
    }

    #[test]
    fn test_image_load_error() {
        let err = MomoiError::image_load_msg("/tmp/test.png", "File not found");
        assert!(err.to_string().contains("/tmp/test.png"));
        assert!(err.to_string().contains("File not found"));
    }

    #[test]
    fn test_not_found_error() {
        let err = MomoiError::not_found("shader preset", "sunset");
        assert!(err.to_string().contains("shader preset"));
        assert!(err.to_string().contains("sunset"));
    }

    #[test]
    fn test_invalid_input_error() {
        let err = MomoiError::invalid_input("color", "invalid hex format");
        assert!(err.to_string().contains("color"));
        assert!(err.to_string().contains("invalid hex format"));
    }

    #[test]
    fn test_conversion_to_wallpaper_error() {
        let err = MomoiError::not_found("output", "DP-1");
        let wall_err: common::WallpaperError = err.into();
        assert!(matches!(wall_err, common::WallpaperError::NotFound(_)));
    }
}
