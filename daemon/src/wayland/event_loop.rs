//! Event loop and periodic task management.
//!
//! This module contains helper functions for the main event loop:
//! - Frame timing calculation
//! - Playlist rotation checking
//! - Schedule checking
//! - Resource monitoring
//! - Initial configuration application

use super::types::WallpaperDaemon;
use anyhow::Result;
use wayland_client::QueueHandle;

/// Calculate the optimal sleep duration based on next expected frame.
///
/// Checks all active animations (transitions, videos, shaders) and returns
/// the minimum delay needed to maintain smooth playback.
///
/// # Returns
///
/// Duration to sleep before next frame check (clamped between 1-100ms)
pub(super) fn get_next_frame_delay(app_data: &WallpaperDaemon) -> std::time::Duration {
    use std::time::Duration;

    // Start with a high value, we'll find the minimum needed
    let mut min_delay = Duration::from_millis(100);

    // Check for active transitions (need 60fps updates)
    for output_data in &app_data.outputs {
        if output_data.transition.is_some() {
            // Transition active, update at 60fps
            let transition_rate = Duration::from_millis(16);
            if transition_rate < min_delay {
                min_delay = transition_rate;
            }
        }
    }

    // Videos (including converted GIFs) produce frames asynchronously, poll at their actual frame rate
    // Check shared video managers (new architecture with GPU scaling)
    #[cfg(feature = "video")]
    for (_video_path, video_manager_arc) in &app_data.video_managers {
        // Try to get frame duration without blocking
        if let Ok(video_manager) = video_manager_arc.try_lock() {
            let video_poll_rate = video_manager.frame_duration();
            if video_poll_rate < min_delay {
                min_delay = video_poll_rate;
            }
        }
    }

    // Clamp to reasonable bounds
    // Min: 1ms (don't busy wait)
    // Max: 100ms (if nothing is animating, check infrequently)
    min_delay.clamp(Duration::from_millis(1), Duration::from_millis(100))
}

/// Check if playlist should rotate and apply next wallpaper.
///
/// Reads playlist state from shared state, checks rotation timing,
/// and applies the next wallpaper with configured transition.
pub(super) fn check_playlist_rotation(
    app_data: &mut WallpaperDaemon,
    qh: &QueueHandle<WallpaperDaemon>,
) -> Result<()> {
    // Check if we have a playlist and if it's time to rotate
    let should_rotate = {
        if let Ok(state) = app_data.state.try_lock() {
            if let Some(ref playlist) = state.playlist {
                playlist.should_rotate()
            } else {
                false
            }
        } else {
            false
        }
    };

    if !should_rotate {
        return Ok(());
    }

    // Get next wallpaper and config
    let next_path = {
        if let Ok(mut state) = app_data.state.try_lock() {
            if let Some(ref mut playlist) = state.playlist {
                if let Some(next) = playlist.next() {
                    let path = next.to_path_buf();

                    // Get transition from config or use default
                    let (trans, dur) = if let Some(ref config) = state.config {
                        if let Some(ref playlist_cfg) = config.playlist {
                            (
                                playlist_cfg.transition.clone(),
                                playlist_cfg.transition_duration,
                            )
                        } else {
                            (
                                config.general.default_transition.clone(),
                                config.general.default_duration,
                            )
                        }
                    } else {
                        ("fade".to_string(), 500)
                    };

                    Some((path, trans, dur))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    };

    if let Some((path, transition, duration)) = next_path {
        log::info!("Playlist rotation: {:?}", path.display());

        // Parse transition type
        let transition_type = super::utils::parse_transition(&transition, duration as i32);

        // Set the wallpaper
        let cmd = crate::WallpaperCommand::SetImage {
            path: path.to_string_lossy().to_string(),
            output: None, // Apply to all outputs
            scale: common::ScaleMode::Fill,
            transition: Some(transition_type),
        };

        super::commands::handle_wallpaper_command(app_data, cmd, qh)?;
    }

    Ok(())
}

/// Check schedule and apply scheduled wallpaper if time has come.
///
/// Reads scheduler state, checks if any schedule should be activated,
/// and applies the scheduled wallpaper with configured transition.
pub(super) fn check_schedule(
    app_data: &mut WallpaperDaemon,
    qh: &QueueHandle<WallpaperDaemon>,
) -> Result<()> {
    // Check if scheduler says we should switch wallpaper
    let scheduled_wallpaper = {
        if let Ok(mut state) = app_data.state.try_lock() {
            if let Some(ref mut scheduler) = state.scheduler {
                if scheduler.should_check() {
                    scheduler.check()
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    };

    if let Some(scheduled) = scheduled_wallpaper {
        log::info!(
            "Schedule activated: {} - {:?}",
            scheduled.schedule_name,
            scheduled.path.display()
        );

        let duration = scheduled.duration as u32;

        // Parse transition type
        let transition_type =
            super::utils::parse_transition(&scheduled.transition, duration as i32);

        // Set the wallpaper
        let cmd = crate::WallpaperCommand::SetImage {
            path: scheduled.path.to_string_lossy().to_string(),
            output: None, // Apply to all outputs
            scale: common::ScaleMode::Fill,
            transition: Some(transition_type),
        };

        super::commands::handle_wallpaper_command(app_data, cmd, qh)?;
    }

    Ok(())
}

/// Check and update resource monitor.
///
/// Periodically updates resource usage statistics (CPU, memory, battery)
/// and adjusts performance mode if needed. Updates shared state with latest stats.
pub(super) fn check_resources(app_data: &mut WallpaperDaemon) -> Result<()> {
    // Only check periodically (every 5 seconds)
    if !app_data.resource_monitor.should_check() {
        return Ok(());
    }

    // Update stats and possibly adjust performance mode
    let stats = app_data.resource_monitor.update()?;
    let mode = app_data.resource_monitor.mode();

    // Update shared state with latest stats
    if let Ok(mut state) = app_data.state.try_lock() {
        state.resource_stats = Some(stats);
        state.performance_mode = format!("{:?}", mode);
    }

    // Log buffer pool statistics to monitor for memory leaks
    for (idx, output) in app_data.outputs.iter().enumerate() {
        if !output.buffer_pool.is_empty() {
            let busy_count = output
                .buffer_pool
                .iter()
                .filter(|b| !b.is_released())
                .count();
            let released_count = output.buffer_pool.len() - busy_count;
            log::debug!(
                "Output {}: buffer pool size = {} (busy: {}, released: {})",
                idx,
                output.buffer_pool.len(),
                busy_count,
                released_count
            );
        }
    }

    // Note: Performance mode changes are logged in resource_monitor.update()
    // Future: Apply throttling based on performance mode
    //  - Reduce video frame rates
    //  - Skip GIF frames
    //  - Pause animations when on battery

    Ok(())
}

/// Apply initial wallpapers from configuration on startup.
///
/// Reads configuration from shared state and applies:
/// - Per-output wallpapers (if configured)
/// - First playlist item (if playlist is configured and no per-output wallpapers)
pub(super) fn apply_initial_config(
    app_data: &mut WallpaperDaemon,
    qh: &QueueHandle<WallpaperDaemon>,
) -> Result<()> {
    let state_lock = app_data.state.try_lock();
    if state_lock.is_err() {
        log::warn!("Could not acquire state lock for initial config");
        return Ok(());
    }

    let state = state_lock.unwrap();
    if state.config.is_none() {
        return Ok(());
    }

    let config = state.config.as_ref().unwrap();

    // Collect all wallpaper commands to apply
    let mut commands = Vec::new();

    // Check if we have per-output wallpapers configured
    for output_cfg in &config.output {
        if let Some(ref wallpaper_path) = output_cfg.wallpaper {
            log::info!(
                "Preparing initial wallpaper for {}: {}",
                output_cfg.name,
                wallpaper_path
            );

            let transition_type = common::TransitionType::Fade {
                duration_ms: output_cfg.duration as u32,
            };

            let scale_mode = super::utils::parse_scale_mode(&output_cfg.scale);

            let cmd = crate::WallpaperCommand::SetImage {
                path: wallpaper_path.clone(),
                output: Some(output_cfg.name.clone()),
                scale: scale_mode,
                transition: Some(transition_type),
            };

            commands.push(cmd);
        }
    }

    // If no per-output wallpapers were configured, try to start playlist
    if commands.is_empty()
        && let Some(ref playlist) = state.playlist
        && let Some(first) = playlist.current()
    {
        log::info!("Starting playlist with: {}", first.display());
        let first_path = first.to_path_buf();

        let cmd = crate::WallpaperCommand::SetImage {
            path: first_path.to_string_lossy().to_string(),
            output: None,
            scale: common::ScaleMode::Fill,
            transition: Some(common::TransitionType::Fade { duration_ms: 500 }),
        };

        commands.push(cmd);
    }

    // Drop the lock before applying commands
    drop(state);

    // Apply all collected commands
    for cmd in commands {
        super::commands::handle_wallpaper_command(app_data, cmd, qh)?;
    }

    Ok(())
}
