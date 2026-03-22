//! Filesystem watcher that triggers shader recompilation on file save.

use crate::{ShaderError, ShaderRegistry};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

/// Watches shader directories and drives hot-reload via [`ShaderRegistry`].
pub struct ShaderWatcher {
    _watcher: RecommendedWatcher,
}

impl ShaderWatcher {
    /// Start watching `watch_paths`, reloading shaders through `registry`.
    ///
    /// `event_tx` receives the shader name after every successful reload.
    ///
    /// # Errors
    /// Returns [`ShaderError::Watcher`] if the OS watcher cannot start.
    pub fn new(
        watch_paths: Vec<PathBuf>,
        registry: ShaderRegistry,
        event_tx: mpsc::Sender<String>,
    ) -> Result<Self, ShaderError> {
        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<Event>| match res {
                Ok(event) if matches!(event.kind, EventKind::Modify(_)) => {
                    for path in &event.paths {
                        let name = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown")
                            .to_owned();
                        match registry.reload(&name, path) {
                            Ok(_) => {
                                info!(shader = %name, "shader hot-reloaded");
                                let _ = event_tx.try_send(name);
                            }
                            Err(e) => error!(shader = %name, error = %e, "reload failed"),
                        }
                    }
                }
                Ok(_) => {}
                Err(e) => warn!(error = %e, "watcher error"),
            })
            .map_err(|e| ShaderError::Watcher(e.to_string()))?;

        for path in watch_paths {
            watcher
                .watch(&path, RecursiveMode::NonRecursive)
                .map_err(|e| ShaderError::Watcher(e.to_string()))?;
        }

        Ok(Self { _watcher: watcher })
    }
}
