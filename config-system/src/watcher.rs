//! Live-reload watcher for the main config file.

use crate::{ConfigError, ConfigLoader, DaemonConfig};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::RwLock;
use std::{path::Path, sync::Arc};
use tokio::sync::mpsc;
use tracing::{error, info};

/// Watches the config file and atomically reloads it on change.
pub struct ConfigWatcher {
    _watcher: RecommendedWatcher,
    /// Always-current config; updated atomically after every successful reload.
    pub config: Arc<RwLock<DaemonConfig>>,
}

impl ConfigWatcher {
    /// Start watching `path`.
    ///
    /// `tx` receives `()` after every successful reload so callers can react.
    ///
    /// # Errors
    /// Returns [`ConfigError::DirNotFound`] if the OS watcher cannot start.
    pub fn start(
        path: &Path,
        initial: DaemonConfig,
        tx: mpsc::Sender<()>,
    ) -> Result<Self, ConfigError> {
        let config = Arc::new(RwLock::new(initial));
        let config_clone = config.clone();
        let path_clone = path.to_path_buf();

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res
                && matches!(event.kind, EventKind::Modify(_))
            {
                match ConfigLoader::load_from(&path_clone) {
                    Ok(new_cfg) => {
                        *config_clone.write() = new_cfg;
                        info!("config reloaded");
                        let _ = tx.try_send(());
                    }
                    Err(e) => error!(error = %e, "config reload failed"),
                }
            }
        })
        .map_err(|e| ConfigError::DirNotFound(e.to_string()))?;

        watcher
            .watch(path, RecursiveMode::NonRecursive)
            .map_err(|e| ConfigError::DirNotFound(e.to_string()))?;

        Ok(Self {
            _watcher: watcher,
            config,
        })
    }
}
