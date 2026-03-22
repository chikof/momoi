//! Shader error types.

use thiserror::Error;

/// Errors originating from the shader subsystem.
#[allow(missing_docs)]
#[derive(Debug, Error)]
pub enum ShaderError {
    /// WGSL or GLSL source could not be parsed.
    #[error("parse error in '{file}': {detail}")]
    Parse { file: String, detail: String },

    /// Shader failed naga validation.
    #[error("validation error in '{file}': {detail}")]
    Validation { file: String, detail: String },

    /// File I/O error reading a shader from disk.
    #[error("io error reading shader '{path}': {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// The requested shader is not registered.
    #[error("shader not found: '{0}'")]
    NotFound(String),

    /// Hot-reload watcher could not be initialised.
    #[error("watcher error: {0}")]
    Watcher(String),
}
