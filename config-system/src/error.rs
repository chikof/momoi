//! Configuration error types.
use thiserror::Error;

/// Errors from the configuration subsystem.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// A config file could not be read from disk.
    #[error("io error reading '{path}': {source}")]
    Io {
        /// The file path that failed to read.
        path: String,
        /// The underlying OS error.
        #[source]
        source: std::io::Error,
    },

    /// The file contents are not valid TOML or don't match the schema.
    #[error("toml parse error in '{path}': {detail}")]
    Toml {
        /// The file path that failed to parse.
        path: String,
        /// Human-readable parse error message.
        detail: String,
    },

    /// The config directory cannot be determined.
    #[error("config directory not found: {0}")]
    DirNotFound(String),

    /// A required configuration field is absent.
    #[error("missing required field '{0}'")]
    MissingField(String),
}
