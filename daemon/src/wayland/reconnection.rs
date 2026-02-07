//! Wayland compositor reconnection logic.
//!
//! Handles automatic reconnection to the Wayland compositor with exponential backoff
//! when the compositor disconnects (broken pipe errors). This is useful for:
//! - Compositor crashes and restarts
//! - User switching compositors
//! - Wayland session restarts
//!
//! Configuration options:
//! - `enable_reconnection`: Enable/disable automatic reconnection
//! - `max_reconnection_retries`: Maximum number of reconnection attempts
//! - `initial_reconnection_backoff_ms`: Initial backoff delay
//! - `max_reconnection_backoff_ms`: Maximum backoff delay (exponential cap)

use crate::{DaemonState, WallpaperCommand};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

/// Run Wayland connection with automatic reconnection on broken pipe errors.
///
/// This function implements exponential backoff retry logic when the compositor
/// disconnects. It will retry up to `max_retries` times before giving up.
///
/// # Arguments
///
/// * `state` - Shared daemon state
/// * `wallpaper_rx` - Channel for receiving wallpaper commands
/// * `run_fn` - Function to run the Wayland blocking loop
///
/// # Returns
///
/// Ok if normal exit, Err if max retries exceeded or non-retryable error
pub(super) fn run_with_reconnect<F>(
    state: Arc<Mutex<DaemonState>>,
    mut wallpaper_rx: mpsc::UnboundedReceiver<WallpaperCommand>,
    mut run_fn: F,
) -> Result<()>
where
    F: FnMut(Arc<Mutex<DaemonState>>, &mut mpsc::UnboundedReceiver<WallpaperCommand>) -> Result<()>,
{
    // Get reconnection settings from config
    let (enable_reconnection, max_retries, initial_backoff, max_backoff) = {
        if let Ok(state_guard) = state.try_lock() {
            if let Some(ref config) = state_guard.config {
                (
                    config.advanced.enable_reconnection,
                    config.advanced.max_reconnection_retries,
                    config.advanced.initial_reconnection_backoff_ms,
                    config.advanced.max_reconnection_backoff_ms,
                )
            } else {
                (true, 10, 1000, 10000)
            }
        } else {
            (true, 10, 1000, 10000)
        }
    };

    if !enable_reconnection {
        log::info!("Reconnection disabled, running single connection");
        return run_fn(state, &mut wallpaper_rx);
    }

    let mut retry_count = 0u32;
    let mut backoff_ms = initial_backoff;

    loop {
        // Check if we should exit
        if let Ok(guard) = state.try_lock()
            && guard.should_exit
        {
            log::info!("Exit signal received, stopping reconnection attempts");
            return Ok(());
        }

        match run_fn(state.clone(), &mut wallpaper_rx) {
            Ok(_) => {
                // Normal exit (e.g., exit command received)
                log::info!("Wayland manager exited normally");
                return Ok(());
            }
            Err(e) => {
                let error_msg = format!("{}", e);

                // Check if it's a broken pipe (compositor disconnected)
                if is_broken_pipe_error(&error_msg) {
                    retry_count += 1;

                    log::warn!(
                        "Wayland compositor disconnected (broken pipe) - attempt {}/{}. Previous connection resources should be dropped now.",
                        retry_count,
                        max_retries
                    );

                    if retry_count > max_retries {
                        log::error!(
                            "Failed to reconnect after {} attempts. Giving up.",
                            max_retries
                        );
                        return Err(anyhow::anyhow!("Max reconnection attempts reached"));
                    }

                    log::warn!(
                        "Reconnecting in {}ms (attempt {}/{})...",
                        backoff_ms,
                        retry_count,
                        max_retries
                    );

                    // Wait before retrying - this also gives GStreamer time to clean up resources
                    std::thread::sleep(std::time::Duration::from_millis(backoff_ms));

                    // Exponential backoff
                    backoff_ms = std::cmp::min(backoff_ms * 2, max_backoff);

                    // Try again
                    continue;
                } else {
                    // Other error - don't retry
                    log::error!("Wayland error (not retrying): {}", e);
                    return Err(e);
                }
            }
        }
    }
}

/// Check if an error message indicates a broken pipe.
///
/// Broken pipe errors occur when the Wayland compositor disconnects.
pub(super) fn is_broken_pipe_error(error_msg: &str) -> bool {
    error_msg.contains("Broken pipe") || error_msg.contains("broken pipe")
}
