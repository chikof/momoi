use anyhow::Result;
use smithay_client_toolkit::{
    compositor::CompositorState, output::OutputState, registry::RegistryState,
    shell::wlr_layer::LayerShell, shm::Shm,
};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use wayland_client::{Connection, QueueHandle, globals::registry_queue_init, protocol::wl_output};

use crate::log_and_continue;
use crate::wallpaper_manager::WallpaperManager;
use crate::{DaemonState, WallpaperCommand};

/// Main entry point for the Wayland manager
pub async fn run(
    state: Arc<Mutex<DaemonState>>,
    wallpaper_rx: mpsc::UnboundedReceiver<WallpaperCommand>,
) -> Result<()> {
    log::info!("Connecting to Wayland compositor...");

    // Run Wayland in a blocking task since it's synchronous
    tokio::task::spawn_blocking(move || run_wayland_with_reconnect(state, wallpaper_rx)).await?
}

fn run_wayland_with_reconnect(
    state: Arc<Mutex<DaemonState>>,
    mut wallpaper_rx: mpsc::UnboundedReceiver<WallpaperCommand>,
) -> Result<()> {
    let mut retry_count = 0u32;
    let max_retries = 10;
    let mut backoff_ms = 1000u64; // Start with 1 second

    loop {
        // Check if we should exit
        if let Ok(guard) = state.try_lock()
            && guard.should_exit
        {
            log::info!("Exit signal received, stopping reconnection attempts");
            return Ok(());
        }

        match run_wayland_blocking(state.clone(), &mut wallpaper_rx) {
            Ok(_) => {
                // Normal exit (e.g., exit command received)
                log::info!("Wayland manager exited normally");
                return Ok(());
            }
            Err(e) => {
                let error_msg = format!("{}", e);

                // Check if it's a broken pipe (compositor disconnected)
                if error_msg.contains("Broken pipe") || error_msg.contains("broken pipe") {
                    retry_count += 1;

                    if retry_count > max_retries {
                        log::error!(
                            "Failed to reconnect after {} attempts. Giving up.",
                            max_retries
                        );
                        return Err(anyhow::anyhow!("Max reconnection attempts reached"));
                    }

                    log::warn!(
                        "Wayland compositor disconnected (attempt {}/{}). Reconnecting in {}ms...",
                        retry_count,
                        max_retries,
                        backoff_ms
                    );

                    // Wait before retrying
                    std::thread::sleep(std::time::Duration::from_millis(backoff_ms));

                    // Exponential backoff, max 10 seconds
                    backoff_ms = std::cmp::min(backoff_ms * 2, 10000);

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

fn run_wayland_blocking(
    state: Arc<Mutex<DaemonState>>,
    wallpaper_rx: &mut mpsc::UnboundedReceiver<WallpaperCommand>,
) -> Result<()> {
    // Initialize resource monitor with config from state
    let resource_config = {
        let state_lock = state.try_lock().expect("Failed to lock state");
        if let Some(ref config) = state_lock.config {
            crate::resource_monitor::ResourceConfig {
                auto_battery_mode: config.advanced.auto_battery_mode,
                enforce_memory_limits: config.advanced.enforce_memory_limits,
                max_memory_mb: config.advanced.max_memory_mb,
                cpu_threshold: config.advanced.cpu_threshold,
            }
        } else {
            crate::resource_monitor::ResourceConfig::default()
        }
    };

    let resource_monitor = crate::resource_monitor::ResourceMonitor::new(resource_config);
    log::info!(
        "Resource monitor initialized (mode: {:?})",
        resource_monitor.mode()
    );

    // Initialize GPU renderer if available
    #[cfg(feature = "gpu")]
    let gpu_renderer = {
        if crate::gpu::is_available() {
            log::info!("GPU acceleration available, initializing...");
            match pollster::block_on(crate::gpu::GpuRenderer::new()) {
                Ok(renderer) => {
                    renderer.context().capabilities().log_info();
                    log::info!("GPU renderer initialized successfully");
                    Some(std::sync::Arc::new(renderer))
                }
                Err(e) => {
                    log::warn!("Failed to initialize GPU renderer: {}", e);
                    log::warn!("Falling back to CPU rendering");
                    None
                }
            }
        } else {
            log::info!("GPU acceleration not available, using CPU rendering");
            None
        }
    };

    let conn = Connection::connect_to_env()?;
    let (globals, mut event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();

    let mut app_data = WallpaperDaemon {
        registry_state: RegistryState::new(&globals),
        compositor_state: CompositorState::bind(&globals, &qh)?,
        layer_shell: LayerShell::bind(&globals, &qh)?,
        output_state: OutputState::new(&globals, &qh),
        shm: Shm::bind(&globals, &qh)?,
        outputs: Vec::new(),
        wallpaper_manager: WallpaperManager::new(),
        state,
        exit: false,
        resource_monitor,
        #[cfg(feature = "gpu")]
        gpu_renderer,
    };

    log::info!("Connected to Wayland compositor");

    // Initial roundtrip to get outputs and create layer surfaces
    // Note: new_output callback will be triggered and create surfaces automatically
    event_queue.roundtrip(&mut app_data)?;

    log::info!(
        "Found {} output(s), created {} layer surface(s)",
        app_data.output_state.outputs().count(),
        app_data.outputs.len()
    );

    // Do another roundtrip to get configure events
    event_queue.roundtrip(&mut app_data)?;

    log::info!(
        "After configure roundtrip: {} configured output(s)",
        app_data.outputs.iter().filter(|o| o.configured).count()
    );

    // Populate shared state with output information
    super::outputs::sync_outputs_to_shared_state(&mut app_data);

    // Restore wallpapers after reconnection (if any were set before)
    super::outputs::restore_wallpapers_from_state(&mut app_data, &qh)?;

    // Apply initial wallpapers from configuration
    log::info!("Applying initial configuration...");
    apply_initial_config(&mut app_data, &qh)?;

    // Event loop with command processing
    loop {
        // Process pending Wayland events (non-blocking dispatch)
        if let Err(e) = event_queue.dispatch_pending(&mut app_data) {
            // Check if it's a broken pipe (compositor disconnected)
            let error_msg = format!("{}", e);
            if error_msg.contains("Broken pipe") || error_msg.contains("broken pipe") {
                log::warn!("Wayland compositor disconnected (broken pipe).");
                return Err(anyhow::anyhow!("Broken pipe"));
            }
            // For other errors, return error
            log::error!("Failed to dispatch Wayland events: {}", e);
            return Err(e.into());
        }

        // Check for wallpaper commands
        if let Ok(cmd) = wallpaper_rx.try_recv() {
            log_and_continue!(
                super::commands::handle_wallpaper_command(&mut app_data, cmd, &qh),
                "handle wallpaper command"
            );
        }

        // Update animated GIF frames
        log_and_continue!(
            super::frame_updates::update_gif_frames(&mut app_data, &qh),
            "update GIF frames"
        );

        // Update video frames
        log_and_continue!(
            super::frame_updates::update_video_frames(&mut app_data, &qh),
            "update video frames"
        );

        // Update shader frames
        log_and_continue!(
            super::frame_updates::update_shader_frames(&mut app_data, &qh),
            "update shader frames"
        );

        // Update transitions
        log_and_continue!(
            super::transitions::update_transitions(&mut app_data, &qh),
            "update transitions"
        );

        // Check playlist rotation
        log_and_continue!(
            check_playlist_rotation(&mut app_data, &qh),
            "check playlist rotation"
        );

        // Check schedule
        log_and_continue!(check_schedule(&mut app_data, &qh), "check schedule");

        // Update resource monitor
        log_and_continue!(check_resources(&mut app_data), "update resource monitor");

        // Flush the connection
        if let Err(e) = event_queue.flush() {
            // Check if it's a broken pipe (compositor disconnected)
            let error_msg = format!("{}", e);
            if error_msg.contains("Broken pipe") || error_msg.contains("broken pipe") {
                log::warn!("Wayland compositor disconnected (broken pipe).");
                return Err(anyhow::anyhow!("Broken pipe"));
            }
            log::error!("Failed to flush Wayland events: {}", e);
        }

        // Check if we should exit (use try_lock to avoid blocking)
        if let Ok(guard) = app_data.state.try_lock()
            && guard.should_exit
        {
            app_data.exit = true;
        }

        if app_data.exit {
            log::info!("Exiting Wayland event loop");
            break;
        }

        // Adaptive sleep: sleep based on next expected frame time
        // Check GIF and video managers for their next frame times
        let next_frame_delay = get_next_frame_delay(&app_data);
        std::thread::sleep(next_frame_delay);
    }

    Ok(())
}

pub struct WallpaperDaemon {
    pub(super) registry_state: RegistryState,
    pub(super) compositor_state: CompositorState,
    pub(super) layer_shell: LayerShell,
    pub(super) output_state: OutputState,
    pub(super) shm: Shm,
    pub(super) outputs: Vec<OutputData>,
    pub(super) wallpaper_manager: WallpaperManager,
    pub(super) state: Arc<Mutex<DaemonState>>,
    pub(super) exit: bool,
    pub(super) resource_monitor: crate::resource_monitor::ResourceMonitor,
    /// Shared GPU renderer (if available and enabled)
    #[cfg(feature = "gpu")]
    pub(super) gpu_renderer: Option<std::sync::Arc<crate::gpu::GpuRenderer>>,
}

pub struct OutputData {
    pub(super) output: wl_output::WlOutput,
    pub(super) layer_surface: Option<smithay_client_toolkit::shell::wlr_layer::LayerSurface>,
    pub(super) buffer: Option<crate::buffer::ShmBuffer>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) scale: f64,
    pub(super) configured: bool,
    pub(super) gif_manager: Option<crate::gif_manager::GifManager>,
    pub(super) video_manager: Option<crate::video_manager::VideoManager>,
    pub(super) shader_manager: Option<crate::shader_manager::ShaderManager>,
    pub(super) overlay_manager: Option<crate::overlay_shader::OverlayManager>,
    /// Active transition (if any)
    pub(super) transition: Option<crate::transition::Transition>,
    /// Pending new wallpaper content (used during transitions)
    pub(super) pending_wallpaper_data: Option<Vec<u8>>,
    /// GPU renderer for accelerated rendering (optional)
    #[cfg(feature = "gpu")]
    pub(super) gpu_renderer: Option<std::sync::Arc<crate::gpu::GpuRenderer>>,
}

/// Frame data ready for rendering (computed in parallel)
pub struct FrameUpdate {
    pub(super) output_index: usize,
    pub(super) argb_data: Vec<u8>,
    pub(super) width: u32,
    pub(super) height: u32,
}

/// Calculate the optimal sleep duration based on next expected frame
fn get_next_frame_delay(app_data: &WallpaperDaemon) -> std::time::Duration {
    use std::time::Duration;

    let mut min_delay = Duration::from_millis(16); // Cap at ~60fps (16ms)

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

    // Check GIF managers for next frame time
    for output_data in &app_data.outputs {
        if let Some(gif_manager) = &output_data.gif_manager {
            let delay = gif_manager.time_until_next_frame();
            if delay < min_delay {
                min_delay = delay;
            }
        }
    }

    // Videos produce frames asynchronously, so we check more frequently
    // but not too frequently to waste CPU
    for output_data in &app_data.outputs {
        if let Some(video_manager) = &output_data.video_manager {
            // Poll at the frame rate (not half) - GStreamer buffers frames for us
            // This reduces CPU usage significantly while still being responsive
            let video_poll_rate = video_manager.frame_duration();
            if video_poll_rate < min_delay {
                min_delay = video_poll_rate;
            }
        }
    }

    // Clamp to reasonable bounds
    // Min: 1ms (don't busy wait)
    // Max: 16ms (don't sleep too long for responsiveness)
    min_delay.clamp(Duration::from_millis(1), Duration::from_millis(16))
}

fn check_playlist_rotation(
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

fn check_schedule(app_data: &mut WallpaperDaemon, qh: &QueueHandle<WallpaperDaemon>) -> Result<()> {
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

/// Check and update resource monitor
fn check_resources(app_data: &mut WallpaperDaemon) -> Result<()> {
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

    // Note: Performance mode changes are logged in resource_monitor.update()
    // Future: Apply throttling based on performance mode
    //  - Reduce video frame rates
    //  - Skip GIF frames
    //  - Pause animations when on battery

    Ok(())
}

/// Apply initial wallpapers from configuration on startup
fn apply_initial_config(
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
