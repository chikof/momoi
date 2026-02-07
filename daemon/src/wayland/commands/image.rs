//! Image wallpaper handler.
//!
//! Handles static image wallpapers (PNG, JPG, etc.) with support for:
//! - GIF detection and conversion to WebM for efficient playback
//! - Video file detection (delegates to video handler)
//! - GPU-accelerated scaling with CPU fallback
//! - Transitions between wallpapers
//! - Overlay shader application

use super::super::WallpaperDaemon;
use crate::apply_overlay_or_warn;
use anyhow::Result;
use smithay_client_toolkit::shell::WaylandSurface;
use wayland_client::QueueHandle;

pub(in crate::wayland) fn set_image_wallpaper(
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

    // Check if this is a GIF (convert to video)
    if crate::wallpaper_manager::WallpaperManager::is_gif(path) {
        log::info!("Detected GIF file, converting to WebM for efficient playback");
        let webm_path = crate::gif_converter::convert_gif_to_webm(path)?;

        log::info!("Using converted WebM: {}", webm_path.display());
        return super::set_video_wallpaper(
            app_data,
            webm_path.to_str().unwrap(),
            output_filter,
            scale,
            transition,
            qh,
        );
    }

    // Check if this is a video
    if crate::wallpaper_manager::WallpaperManager::is_video(path) {
        log::info!("Detected video file, loading with VideoManager");
        return super::set_video_wallpaper(app_data, path, output_filter, scale, transition, qh);
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

        // Clear any video managers (can't have multiple types)
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
            super::super::overlay::apply_overlay_to_frame,
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
