use anyhow::Result;
use smithay_client_toolkit::shell::WaylandSurface;
use wayland_client::QueueHandle;

use super::WallpaperDaemon;
use crate::{WallpaperCommand, apply_overlay_or_warn};

/// Main command handler dispatcher
pub(super) fn handle_wallpaper_command(
    app_data: &mut WallpaperDaemon,
    cmd: WallpaperCommand,
    qh: &QueueHandle<WallpaperDaemon>,
) -> Result<()> {
    match cmd {
        WallpaperCommand::SetImage {
            path,
            output,
            scale,
            transition,
        } => set_image_wallpaper(app_data, &path, output.as_deref(), scale, transition, qh),
        WallpaperCommand::SetColor { color, output } => {
            set_color_wallpaper(app_data, &color, output.as_deref(), qh)
        }
        WallpaperCommand::SetShader {
            shader,
            output,
            transition,
            params,
        } => set_shader_wallpaper(app_data, &shader, output.as_deref(), transition, params, qh),
        WallpaperCommand::SetOverlay {
            overlay,
            params,
            output,
        } => super::overlay::set_overlay_shader(
            &mut app_data.outputs,
            &app_data.output_state,
            &overlay,
            params,
            output.as_deref(),
        ),
        WallpaperCommand::ClearOverlay { output } => super::overlay::clear_overlay_shader(
            &mut app_data.outputs,
            &app_data.output_state,
            output.as_deref(),
        ),
    }
}

fn set_image_wallpaper(
    app_data: &mut WallpaperDaemon,
    path: &str,
    output_filter: Option<&str>,
    scale: common::ScaleMode,
    transition: Option<common::TransitionType>,
    qh: &QueueHandle<WallpaperDaemon>,
) -> Result<()> {
    log::info!(
        "Setting image wallpaper: {} for output: {:?} with transition: {:?}",
        path,
        output_filter,
        transition
    );

    // Check if this is a video
    if crate::wallpaper_manager::WallpaperManager::is_video(path) {
        log::info!("Detected video file, loading with VideoManager");
        return set_video_wallpaper(app_data, path, output_filter, scale, transition, qh);
    }

    // Check if this is an animated GIF
    let is_animated = crate::wallpaper_manager::WallpaperManager::is_animated_gif(path)?;

    if is_animated {
        log::info!("Detected animated GIF, loading with GifManager");
        return set_animated_gif(app_data, path, output_filter, scale, transition, qh);
    }

    // Load and clone the image (so we don't hold a borrow to wallpaper_manager)
    let image = app_data.wallpaper_manager.load_image(path)?.clone();

    // Apply to matching outputs
    for output_data in &mut app_data.outputs {
        if !output_data.configured {
            continue;
        }

        // Check if this output matches the filter
        if let Some(filter) = output_filter
            && let Some(info) = app_data.output_state.info(&output_data.output)
            && let Some(name) = &info.name
            && name != filter
            && filter != "all"
        {
            continue;
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
                            let scaled = app_data
                                .wallpaper_manager
                                .scale_image(&image, width, height, scale)?;
                            let result = app_data.wallpaper_manager.rgba_to_argb8888(&scaled);
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
                    let scaled = app_data
                        .wallpaper_manager
                        .scale_image(&image, width, height, scale)?;
                    let result = app_data.wallpaper_manager.rgba_to_argb8888(&scaled);
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
                let scaled = app_data
                    .wallpaper_manager
                    .scale_image(&image, width, height, scale)?;
                app_data.wallpaper_manager.rgba_to_argb8888(&scaled)
            }
        };

        // Clear any GIF/video managers (can't have multiple types)
        output_data.gif_manager = None;
        output_data.video_manager = None;

        // Handle transition if requested
        if let Some(ref trans_config) = transition
            && trans_config.duration_ms() > 0
            // Capture current frame as "old frame" for transition
            && let Some(buffer) = &output_data.buffer
            && let Ok(old_frame_data) = buffer.read_data()
        {
            // Start transition
            let transition_type = crate::transition::TransitionType::from(trans_config);
            let duration = std::time::Duration::from_millis(trans_config.duration_ms() as u64);

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

        // No transition or transition setup failed - apply immediately
        // Apply overlay if present
        let mut final_data = argb_data;
        apply_overlay_or_warn!(
            super::overlay::apply_overlay_to_frame,
            output_data,
            &mut final_data,
            width,
            height,
            "image"
        );

        // Create or update buffer
        let mut buffer = crate::buffer::ShmBuffer::new(&app_data.shm.wl_shm(), width, height, qh)?;
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

        // Mark buffer as busy (compositor is using it)
        // Just replace buffer directly

        // Swap buffer (moves old buffer to pool)
        output_data.buffer = Some(buffer);

        log::info!("Applied wallpaper to output {}x{}", width, height);
    }

    // Update shared state
    if let Ok(mut state) = app_data.state.try_lock() {
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
            let output_names: Vec<String> = state.outputs.iter().map(|o| o.name.clone()).collect();
            for name in output_names {
                state.wallpapers.insert(name, wallpaper_type.clone());
            }
        }
    }

    Ok(())
}

fn set_animated_gif(
    app_data: &mut WallpaperDaemon,
    path: &str,
    output_filter: Option<&str>,
    scale: common::ScaleMode,
    _transition: Option<common::TransitionType>,
    qh: &QueueHandle<WallpaperDaemon>,
) -> Result<()> {
    // TODO: Implement transitions for GIFs
    // For now, we just apply immediately
    // Apply to matching outputs
    for output_data in &mut app_data.outputs {
        if !output_data.configured {
            continue;
        }

        // Check if this output matches the filter
        if let Some(filter) = output_filter
            && let Some(info) = app_data.output_state.info(&output_data.output)
            && let Some(name) = &info.name
            && name != filter
            && filter != "all"
        {
            continue;
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
            &app_data.wallpaper_manager,
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
        apply_overlay_or_warn!(
            super::overlay::apply_overlay_to_frame,
            output_data,
            &mut final_data,
            width,
            height,
            "GIF first frame"
        );

        // Create buffer and render
        let mut buffer = crate::buffer::ShmBuffer::new(&app_data.shm.wl_shm(), width, height, qh)?;
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

        // Mark buffer as busy (compositor is using it)
        // Just replace buffer directly

        // Swap buffer (moves old buffer to pool)
        output_data.buffer = Some(buffer);

        output_data.gif_manager = Some(gif_manager);

        log::info!("Applied animated GIF to output {}x{}", width, height);
    }

    // Update shared state
    if let Ok(mut state) = app_data.state.try_lock() {
        let wallpaper_type = common::WallpaperType::Image(path.to_string());
        if let Some(filter) = output_filter {
            if let Some(info) = app_data.output_state.info(
                &app_data
                    .outputs
                    .iter()
                    .find(|o| o.configured)
                    .unwrap()
                    .output,
            ) {
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
            let output_names: Vec<String> = state.outputs.iter().map(|o| o.name.clone()).collect();
            for name in output_names {
                state.wallpapers.insert(name, wallpaper_type.clone());
            }
        }
    }

    Ok(())
}

fn set_color_wallpaper(
    app_data: &mut WallpaperDaemon,
    color: &str,
    output_filter: Option<&str>,
    qh: &QueueHandle<WallpaperDaemon>,
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
    for output_data in &mut app_data.outputs {
        if !output_data.configured {
            continue;
        }

        // Check if this output matches the filter
        if let Some(filter) = output_filter
            && let Some(info) = app_data.output_state.info(&output_data.output)
            && let Some(name) = &info.name
            && name != filter
            && filter != "all"
        {
            continue;
        }

        let width = output_data.width;
        let height = output_data.height;

        if width == 0 || height == 0 {
            continue;
        }

        // Create buffer and fill with color
        let mut buffer = crate::buffer::ShmBuffer::new(app_data.shm.wl_shm(), width, height, qh)?;
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

        // Mark buffer as busy (compositor is using it)
        // Just replace buffer directly

        // Swap buffer (moves old buffer to pool)
        output_data.buffer = Some(buffer);

        log::info!("Applied color to output {}x{}", width, height);
    }

    // Update shared state
    if let Ok(mut state) = app_data.state.try_lock() {
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
            let output_names: Vec<String> = state.outputs.iter().map(|o| o.name.clone()).collect();
            for name in output_names {
                state.wallpapers.insert(name, wallpaper_type.clone());
            }
        }
    }

    Ok(())
}

fn set_shader_wallpaper(
    app_data: &mut WallpaperDaemon,
    shader_name: &str,
    output_filter: Option<&str>,
    _transition: Option<common::TransitionType>,
    mut params: Option<common::ShaderParams>,
    qh: &QueueHandle<WallpaperDaemon>,
) -> Result<()> {
    log::info!(
        "Setting shader wallpaper: {} for output: {:?}",
        shader_name,
        output_filter
    );

    // Check if params contains a preset marker
    if let Some(ref p) = params
        && let Some(ref color1) = p.color1
        && let Some(preset_name) = color1.strip_prefix("preset:")
    {
        // Look up preset in config
        if let Ok(state) = app_data.state.try_lock() {
            if let Some(config) = &state.config {
                if let Some(preset) = config.shader_preset.iter().find(|p| p.name == preset_name) {
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

    // Parse shader type
    let shader = crate::shader_manager::BuiltinShader::from_str(shader_name).ok_or_else(|| {
        anyhow::anyhow!(
            "Unknown shader: {}. Available: plasma, waves, matrix, gradient, starfield",
            shader_name
        )
    })?;

    // Apply to matching outputs
    for output_data in &mut app_data.outputs {
        if !output_data.configured {
            continue;
        }

        // Check if this output matches the filter
        if let Some(filter) = output_filter
            && let Some(info) = app_data.output_state.info(&output_data.output)
            && let Some(name) = &info.name
            && name != filter
            && filter != "all"
        {
            continue;
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
    if let Ok(mut state) = app_data.state.try_lock() {
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
            let output_names: Vec<String> = state.outputs.iter().map(|o| o.name.clone()).collect();
            for name in output_names {
                state.wallpapers.insert(name, wallpaper_type.clone());
            }
        }
    }

    Ok(())
}

#[cfg(feature = "video")]
fn set_video_wallpaper(
    app_data: &mut WallpaperDaemon,
    path: &str,
    output_filter: Option<&str>,
    scale: common::ScaleMode,
    _transition: Option<common::TransitionType>,
    _qh: &QueueHandle<WallpaperDaemon>,
) -> Result<()> {
    // TODO: Implement transitions for videos
    // For now, we just apply immediately
    log::info!(
        "Setting video wallpaper: {} for output: {:?}",
        path,
        output_filter
    );

    // Apply to matching outputs
    for output_data in &mut app_data.outputs {
        if !output_data.configured {
            continue;
        }

        // Check if this output matches the filter
        if let Some(filter) = output_filter
            && let Some(info) = app_data.output_state.info(&output_data.output)
            && let Some(name) = &info.name
            && name != filter
            && filter != "all"
        {
            continue;
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
    if let Ok(mut state) = app_data.state.try_lock() {
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
            let output_names: Vec<String> = state.outputs.iter().map(|o| o.name.clone()).collect();
            for name in output_names {
                state.wallpapers.insert(name, wallpaper_type.clone());
            }
        }
    }

    Ok(())
}

#[cfg(not(feature = "video"))]
fn set_video_wallpaper(
    _app_data: &mut WallpaperDaemon,
    _path: &str,
    _output_filter: Option<&str>,
    _scale: common::ScaleMode,
    _transition: Option<common::TransitionType>,
    _qh: &QueueHandle<WallpaperDaemon>,
) -> Result<()> {
    anyhow::bail!("Video support not compiled in. Build with --features video")
}
