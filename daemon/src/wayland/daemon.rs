//! Wayland wallpaper daemon.
//!
//! This module orchestrates the Wayland integration:
//! - Connects to compositor
//! - Manages outputs (monitors)
//! - Handles wallpaper commands via IPC
//! - Updates video/shader frames
//! - Manages transitions
//! - Automatic reconnection on compositor disconnect

// Re-export main types for other wayland modules
pub(super) use super::types::WallpaperDaemon;

use anyhow::Result;
use smithay_client_toolkit::{
    compositor::CompositorState, output::OutputState, registry::RegistryState,
    shell::wlr_layer::LayerShell, shm::Shm,
};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use wayland_client::{Connection, globals::registry_queue_init};

use crate::log_and_continue;
use crate::wallpaper_manager::WallpaperManager;
use crate::{DaemonState, WallpaperCommand};

/// Main entry point for the Wayland manager.
///
/// Runs in a blocking task and handles reconnection automatically.
pub async fn run(
    state: Arc<Mutex<DaemonState>>,
    wallpaper_rx: mpsc::UnboundedReceiver<WallpaperCommand>,
) -> Result<()> {
    log::info!("Connecting to Wayland compositor...");

    // Run Wayland in a blocking task since it's synchronous
    tokio::task::spawn_blocking(move || {
        super::reconnection::run_with_reconnect(state, wallpaper_rx, run_wayland_blocking)
    })
    .await?
}

/// Run a single Wayland connection (blocking).
///
/// This function:
/// 1. Initializes GPU renderer (if available)
/// 2. Connects to Wayland compositor
/// 3. Creates layer surfaces for each output
/// 4. Applies initial configuration
/// 5. Runs event loop until exit or compositor disconnect
///
/// # Arguments
///
/// * `state` - Shared daemon state
/// * `wallpaper_rx` - Channel for receiving wallpaper commands
///
/// # Returns
///
/// Ok on normal exit, Err on fatal error or broken pipe (reconnection will retry)
fn run_wayland_blocking(
    state: Arc<Mutex<DaemonState>>,
    wallpaper_rx: &mut mpsc::UnboundedReceiver<WallpaperCommand>,
) -> Result<()> {
    log::info!("run_wayland_blocking - Starting new Wayland connection");

    // Initialize resource monitor with config from state
    let resource_config = match state.try_lock() {
        Ok(state_lock) => {
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
        }
        Err(_) => {
            log::warn!("State lock unavailable during reconnection; using defaults");
            crate::resource_monitor::ResourceConfig::default()
        }
    };

    let resource_monitor = crate::resource_monitor::ResourceMonitor::new(resource_config.clone());
    log::info!(
        "Resource monitor initialized (mode: {:?})",
        resource_monitor.mode()
    );
    log::debug!(
        "Resource monitor configured for reconnections with max_memory_mb={}",
        resource_config.max_memory_mb
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
        #[cfg(feature = "video")]
        video_managers: std::collections::HashMap::new(),
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
    super::event_loop::apply_initial_config(&mut app_data, &qh)?;

    // Event loop with command processing
    loop {
        // Process pending Wayland events (non-blocking dispatch)
        if let Err(e) = event_queue.dispatch_pending(&mut app_data) {
            // Check if it's a broken pipe (compositor disconnected)
            let error_msg = format!("{}", e);
            if super::reconnection::is_broken_pipe_error(&error_msg) {
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

        // Update video frames (GIFs are converted to video)
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
            super::event_loop::check_playlist_rotation(&mut app_data, &qh),
            "check playlist rotation"
        );

        // Check schedule
        log_and_continue!(
            super::event_loop::check_schedule(&mut app_data, &qh),
            "check schedule"
        );

        // Update resource monitor
        log_and_continue!(
            super::event_loop::check_resources(&mut app_data),
            "update resource monitor"
        );

        // Flush the connection
        if let Err(e) = event_queue.flush() {
            // Check if it's a broken pipe (compositor disconnected)
            let error_msg = format!("{}", e);
            if super::reconnection::is_broken_pipe_error(&error_msg) {
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

        // Clean up released buffers from pools to prevent memory leaks
        for output in &mut app_data.outputs {
            output.cleanup_buffer_pool();
        }

        // Adaptive sleep: sleep based on next expected frame time
        // Check GIF and video managers for their next frame times
        let next_frame_delay = super::event_loop::get_next_frame_delay(&app_data);
        std::thread::sleep(next_frame_delay);
    }

    log::info!(
        "run_wayland_blocking - Exiting, app_data will be dropped now (outputs: {})",
        app_data.outputs.len()
    );

    // Explicitly drop app_data to ensure cleanup happens before we return
    drop(app_data);

    log::info!("run_wayland_blocking - app_data dropped, resources cleaned up");
    Ok(())
}
