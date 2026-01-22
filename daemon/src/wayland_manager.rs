use anyhow::Result;
use rayon::prelude::*;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{Shm, ShmHandler},
};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_buffer, wl_shm_pool},
    protocol::{wl_output, wl_surface},
    Connection, Dispatch, Proxy, QueueHandle,
};

use crate::wallpaper_manager::WallpaperManager;
use crate::{DaemonState, WallpaperCommand};

pub async fn run(
    state: Arc<Mutex<DaemonState>>,
    wallpaper_rx: mpsc::UnboundedReceiver<WallpaperCommand>,
) -> Result<()> {
    log::info!("Connecting to Wayland compositor...");

    // Run Wayland in a blocking task since it's synchronous
    let result =
        tokio::task::spawn_blocking(move || run_wayland_with_reconnect(state, wallpaper_rx))
            .await?;

    result
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
        if let Ok(guard) = state.try_lock() {
            if guard.should_exit {
                log::info!("Exit signal received, stopping reconnection attempts");
                return Ok(());
            }
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
    app_data.sync_outputs_to_shared_state();

    // Restore wallpapers after reconnection (if any were set before)
    app_data.restore_wallpapers_from_state(&qh)?;

    // Apply initial wallpapers from configuration
    log::info!("Applying initial configuration...");
    app_data.apply_initial_config(&qh)?;

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
            if let Err(e) = app_data.handle_wallpaper_command(cmd, &qh) {
                log::error!("Failed to handle wallpaper command: {}", e);
            }
        }

        // Update animated GIF frames
        if let Err(e) = app_data.update_gif_frames(&qh) {
            log::error!("Failed to update GIF frames: {}", e);
        }

        // Update video frames
        if let Err(e) = app_data.update_video_frames(&qh) {
            log::error!("Failed to update video frames: {}", e);
        }

        // Update shader frames
        if let Err(e) = app_data.update_shader_frames(&qh) {
            log::error!("Failed to update shader frames: {}", e);
        }

        // Update transitions
        if let Err(e) = app_data.update_transitions(&qh) {
            log::error!("Failed to update transitions: {}", e);
        }

        // Check playlist rotation
        if let Err(e) = app_data.check_playlist_rotation(&qh) {
            log::error!("Failed to check playlist rotation: {}", e);
        }

        // Check schedule
        if let Err(e) = app_data.check_schedule(&qh) {
            log::error!("Failed to check schedule: {}", e);
        }

        // Update resource monitor
        if let Err(e) = app_data.check_resources() {
            log::error!("Failed to update resource monitor: {}", e);
        }

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
        if let Ok(guard) = app_data.state.try_lock() {
            if guard.should_exit {
                app_data.exit = true;
            }
        }

        if app_data.exit {
            log::info!("Exiting Wayland event loop");
            break;
        }

        // Adaptive sleep: sleep based on next expected frame time
        // Check GIF and video managers for their next frame times
        let next_frame_delay = app_data.get_next_frame_delay();
        std::thread::sleep(next_frame_delay);
    }

    Ok(())
}

pub struct WallpaperDaemon {
    registry_state: RegistryState,
    compositor_state: CompositorState,
    layer_shell: LayerShell,
    output_state: OutputState,
    shm: Shm,
    outputs: Vec<OutputData>,
    wallpaper_manager: WallpaperManager,
    state: Arc<Mutex<DaemonState>>,
    exit: bool,
    resource_monitor: crate::resource_monitor::ResourceMonitor,
    /// Shared GPU renderer (if available and enabled)
    #[cfg(feature = "gpu")]
    gpu_renderer: Option<std::sync::Arc<crate::gpu::GpuRenderer>>,
}

struct OutputData {
    output: wl_output::WlOutput,
    layer_surface: Option<LayerSurface>,
    buffer: Option<crate::buffer::ShmBuffer>,
    width: u32,
    height: u32,
    scale: f64,
    configured: bool,
    gif_manager: Option<crate::gif_manager::GifManager>,
    video_manager: Option<crate::video_manager::VideoManager>,
    shader_manager: Option<crate::shader_manager::ShaderManager>,
    overlay_manager: Option<crate::overlay_shader::OverlayManager>,
    /// Active transition (if any)
    transition: Option<crate::transition::Transition>,
    /// Pending new wallpaper content (used during transitions)
    pending_wallpaper_data: Option<Vec<u8>>,
    /// GPU renderer for accelerated rendering (optional)
    #[cfg(feature = "gpu")]
    gpu_renderer: Option<std::sync::Arc<crate::gpu::GpuRenderer>>,
}

/// Frame data ready for rendering (computed in parallel)
struct FrameUpdate {
    output_index: usize,
    argb_data: Vec<u8>,
    width: u32,
    height: u32,
}

impl WallpaperDaemon {
    fn sync_outputs_to_shared_state(&mut self) {
        if let Ok(mut state) = self.state.try_lock() {
            state.outputs.clear();

            for output_data in &self.outputs {
                if let Some(info) = self.output_state.info(&output_data.output) {
                    let output_info = common::OutputInfo {
                        name: info.name.clone().unwrap_or_else(|| "Unknown".to_string()),
                        width: output_data.width,
                        height: output_data.height,
                        scale: info.scale_factor as f64,
                        refresh_rate: None,
                    };
                    log::info!(
                        "Added output to shared state: {} ({}x{})",
                        output_info.name,
                        output_info.width,
                        output_info.height
                    );
                    state.outputs.push(output_info);
                }
            }
        } else {
            log::warn!("Could not acquire state lock to sync outputs");
        }
    }

    /// Restore wallpapers from shared state after reconnection
    fn restore_wallpapers_from_state(&mut self, qh: &QueueHandle<Self>) -> Result<()> {
        let wallpapers = if let Ok(state) = self.state.try_lock() {
            state.wallpapers.clone()
        } else {
            log::warn!("Could not acquire state lock to restore wallpapers");
            return Ok(());
        };

        if wallpapers.is_empty() {
            log::debug!("No wallpapers to restore");
            return Ok(());
        }

        log::info!(
            "Restoring {} wallpaper(s) after reconnection",
            wallpapers.len()
        );

        for (output_name, wallpaper_type) in wallpapers {
            let cmd = match wallpaper_type {
                common::WallpaperType::Image(path) => {
                    log::info!("Restoring image wallpaper on {}: {}", output_name, path);
                    WallpaperCommand::SetImage {
                        path,
                        output: Some(output_name.clone()),
                        scale: common::ScaleMode::Fill, // Default scale mode
                        transition: None,               // No transition on restore
                    }
                }
                common::WallpaperType::Video(path) => {
                    log::info!("Restoring video wallpaper on {}: {}", output_name, path);
                    WallpaperCommand::SetImage {
                        path,
                        output: Some(output_name.clone()),
                        scale: common::ScaleMode::Fill,
                        transition: None,
                    }
                }
                common::WallpaperType::Color(color) => {
                    log::info!("Restoring color wallpaper on {}: {}", output_name, color);
                    WallpaperCommand::SetColor {
                        color,
                        output: Some(output_name.clone()),
                    }
                }
                common::WallpaperType::Shader(shader) => {
                    log::info!("Restoring shader wallpaper on {}: {}", output_name, shader);
                    WallpaperCommand::SetShader {
                        shader,
                        output: Some(output_name.clone()),
                        transition: None,
                        params: None, // Use default params when restoring
                    }
                }
                common::WallpaperType::None => {
                    log::debug!("Skipping 'None' wallpaper for {}", output_name);
                    continue;
                }
            };

            // Apply the wallpaper
            if let Err(e) = self.handle_wallpaper_command(cmd, qh) {
                log::error!("Failed to restore wallpaper for {}: {}", output_name, e);
            }
        }

        Ok(())
    }

    fn create_layer_surface(
        &mut self,
        output: wl_output::WlOutput,
        qh: &QueueHandle<Self>,
    ) -> Result<()> {
        let surface = self.compositor_state.create_surface(qh);

        let layer_surface = self.layer_shell.create_layer_surface(
            qh,
            surface,
            Layer::Background,
            Some("wallpaper"),
            Some(&output),
        );

        // Configure the layer surface
        layer_surface.set_anchor(Anchor::all());
        layer_surface.set_exclusive_zone(-1);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer_surface.commit();

        self.outputs.push(OutputData {
            output,
            layer_surface: Some(layer_surface),
            buffer: None,
            width: 0,
            height: 0,
            scale: 1.0,
            configured: false,
            gif_manager: None,
            video_manager: None,
            shader_manager: None,
            overlay_manager: None,
            transition: None,
            pending_wallpaper_data: None,
            #[cfg(feature = "gpu")]
            gpu_renderer: None,
        });

        log::info!("Created layer surface for output");
        Ok(())
    }

    fn handle_wallpaper_command(
        &mut self,
        cmd: WallpaperCommand,
        qh: &QueueHandle<Self>,
    ) -> Result<()> {
        match cmd {
            WallpaperCommand::SetImage {
                path,
                output,
                scale,
                transition,
            } => self.set_image_wallpaper(&path, output.as_deref(), scale, transition, qh),
            WallpaperCommand::SetColor { color, output } => {
                self.set_color_wallpaper(&color, output.as_deref(), qh)
            }
            WallpaperCommand::SetShader {
                shader,
                output,
                transition,
                params,
            } => self.set_shader_wallpaper(&shader, output.as_deref(), transition, params, qh),
            WallpaperCommand::SetOverlay {
                overlay,
                params,
                output,
            } => self.set_overlay_shader(&overlay, params, output.as_deref(), qh),
            WallpaperCommand::ClearOverlay { output } => {
                self.clear_overlay_shader(output.as_deref())
            }
        }
    }

    fn set_image_wallpaper(
        &mut self,
        path: &str,
        output_filter: Option<&str>,
        scale: common::ScaleMode,
        transition: Option<common::TransitionType>,
        qh: &QueueHandle<Self>,
    ) -> Result<()> {
        log::info!(
            "Setting image wallpaper: {} for output: {:?} with transition: {:?}",
            path,
            output_filter,
            transition
        );

        // Check if this is a video
        if WallpaperManager::is_video(path) {
            log::info!("Detected video file, loading with VideoManager");
            return self.set_video_wallpaper(path, output_filter, scale, transition, qh);
        }

        // Check if this is an animated GIF
        let is_animated = WallpaperManager::is_animated_gif(path)?;

        if is_animated {
            log::info!("Detected animated GIF, loading with GifManager");
            return self.set_animated_gif(path, output_filter, scale, transition, qh);
        }

        // Load and clone the image (so we don't hold a borrow to wallpaper_manager)
        let image = self.wallpaper_manager.load_image(path)?.clone();

        // Apply to matching outputs
        for output_data in &mut self.outputs {
            if !output_data.configured {
                continue;
            }

            // Check if this output matches the filter
            if let Some(filter) = output_filter {
                if let Some(info) = self.output_state.info(&output_data.output) {
                    if let Some(name) = &info.name {
                        if name != filter && filter != "all" {
                            continue;
                        }
                    }
                }
            }

            let width = output_data.width;
            let height = output_data.height;

            if width == 0 || height == 0 {
                continue;
            }

            // Scale image to fit output
            // Try GPU acceleration first, fall back to CPU if unavailable
            let argb_data = {
                #[cfg(feature = "gpu")]
                {
                    if let Some(ref gpu) = output_data.gpu_renderer {
                        let start = std::time::Instant::now();
                        log::debug!("Using GPU acceleration for image scaling");
                        // Convert DynamicImage to RGBA
                        let rgba_image = image.to_rgba8();
                        let (src_width, src_height) = rgba_image.dimensions();

                        match gpu.render_image(
                            rgba_image.as_raw(),
                            src_width,
                            src_height,
                            width,
                            height,
                        ) {
                            Ok(data) => {
                                let elapsed = start.elapsed();
                                log::info!(
                                    "GPU rendering: {}x{} -> {}x{} in {:.2}ms",
                                    src_width,
                                    src_height,
                                    width,
                                    height,
                                    elapsed.as_secs_f64() * 1000.0
                                );
                                data
                            }
                            Err(e) => {
                                log::warn!("GPU rendering failed: {}, falling back to CPU", e);
                                // Fallback to CPU
                                let start_cpu = std::time::Instant::now();
                                let scaled = self
                                    .wallpaper_manager
                                    .scale_image(&image, width, height, scale)?;
                                let result = self.wallpaper_manager.rgba_to_argb8888(&scaled);
                                let elapsed = start_cpu.elapsed();
                                log::info!(
                                    "CPU rendering (fallback): {}x{} in {:.2}ms",
                                    width,
                                    height,
                                    elapsed.as_secs_f64() * 1000.0
                                );
                                result
                            }
                        }
                    } else {
                        // No GPU, use CPU
                        let start = std::time::Instant::now();
                        let scaled = self
                            .wallpaper_manager
                            .scale_image(&image, width, height, scale)?;
                        let result = self.wallpaper_manager.rgba_to_argb8888(&scaled);
                        let elapsed = start.elapsed();
                        log::info!(
                            "CPU rendering: {}x{} in {:.2}ms",
                            width,
                            height,
                            elapsed.as_secs_f64() * 1000.0
                        );
                        result
                    }
                }

                #[cfg(not(feature = "gpu"))]
                {
                    // GPU feature disabled, use CPU
                    let scaled = self
                        .wallpaper_manager
                        .scale_image(&image, width, height, scale)?;
                    self.wallpaper_manager.rgba_to_argb8888(&scaled)
                }
            };

            // Clear any GIF/video managers (can't have multiple types)
            output_data.gif_manager = None;
            output_data.video_manager = None;

            // Handle transition if requested
            if let Some(ref trans_config) = transition {
                if trans_config.duration_ms() > 0 {
                    // Capture current frame as "old frame" for transition
                    if let Some(buffer) = &output_data.buffer {
                        if let Ok(old_frame_data) = buffer.read_data() {
                            // Start transition
                            let transition_type =
                                crate::transition::TransitionType::from(trans_config);
                            let duration =
                                std::time::Duration::from_millis(trans_config.duration_ms() as u64);

                            output_data.transition = Some(crate::transition::Transition::new(
                                transition_type,
                                duration,
                                old_frame_data,
                                width,
                                height,
                                #[cfg(feature = "gpu")]
                                output_data.gpu_renderer.clone(),
                            ));

                            // Store new wallpaper as pending
                            output_data.pending_wallpaper_data = Some(argb_data);

                            log::info!(
                                "Starting {:?} transition ({}ms) for output {}x{}",
                                transition_type,
                                trans_config.duration_ms(),
                                width,
                                height
                            );

                            // Don't update buffer yet - transition will handle it
                            continue;
                        }
                    }
                }
            }

            // No transition or transition setup failed - apply immediately
            // Apply overlay if present
            let mut final_data = argb_data;
            if let Err(e) =
                Self::apply_overlay_to_frame(output_data, &mut final_data, width, height)
            {
                log::warn!("Failed to apply overlay to image: {}", e);
            }

            // Create or update buffer
            let mut buffer = crate::buffer::ShmBuffer::new(&self.shm.wl_shm(), width, height, qh)?;
            buffer.write_image_data(&final_data)?;

            // Attach and commit
            if let Some(layer_surface) = &output_data.layer_surface {
                layer_surface
                    .wl_surface()
                    .attach(Some(buffer.buffer()), 0, 0);
                layer_surface
                    .wl_surface()
                    .damage_buffer(0, 0, width as i32, height as i32);
                layer_surface.wl_surface().commit();
            }

            output_data.buffer = Some(buffer);

            log::info!("Applied wallpaper to output {}x{}", width, height);
        }

        // Update shared state
        if let Ok(mut state) = self.state.try_lock() {
            let wallpaper_type = common::WallpaperType::Image(path.to_string());
            if let Some(filter) = output_filter {
                if filter == "all" {
                    let output_names: Vec<String> =
                        state.outputs.iter().map(|o| o.name.clone()).collect();
                    for name in output_names {
                        state.wallpapers.insert(name, wallpaper_type.clone());
                    }
                } else {
                    state.wallpapers.insert(filter.to_string(), wallpaper_type);
                }
            } else {
                // Apply to all outputs
                let output_names: Vec<String> =
                    state.outputs.iter().map(|o| o.name.clone()).collect();
                for name in output_names {
                    state.wallpapers.insert(name, wallpaper_type.clone());
                }
            }
        }

        Ok(())
    }

    fn set_animated_gif(
        &mut self,
        path: &str,
        output_filter: Option<&str>,
        scale: common::ScaleMode,
        _transition: Option<common::TransitionType>,
        qh: &QueueHandle<Self>,
    ) -> Result<()> {
        // TODO: Implement transitions for GIFs
        // For now, we just apply immediately
        // Apply to matching outputs
        for output_data in &mut self.outputs {
            if !output_data.configured {
                continue;
            }

            // Check if this output matches the filter
            if let Some(filter) = output_filter {
                if let Some(info) = self.output_state.info(&output_data.output) {
                    if let Some(name) = &info.name {
                        if name != filter && filter != "all" {
                            continue;
                        }
                    }
                }
            }

            let width = output_data.width;
            let height = output_data.height;

            if width == 0 || height == 0 {
                continue;
            }

            // Load GIF manager with pre-scaling
            let gif_manager = crate::gif_manager::GifManager::load(
                path,
                width,
                height,
                scale,
                &self.wallpaper_manager,
                #[cfg(feature = "gpu")]
                output_data.gpu_renderer.as_ref().map(|arc| arc.as_ref()),
            )?;
            log::info!(
                "Loaded animated GIF with {} frames for output {}x{}",
                gif_manager.frame_count(),
                width,
                height
            );

            // Get the first frame data (already scaled and converted)
            let argb_data = gif_manager.current_frame_data();

            // Apply overlay if present
            let mut final_data = argb_data.to_vec();
            if let Err(e) =
                Self::apply_overlay_to_frame(output_data, &mut final_data, width, height)
            {
                log::warn!("Failed to apply overlay to GIF first frame: {}", e);
            }

            // Create buffer and render
            let mut buffer = crate::buffer::ShmBuffer::new(&self.shm.wl_shm(), width, height, qh)?;
            buffer.write_image_data(&final_data)?;

            // Attach and commit
            if let Some(layer_surface) = &output_data.layer_surface {
                layer_surface
                    .wl_surface()
                    .attach(Some(buffer.buffer()), 0, 0);
                layer_surface
                    .wl_surface()
                    .damage_buffer(0, 0, width as i32, height as i32);
                layer_surface.wl_surface().commit();
            }

            output_data.buffer = Some(buffer);
            output_data.gif_manager = Some(gif_manager);

            log::info!("Applied animated GIF to output {}x{}", width, height);
        }

        // Update shared state
        if let Ok(mut state) = self.state.try_lock() {
            let wallpaper_type = common::WallpaperType::Image(path.to_string());
            if let Some(filter) = output_filter {
                if let Some(info) = self
                    .output_state
                    .info(&self.outputs.iter().find(|o| o.configured).unwrap().output)
                {
                    if let Some(name) = &info.name {
                        state
                            .wallpapers
                            .insert(name.clone(), wallpaper_type.clone());
                    }
                } else {
                    state.wallpapers.insert(filter.to_string(), wallpaper_type);
                }
            } else {
                // Apply to all outputs
                let output_names: Vec<String> =
                    state.outputs.iter().map(|o| o.name.clone()).collect();
                for name in output_names {
                    state.wallpapers.insert(name, wallpaper_type.clone());
                }
            }
        }

        Ok(())
    }

    fn set_color_wallpaper(
        &mut self,
        color: &str,
        output_filter: Option<&str>,
        qh: &QueueHandle<Self>,
    ) -> Result<()> {
        log::info!(
            "Setting color wallpaper: {} for output: {:?}",
            color,
            output_filter
        );

        // Parse color
        let (r, g, b, a) = crate::buffer::parse_hex_color(color)
            .ok_or_else(|| anyhow::anyhow!("Invalid color format: {}", color))?;

        // Apply to matching outputs
        for output_data in &mut self.outputs {
            if !output_data.configured {
                continue;
            }

            // Check if this output matches the filter
            if let Some(filter) = output_filter {
                if let Some(info) = self.output_state.info(&output_data.output) {
                    if let Some(name) = &info.name {
                        if name != filter && filter != "all" {
                            continue;
                        }
                    }
                }
            }

            let width = output_data.width;
            let height = output_data.height;

            if width == 0 || height == 0 {
                continue;
            }

            // Create buffer and fill with color
            let mut buffer = crate::buffer::ShmBuffer::new(&self.shm.wl_shm(), width, height, qh)?;
            buffer.fill_color(r, g, b, a);

            // Attach and commit
            if let Some(layer_surface) = &output_data.layer_surface {
                layer_surface
                    .wl_surface()
                    .attach(Some(buffer.buffer()), 0, 0);
                layer_surface
                    .wl_surface()
                    .damage_buffer(0, 0, width as i32, height as i32);
                layer_surface.wl_surface().commit();
            }

            output_data.buffer = Some(buffer);

            log::info!("Applied color to output {}x{}", width, height);
        }

        // Update shared state
        if let Ok(mut state) = self.state.try_lock() {
            let wallpaper_type = common::WallpaperType::Color(color.to_string());
            if let Some(filter) = output_filter {
                if filter == "all" {
                    let output_names: Vec<String> =
                        state.outputs.iter().map(|o| o.name.clone()).collect();
                    for name in output_names {
                        state.wallpapers.insert(name, wallpaper_type.clone());
                    }
                } else {
                    state.wallpapers.insert(filter.to_string(), wallpaper_type);
                }
            } else {
                // Apply to all outputs
                let output_names: Vec<String> =
                    state.outputs.iter().map(|o| o.name.clone()).collect();
                for name in output_names {
                    state.wallpapers.insert(name, wallpaper_type.clone());
                }
            }
        }

        Ok(())
    }

    fn set_shader_wallpaper(
        &mut self,
        shader_name: &str,
        output_filter: Option<&str>,
        _transition: Option<common::TransitionType>,
        mut params: Option<common::ShaderParams>,
        qh: &QueueHandle<Self>,
    ) -> Result<()> {
        log::info!(
            "Setting shader wallpaper: {} for output: {:?}",
            shader_name,
            output_filter
        );

        // Check if params contains a preset marker
        if let Some(ref p) = params {
            if let Some(ref color1) = p.color1 {
                if let Some(preset_name) = color1.strip_prefix("preset:") {
                    // Look up preset in config
                    if let Ok(state) = self.state.try_lock() {
                        if let Some(config) = &state.config {
                            if let Some(preset) =
                                config.shader_preset.iter().find(|p| p.name == preset_name)
                            {
                                log::info!("Using shader preset: {}", preset_name);
                                params = Some(preset.to_params());
                            } else {
                                log::warn!("Shader preset '{}' not found in config", preset_name);
                                params = None;
                            }
                        } else {
                            log::warn!("Cannot use preset '{}': no config loaded", preset_name);
                            params = None;
                        }
                    }
                }
            }
        }

        // Parse shader type
        let shader =
            crate::shader_manager::BuiltinShader::from_str(shader_name).ok_or_else(|| {
                anyhow::anyhow!(
                    "Unknown shader: {}. Available: plasma, waves, matrix, gradient, starfield",
                    shader_name
                )
            })?;

        // Apply to matching outputs
        for output_data in &mut self.outputs {
            if !output_data.configured {
                continue;
            }

            // Check if this output matches the filter
            if let Some(filter) = output_filter {
                if let Some(info) = self.output_state.info(&output_data.output) {
                    if let Some(name) = &info.name {
                        if name != filter && filter != "all" {
                            continue;
                        }
                    }
                }
            }

            let width = output_data.width;
            let height = output_data.height;

            if width == 0 || height == 0 {
                continue;
            }

            // Clear any existing video/gif managers
            output_data.gif_manager = None;
            output_data.video_manager = None;

            // Create shader manager for this output
            let shader_mgr = crate::shader_manager::ShaderManager::new(
                shader,
                width,
                height,
                params.clone(),
                #[cfg(feature = "gpu")]
                output_data.gpu_renderer.clone(),
            );
            output_data.shader_manager = Some(shader_mgr);

            log::info!(
                "Applied shader '{}' to output {}x{}",
                shader_name,
                width,
                height
            );
        }

        // Update shared state
        if let Ok(mut state) = self.state.try_lock() {
            let wallpaper_type = common::WallpaperType::Shader(shader_name.to_string());
            if let Some(filter) = output_filter {
                if filter == "all" {
                    let output_names: Vec<String> =
                        state.outputs.iter().map(|o| o.name.clone()).collect();
                    for name in output_names {
                        state.wallpapers.insert(name, wallpaper_type.clone());
                    }
                } else {
                    state.wallpapers.insert(filter.to_string(), wallpaper_type);
                }
            } else {
                // Apply to all outputs
                let output_names: Vec<String> =
                    state.outputs.iter().map(|o| o.name.clone()).collect();
                for name in output_names {
                    state.wallpapers.insert(name, wallpaper_type.clone());
                }
            }
        }

        Ok(())
    }

    fn set_overlay_shader(
        &mut self,
        overlay_name: &str,
        params: crate::overlay_shader::OverlayParams,
        output_filter: Option<&str>,
        _qh: &QueueHandle<Self>,
    ) -> Result<()> {
        log::info!(
            "Setting overlay shader: {} for output: {:?}",
            overlay_name,
            output_filter
        );

        let overlay = crate::overlay_shader::OverlayShader::from_str(overlay_name, &params)
            .ok_or_else(|| anyhow::anyhow!("Unknown overlay: {}. Available: vignette, scanlines, film-grain, chromatic, crt, pixelate, tint", overlay_name))?;

        for output_data in &mut self.outputs {
            if !output_data.configured {
                continue;
            }

            // Check output filter
            if let Some(filter) = output_filter {
                if let Some(info) = self.output_state.info(&output_data.output) {
                    if let Some(name) = &info.name {
                        if name != filter && filter != "all" {
                            continue;
                        }
                    }
                }
            }

            let overlay_mgr = crate::overlay_shader::OverlayManager::new(overlay);
            output_data.overlay_manager = Some(overlay_mgr);

            log::info!("Applied overlay '{}' to output", overlay_name);
        }

        Ok(())
    }

    fn clear_overlay_shader(&mut self, output_filter: Option<&str>) -> Result<()> {
        log::info!("Clearing overlay shader for output: {:?}", output_filter);

        for output_data in &mut self.outputs {
            if !output_data.configured {
                continue;
            }

            // Check output filter
            if let Some(filter) = output_filter {
                if let Some(info) = self.output_state.info(&output_data.output) {
                    if let Some(name) = &info.name {
                        if name != filter && filter != "all" {
                            continue;
                        }
                    }
                }
            }

            output_data.overlay_manager = None;
            log::info!("Cleared overlay from output");
        }

        Ok(())
    }

    /// Apply overlay effect to frame data
    /// Strategy: Use CPU overlays for videos (faster due to no GPU roundtrip)
    ///           Use GPU overlays for static content where possible
    #[cfg(feature = "gpu")]
    fn apply_overlay_to_frame(
        output_data: &mut OutputData,
        frame_data: &mut Vec<u8>,
        width: u32,
        height: u32,
    ) -> Result<()> {
        if let Some(overlay_mgr) = &mut output_data.overlay_manager {
            // For now, always use CPU overlay to avoid frame drops
            // TODO: Re-enable GPU overlays when we have full GPU pipeline
            overlay_mgr.apply_overlay(frame_data, width, height)?;
        }
        Ok(())
    }

    /// Apply overlay effect to frame data using CPU only
    #[cfg(not(feature = "gpu"))]
    fn apply_overlay_to_frame(
        output_data: &mut OutputData,
        frame_data: &mut Vec<u8>,
        width: u32,
        height: u32,
    ) -> Result<()> {
        if let Some(overlay_mgr) = &mut output_data.overlay_manager {
            overlay_mgr.apply_overlay(frame_data, width, height)?;
        }
        Ok(())
    }

    fn update_gif_frames(&mut self, qh: &QueueHandle<Self>) -> Result<()> {
        use std::time::Instant;
        let start = Instant::now();

        // Phase 1: Parallel - Check for new frames and extract frame data
        let updates: Vec<FrameUpdate> = self
            .outputs
            .par_iter_mut()
            .enumerate()
            .filter_map(|(idx, output_data)| {
                // Skip if no GIF manager
                let gif_manager = output_data.gif_manager.as_mut()?;

                // Check if we need to update the frame
                if !gif_manager.update() {
                    return None; // No frame change needed yet
                }

                log::debug!(
                    "Advancing to GIF frame {}/{}",
                    gif_manager.current_frame_index() + 1,
                    gif_manager.frame_count()
                );

                // Get the pre-scaled frame data
                let argb_data = gif_manager.current_frame_data().to_vec();

                Some(FrameUpdate {
                    output_index: idx,
                    argb_data,
                    width: output_data.width,
                    height: output_data.height,
                })
            })
            .collect();

        let parallel_time = start.elapsed();

        // Phase 2: Sequential - Create buffers and perform Wayland operations
        let mut buffers_updated = 0;
        for update in updates {
            let output_data = &mut self.outputs[update.output_index];

            // Apply overlay if present
            let mut final_data = update.argb_data;
            if let Err(e) = Self::apply_overlay_to_frame(
                output_data,
                &mut final_data,
                update.width,
                update.height,
            ) {
                log::warn!("Failed to apply overlay to GIF frame: {}", e);
            }

            // Create new buffer and render (no scaling needed!)
            let mut buffer =
                crate::buffer::ShmBuffer::new(self.shm.wl_shm(), update.width, update.height, qh)?;
            buffer.write_image_data(&final_data)?;

            // Attach and commit
            if let Some(layer_surface) = &output_data.layer_surface {
                layer_surface
                    .wl_surface()
                    .attach(Some(buffer.buffer()), 0, 0);
                layer_surface.wl_surface().damage_buffer(
                    0,
                    0,
                    update.width as i32,
                    update.height as i32,
                );
                layer_surface.wl_surface().commit();
            }

            output_data.buffer = Some(buffer);
            buffers_updated += 1;
        }

        let total_time = start.elapsed();

        // Log performance stats occasionally
        if buffers_updated > 0 {
            static mut UPDATE_COUNTER: u32 = 0;
            unsafe {
                UPDATE_COUNTER += 1;
                if UPDATE_COUNTER % 50 == 0 {
                    log::debug!(
                        "GIF frame update: {} outputs in {:.2}ms (parallel: {:.2}ms, sequential: {:.2}ms)",
                        buffers_updated,
                        total_time.as_secs_f64() * 1000.0,
                        parallel_time.as_secs_f64() * 1000.0,
                        (total_time - parallel_time).as_secs_f64() * 1000.0
                    );
                }
            }
        }

        Ok(())
    }

    #[cfg(feature = "video")]
    fn set_video_wallpaper(
        &mut self,
        path: &str,
        output_filter: Option<&str>,
        scale: common::ScaleMode,
        _transition: Option<common::TransitionType>,
        qh: &QueueHandle<Self>,
    ) -> Result<()> {
        // TODO: Implement transitions for videos
        // For now, we just apply immediately
        log::info!(
            "Setting video wallpaper: {} for output: {:?}",
            path,
            output_filter
        );

        // Apply to matching outputs
        for output_data in &mut self.outputs {
            if !output_data.configured {
                continue;
            }

            // Check if this output matches the filter
            if let Some(filter) = output_filter {
                if let Some(info) = self.output_state.info(&output_data.output) {
                    if let Some(name) = &info.name {
                        if name != filter && filter != "all" {
                            continue;
                        }
                    }
                }
            }

            let width = output_data.width;
            let height = output_data.height;

            if width == 0 || height == 0 {
                continue;
            }

            // Clear any GIF manager (can't have both)
            output_data.gif_manager = None;

            // Load video with VideoManager
            let mut video_manager = crate::video_manager::VideoManager::load(
                path, width, height, scale, true, // muted by default
            )?;

            // Start playback
            video_manager.play()?;

            log::info!("Loaded and started video for output {}x{}", width, height);

            output_data.video_manager = Some(video_manager);
        }

        // Update shared state
        if let Ok(mut state) = self.state.try_lock() {
            let wallpaper_type = common::WallpaperType::Video(path.to_string());
            if let Some(filter) = output_filter {
                if filter == "all" {
                    let output_names: Vec<String> =
                        state.outputs.iter().map(|o| o.name.clone()).collect();
                    for name in output_names {
                        state.wallpapers.insert(name, wallpaper_type.clone());
                    }
                } else {
                    state.wallpapers.insert(filter.to_string(), wallpaper_type);
                }
            } else {
                // Apply to all outputs
                let output_names: Vec<String> =
                    state.outputs.iter().map(|o| o.name.clone()).collect();
                for name in output_names {
                    state.wallpapers.insert(name, wallpaper_type.clone());
                }
            }
        }

        Ok(())
    }

    #[cfg(not(feature = "video"))]
    fn set_video_wallpaper(
        &mut self,
        _path: &str,
        _output_filter: Option<&str>,
        _scale: common::ScaleMode,
        _transition: Option<common::TransitionType>,
        _qh: &QueueHandle<Self>,
    ) -> Result<()> {
        anyhow::bail!("Video support not compiled in. Build with --features video")
    }

    #[cfg(feature = "video")]
    fn update_video_frames(&mut self, qh: &QueueHandle<Self>) -> Result<()> {
        use std::time::Instant;
        let start = Instant::now();

        // Phase 1: Parallel - Check for new frames and extract frame data
        // We can't parallelize the mutable iteration directly, so we collect indices
        // of outputs that need updates, then process them in parallel
        let updates: Vec<FrameUpdate> = self
            .outputs
            .par_iter_mut()
            .enumerate()
            .filter_map(|(idx, output_data)| {
                // Skip if no video manager
                let video_manager = output_data.video_manager.as_mut()?;

                // Check if there's a new frame to render
                if !video_manager.update() {
                    return None; // No new frame yet
                }

                // Get the current frame data
                let argb_data = video_manager.current_frame_data()?;

                Some(FrameUpdate {
                    output_index: idx,
                    argb_data,
                    width: output_data.width,
                    height: output_data.height,
                })
            })
            .collect();

        let parallel_time = start.elapsed();

        // Phase 2: Sequential - Apply buffer updates and Wayland operations
        let mut buffers_updated = 0;
        for update in updates {
            let output_data = &mut self.outputs[update.output_index];

            log::trace!(
                "Rendering video frame for output {}x{}",
                update.width,
                update.height
            );

            // Apply overlay if present
            let mut final_data = update.argb_data;
            if let Err(e) = Self::apply_overlay_to_frame(
                output_data,
                &mut final_data,
                update.width,
                update.height,
            ) {
                log::warn!("Failed to apply overlay to video frame: {}", e);
            }

            // Reuse existing buffer if possible, otherwise create new one
            if let Some(buffer) = &mut output_data.buffer {
                // Reuse existing buffer (just update data)
                if let Err(e) = buffer.write_image_data(&final_data) {
                    log::warn!("Failed to reuse buffer, creating new one: {}", e);
                    // Create new buffer as fallback
                    let mut new_buffer = crate::buffer::ShmBuffer::new(
                        self.shm.wl_shm(),
                        update.width,
                        update.height,
                        qh,
                    )?;
                    new_buffer.write_image_data(&final_data)?;
                    output_data.buffer = Some(new_buffer);
                }
            } else {
                // No existing buffer, create new one
                let mut buffer = crate::buffer::ShmBuffer::new(
                    self.shm.wl_shm(),
                    update.width,
                    update.height,
                    qh,
                )?;
                buffer.write_image_data(&final_data)?;
                output_data.buffer = Some(buffer);
            }

            // Attach and commit
            if let Some(layer_surface) = &output_data.layer_surface {
                if let Some(buffer) = &output_data.buffer {
                    layer_surface
                        .wl_surface()
                        .attach(Some(buffer.buffer()), 0, 0);
                    layer_surface.wl_surface().damage_buffer(
                        0,
                        0,
                        update.width as i32,
                        update.height as i32,
                    );
                    layer_surface.wl_surface().commit();
                }
            }

            buffers_updated += 1;
        }

        let total_time = start.elapsed();

        // Log performance stats occasionally (every 100th update with changes)
        if buffers_updated > 0 {
            static mut UPDATE_COUNTER: u32 = 0;
            unsafe {
                UPDATE_COUNTER += 1;
                if UPDATE_COUNTER % 100 == 0 {
                    log::debug!(
                        "Video frame update: {} outputs in {:.2}ms (parallel: {:.2}ms, sequential: {:.2}ms)",
                        buffers_updated,
                        total_time.as_secs_f64() * 1000.0,
                        parallel_time.as_secs_f64() * 1000.0,
                        (total_time - parallel_time).as_secs_f64() * 1000.0
                    );
                }
            }
        }

        Ok(())
    }

    #[cfg(not(feature = "video"))]
    fn update_video_frames(&mut self, _qh: &QueueHandle<Self>) -> Result<()> {
        Ok(())
    }

    /// Update shader frames
    fn update_shader_frames(&mut self, qh: &QueueHandle<Self>) -> Result<()> {
        for output_data in &mut self.outputs {
            let shader_mgr = match &mut output_data.shader_manager {
                Some(mgr) => mgr,
                None => continue,
            };

            // Check if it's time to render next frame
            if !shader_mgr.should_render() {
                continue;
            }

            let width = output_data.width;
            let height = output_data.height;

            // Render shader frame
            let mut frame_data = shader_mgr.render_frame(width, height)?;

            // Apply overlay if present
            if let Err(e) =
                Self::apply_overlay_to_frame(output_data, &mut frame_data, width, height)
            {
                log::warn!("Failed to apply overlay to shader frame: {}", e);
            }

            // Update buffer
            if let Some(buffer) = &mut output_data.buffer {
                if let Err(e) = buffer.write_image_data(&frame_data) {
                    log::warn!("Failed to update shader buffer: {}", e);
                    continue;
                }
            } else {
                let mut buffer =
                    crate::buffer::ShmBuffer::new(&self.shm.wl_shm(), width, height, qh)?;
                buffer.write_image_data(&frame_data)?;
                output_data.buffer = Some(buffer);
            }

            // Commit to Wayland
            if let Some(layer_surface) = &output_data.layer_surface {
                if let Some(buffer) = &output_data.buffer {
                    layer_surface
                        .wl_surface()
                        .attach(Some(buffer.buffer()), 0, 0);
                    layer_surface
                        .wl_surface()
                        .damage_buffer(0, 0, width as i32, height as i32);
                    layer_surface.wl_surface().commit();
                }
            }
        }

        Ok(())
    }

    /// Update active transitions
    fn update_transitions(&mut self, qh: &QueueHandle<Self>) -> Result<()> {
        for output_data in &mut self.outputs {
            let Some(transition) = &output_data.transition else {
                continue; // No active transition
            };

            if transition.is_complete() {
                // Transition finished, commit the new wallpaper
                log::info!(
                    "Transition complete for output {}x{}",
                    output_data.width,
                    output_data.height
                );

                // Commit the final pending wallpaper before clearing state
                if let Some(pending_data) = &output_data.pending_wallpaper_data {
                    let width = output_data.width;
                    let height = output_data.height;

                    // Apply overlay if present
                    let mut final_data = pending_data.clone();
                    if let Err(e) =
                        Self::apply_overlay_to_frame(output_data, &mut final_data, width, height)
                    {
                        log::warn!("Failed to apply overlay after transition: {}", e);
                    }

                    // Update buffer with final wallpaper
                    if let Some(buffer) = &mut output_data.buffer {
                        if let Err(e) = buffer.write_image_data(&final_data) {
                            log::warn!("Failed to write final wallpaper after transition: {}", e);
                            let mut new_buffer = crate::buffer::ShmBuffer::new(
                                self.shm.wl_shm(),
                                width,
                                height,
                                qh,
                            )?;
                            new_buffer.write_image_data(&final_data)?;
                            output_data.buffer = Some(new_buffer);
                        }
                    } else {
                        let mut buffer =
                            crate::buffer::ShmBuffer::new(self.shm.wl_shm(), width, height, qh)?;
                        buffer.write_image_data(&final_data)?;
                        output_data.buffer = Some(buffer);
                    }

                    // Commit to Wayland
                    if let Some(layer_surface) = &output_data.layer_surface {
                        if let Some(buffer) = &output_data.buffer {
                            layer_surface
                                .wl_surface()
                                .attach(Some(buffer.buffer()), 0, 0);
                            layer_surface.wl_surface().damage_buffer(
                                0,
                                0,
                                width as i32,
                                height as i32,
                            );
                            layer_surface.wl_surface().commit();
                        }
                    }
                }

                // Clear transition state
                output_data.transition = None;
                output_data.pending_wallpaper_data = None;
                continue;
            }

            // Get the new frame data (pending wallpaper or current content)
            let new_frame = if let Some(pending) = &output_data.pending_wallpaper_data {
                pending.clone()
            } else {
                // No pending data, skip this transition update
                continue;
            };

            // Blend the frames
            let blended_frame = transition.blend_frames(&new_frame);

            let width = output_data.width;
            let height = output_data.height;

            // Create/update buffer with blended frame
            if let Some(buffer) = &mut output_data.buffer {
                if let Err(e) = buffer.write_image_data(&blended_frame) {
                    log::warn!("Failed to reuse buffer during transition: {}", e);
                    let mut new_buffer =
                        crate::buffer::ShmBuffer::new(self.shm.wl_shm(), width, height, qh)?;
                    new_buffer.write_image_data(&blended_frame)?;
                    output_data.buffer = Some(new_buffer);
                }
            } else {
                let mut buffer =
                    crate::buffer::ShmBuffer::new(self.shm.wl_shm(), width, height, qh)?;
                buffer.write_image_data(&blended_frame)?;
                output_data.buffer = Some(buffer);
            }

            // Attach and commit
            if let Some(layer_surface) = &output_data.layer_surface {
                if let Some(buffer) = &output_data.buffer {
                    layer_surface
                        .wl_surface()
                        .attach(Some(buffer.buffer()), 0, 0);
                    layer_surface
                        .wl_surface()
                        .damage_buffer(0, 0, width as i32, height as i32);
                    layer_surface.wl_surface().commit();
                }
            }
        }

        Ok(())
    }

    /// Calculate the optimal sleep duration based on next expected frame
    fn get_next_frame_delay(&self) -> std::time::Duration {
        use std::time::Duration;

        let mut min_delay = Duration::from_millis(16); // Cap at ~60fps (16ms)

        // Check for active transitions (need 60fps updates)
        for output_data in &self.outputs {
            if output_data.transition.is_some() {
                // Transition active, update at 60fps
                let transition_rate = Duration::from_millis(16);
                if transition_rate < min_delay {
                    min_delay = transition_rate;
                }
            }
        }

        // Check GIF managers for next frame time
        for output_data in &self.outputs {
            if let Some(gif_manager) = &output_data.gif_manager {
                let delay = gif_manager.time_until_next_frame();
                if delay < min_delay {
                    min_delay = delay;
                }
            }
        }

        // Videos produce frames asynchronously, so we check more frequently
        // but not too frequently to waste CPU
        for output_data in &self.outputs {
            if let Some(video_manager) = &output_data.video_manager {
                // Use detected frame duration, or fall back to 8ms polling
                // We poll at half the frame duration to catch frames promptly
                let video_poll_rate = video_manager.frame_duration() / 2;
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

    fn check_playlist_rotation(&mut self, qh: &QueueHandle<Self>) -> Result<()> {
        // Check if we have a playlist and if it's time to rotate
        let should_rotate = {
            if let Ok(state) = self.state.try_lock() {
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
            if let Ok(mut state) = self.state.try_lock() {
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
            let transition_type = match transition.as_str() {
                "none" => common::TransitionType::None,
                "fade" => common::TransitionType::Fade {
                    duration_ms: duration as u32,
                },
                "wipe-left" => common::TransitionType::WipeLeft {
                    duration_ms: duration as u32,
                },
                "wipe-right" => common::TransitionType::WipeRight {
                    duration_ms: duration as u32,
                },
                "wipe-top" => common::TransitionType::WipeTop {
                    duration_ms: duration as u32,
                },
                "wipe-bottom" => common::TransitionType::WipeBottom {
                    duration_ms: duration as u32,
                },
                "wipe-angle" => common::TransitionType::WipeAngle {
                    angle_degrees: 45.0,
                    duration_ms: duration as u32,
                },
                "center" => common::TransitionType::Center {
                    duration_ms: duration as u32,
                },
                "outer" => common::TransitionType::Outer {
                    duration_ms: duration as u32,
                },
                "random" => {
                    use rand::Rng;
                    let mut rng = rand::thread_rng();
                    let dur_ms = duration as u32;
                    match rng.gen_range(0..8) {
                        0 => common::TransitionType::Fade {
                            duration_ms: dur_ms,
                        },
                        1 => common::TransitionType::WipeLeft {
                            duration_ms: dur_ms,
                        },
                        2 => common::TransitionType::WipeRight {
                            duration_ms: dur_ms,
                        },
                        3 => common::TransitionType::WipeTop {
                            duration_ms: dur_ms,
                        },
                        4 => common::TransitionType::WipeBottom {
                            duration_ms: dur_ms,
                        },
                        5 => common::TransitionType::WipeAngle {
                            angle_degrees: 45.0,
                            duration_ms: dur_ms,
                        },
                        6 => common::TransitionType::Center {
                            duration_ms: dur_ms,
                        },
                        _ => common::TransitionType::Outer {
                            duration_ms: dur_ms,
                        },
                    }
                }
                _ => common::TransitionType::Fade {
                    duration_ms: duration as u32,
                },
            };

            // Set the wallpaper
            let cmd = crate::WallpaperCommand::SetImage {
                path: path.to_string_lossy().to_string(),
                output: None, // Apply to all outputs
                scale: common::ScaleMode::Fill,
                transition: Some(transition_type),
            };

            self.handle_wallpaper_command(cmd, qh)?;
        }

        Ok(())
    }

    fn check_schedule(&mut self, qh: &QueueHandle<Self>) -> Result<()> {
        // Check if scheduler says we should switch wallpaper
        let scheduled_wallpaper = {
            if let Ok(mut state) = self.state.try_lock() {
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
            let transition_type = match scheduled.transition.as_str() {
                "none" => common::TransitionType::None,
                "fade" => common::TransitionType::Fade {
                    duration_ms: duration,
                },
                "wipe-left" => common::TransitionType::WipeLeft {
                    duration_ms: duration,
                },
                "wipe-right" => common::TransitionType::WipeRight {
                    duration_ms: duration,
                },
                "wipe-top" => common::TransitionType::WipeTop {
                    duration_ms: duration,
                },
                "wipe-bottom" => common::TransitionType::WipeBottom {
                    duration_ms: duration,
                },
                "wipe-angle" => common::TransitionType::WipeAngle {
                    angle_degrees: 45.0,
                    duration_ms: duration,
                },
                "center" => common::TransitionType::Center {
                    duration_ms: duration,
                },
                "outer" => common::TransitionType::Outer {
                    duration_ms: duration,
                },
                "random" => {
                    use rand::Rng;
                    let mut rng = rand::thread_rng();
                    match rng.gen_range(0..8) {
                        0 => common::TransitionType::Fade {
                            duration_ms: duration,
                        },
                        1 => common::TransitionType::WipeLeft {
                            duration_ms: duration,
                        },
                        2 => common::TransitionType::WipeRight {
                            duration_ms: duration,
                        },
                        3 => common::TransitionType::WipeTop {
                            duration_ms: duration,
                        },
                        4 => common::TransitionType::WipeBottom {
                            duration_ms: duration,
                        },
                        5 => common::TransitionType::WipeAngle {
                            angle_degrees: 45.0,
                            duration_ms: duration,
                        },
                        6 => common::TransitionType::Center {
                            duration_ms: duration,
                        },
                        _ => common::TransitionType::Outer {
                            duration_ms: duration,
                        },
                    }
                }
                _ => common::TransitionType::Fade {
                    duration_ms: duration,
                },
            };

            // Set the wallpaper
            let cmd = crate::WallpaperCommand::SetImage {
                path: scheduled.path.to_string_lossy().to_string(),
                output: None, // Apply to all outputs
                scale: common::ScaleMode::Fill,
                transition: Some(transition_type),
            };

            self.handle_wallpaper_command(cmd, qh)?;
        }

        Ok(())
    }

    /// Check and update resource monitor
    fn check_resources(&mut self) -> Result<()> {
        // Only check periodically (every 5 seconds)
        if !self.resource_monitor.should_check() {
            return Ok(());
        }

        // Update stats and possibly adjust performance mode
        let stats = self.resource_monitor.update()?;
        let mode = self.resource_monitor.mode();

        // Update shared state with latest stats
        if let Ok(mut state) = self.state.try_lock() {
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
    fn apply_initial_config(&mut self, qh: &QueueHandle<Self>) -> Result<()> {
        let state_lock = self.state.try_lock();
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

                let scale_mode = parse_scale_mode(&output_cfg.scale);

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
        if commands.is_empty() {
            if let Some(ref playlist) = state.playlist {
                if let Some(first) = playlist.current() {
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
            }
        }

        // Drop the lock before applying commands
        drop(state);

        // Apply all collected commands
        for cmd in commands {
            self.handle_wallpaper_command(cmd, qh)?;
        }

        Ok(())
    }
}

/// Parse scale mode string to ScaleMode enum
fn parse_scale_mode(scale: &str) -> common::ScaleMode {
    match scale {
        "center" => common::ScaleMode::Center,
        "fill" => common::ScaleMode::Fill,
        "fit" => common::ScaleMode::Fit,
        "stretch" => common::ScaleMode::Stretch,
        "tile" => common::ScaleMode::Tile,
        _ => common::ScaleMode::Fill,
    }
}

impl CompositorHandler for WallpaperDaemon {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
        // Handle scale factor changes
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
        // Handle transform changes
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        // Handle frame callbacks for animations
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for WallpaperDaemon {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        log::info!("New output detected (output id: {:?})", output.id());
        if let Err(e) = self.create_layer_surface(output, qh) {
            log::error!("Failed to create layer surface: {}", e);
        }
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if let Some(info) = self.output_state.info(&output) {
            log::info!(
                "Output updated: {:?} (id: {:?}) - {}x{} @ {}",
                info.name,
                output.id(),
                info.logical_size.map(|(w, _)| w).unwrap_or(0),
                info.logical_size.map(|(_, h)| h).unwrap_or(0),
                info.scale_factor,
            );

            // Update our shared state with output info
            if let Ok(mut state) = self.state.try_lock() {
                let output_info = common::OutputInfo {
                    name: info.name.clone().unwrap_or_else(|| "Unknown".to_string()),
                    width: info.logical_size.map(|(w, _)| w as u32).unwrap_or(0),
                    height: info.logical_size.map(|(_, h)| h as u32).unwrap_or(0),
                    scale: info.scale_factor as f64,
                    refresh_rate: None,
                };

                // Update or add output info
                if let Some(existing) = state
                    .outputs
                    .iter_mut()
                    .find(|o| o.name == output_info.name)
                {
                    *existing = output_info;
                    log::debug!("Updated existing output in shared state: {}", existing.name);
                } else {
                    log::info!("Added new output to shared state: {}", output_info.name);
                    state.outputs.push(output_info);
                }
            } else {
                log::warn!("Could not acquire state lock to update output info");
            }
        } else {
            log::warn!(
                "Output updated but no info available (id: {:?})",
                output.id()
            );
        }
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        log::info!("Output destroyed");
        self.outputs.retain(|o| o.output != output);
    }
}

impl LayerShellHandler for WallpaperDaemon {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        log::info!("Layer surface closed");
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let (width, height) = configure.new_size;
        log::info!("Layer surface configured: {}x{}", width, height);

        // Find the output data for this layer surface
        let output_data = self
            .outputs
            .iter_mut()
            .find(|o| o.layer_surface.as_ref() == Some(layer));

        if let Some(output_data) = output_data {
            output_data.width = width;
            output_data.height = height;
            output_data.configured = true;

            // Assign shared GPU renderer to this output if available
            #[cfg(feature = "gpu")]
            if output_data.gpu_renderer.is_none() {
                output_data.gpu_renderer = self.gpu_renderer.clone();
            }

            // Create a buffer and render a default dark gray color
            if width > 0 && height > 0 {
                match crate::buffer::ShmBuffer::new(self.shm.wl_shm(), width, height, qh) {
                    Ok(mut buffer) => {
                        // Fill with dark gray (#1e1e1e)
                        buffer.fill_color(0x1e, 0x1e, 0x1e, 0xff);

                        // Attach buffer and commit
                        layer.wl_surface().attach(Some(buffer.buffer()), 0, 0);
                        layer.wl_surface().commit();

                        output_data.buffer = Some(buffer);
                        log::info!("Rendered default color to output");
                    }
                    Err(e) => {
                        log::error!("Failed to create buffer: {}", e);
                        layer.wl_surface().commit();
                    }
                }
            } else {
                layer.wl_surface().commit();
            }
        }
    }
}

impl ProvidesRegistryState for WallpaperDaemon {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState];
}

impl ShmHandler for WallpaperDaemon {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

// Implement Dispatch for wl_buffer (no-op, we don't handle buffer events)
impl Dispatch<wl_buffer::WlBuffer, ()> for WallpaperDaemon {
    fn event(
        _state: &mut Self,
        _proxy: &wl_buffer::WlBuffer,
        _event: <wl_buffer::WlBuffer as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        // No buffer events to handle
    }
}

// Implement Dispatch for wl_shm_pool (no-op, we don't handle pool events)
impl Dispatch<wl_shm_pool::WlShmPool, ()> for WallpaperDaemon {
    fn event(
        _state: &mut Self,
        _proxy: &wl_shm_pool::WlShmPool,
        _event: <wl_shm_pool::WlShmPool as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        // No pool events to handle
    }
}

delegate_compositor!(WallpaperDaemon);
delegate_output!(WallpaperDaemon);
delegate_layer!(WallpaperDaemon);
delegate_shm!(WallpaperDaemon);
delegate_registry!(WallpaperDaemon);
