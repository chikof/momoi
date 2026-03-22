//! Loads the daemon config from `~/.config/momoi/config.toml`.

use crate::{ConfigError, DaemonConfig};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Responsible for reading and deserialising config files.
#[derive(Debug, Default)]
pub struct ConfigLoader;

impl ConfigLoader {
    /// Return the default config directory: `$XDG_CONFIG_HOME/momoi`.
    ///
    /// # Errors
    /// Returns [`ConfigError::DirNotFound`] if the home directory is unknown.
    pub fn config_dir() -> Result<PathBuf, ConfigError> {
        dirs::config_dir()
            .map(|d| d.join("momoi"))
            .ok_or_else(|| ConfigError::DirNotFound("could not determine $HOME".into()))
    }

    /// Load config from an explicit file path.
    ///
    /// # Errors
    /// Returns [`ConfigError::Io`] on read failure or [`ConfigError::Toml`] on parse failure.
    pub fn load_from(path: &Path) -> Result<DaemonConfig, ConfigError> {
        debug!(path = %path.display(), "loading config");
        let text = std::fs::read_to_string(path).map_err(|e| ConfigError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        let config: DaemonConfig = toml::from_str(&text).map_err(|e| ConfigError::Toml {
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;
        info!(path = %path.display(), "config loaded");
        Ok(config)
    }

    /// Load from the default location, returning built-in defaults if the file is absent.
    ///
    /// # Errors
    /// Propagates errors other than file-not-found.
    pub fn load_or_default() -> Result<DaemonConfig, ConfigError> {
        let path = Self::config_dir()?.join("config.toml");
        if path.exists() {
            Self::load_from(&path)
        } else {
            info!("no config file found, using built-in defaults");
            Ok(DaemonConfig::default())
        }
    }
}
