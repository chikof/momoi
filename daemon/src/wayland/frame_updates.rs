use super::{FrameUpdate, WallpaperDaemon};
use crate::apply_overlay_or_warn;
use anyhow::Result;
use smithay_client_toolkit::shell::WaylandSurface;
use wayland_client::QueueHandle;

#[cfg(feature = "video")]
pub(super) fn update_video_frames(
    app_data: &mut WallpaperDaemon,
    qh: &QueueHandle<WallpaperDaemon>,
) -> Result<()> {
    use std::time::Instant;
    let start = Instant::now();

    // Single VideoManager per video path, GPU scales to each output resolution
    let mut updates: Vec<FrameUpdate> = Vec::new();

    // Collect output info first to avoid borrow checker issues
    let output_infos: Vec<(usize, String, u32, u32)> = app_data
        .outputs
        .iter()
        .enumerate()
        .filter_map(|(idx, out_data)| {
            let path = out_data.video_path.as_ref()?.clone();
            Some((idx, path, out_data.width, out_data.height))
        })
        .collect();

    // Process each unique video path once
    let mut processed_paths: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (_first_idx, video_path, _, _) in &output_infos {
        // Skip if we already processed this video
        if !processed_paths.insert(video_path.clone()) {
            continue;
        }

        let video_manager_arc = match app_data.video_managers.get(video_path) {
            Some(arc) => arc,
            None => {
                log::warn!("Video path {} not found in shared managers", video_path);
                continue;
            }
        };

        // Lock the shared VideoManager (blocking)
        let mut video_manager = video_manager_arc.blocking_lock();

        // CRITICAL: Call update() to process GStreamer messages (EOS, errors, etc.)
        // This handles video looping and error detection
        video_manager.update();

        // Deduplicate scaling operations: only call GPU once per unique resolution
        // Collect unique resolutions for this video
        let unique_resolutions: Vec<(u32, u32)> = output_infos
            .iter()
            .filter(|(_, path, _, _)| path == video_path)
            .map(|(_, _, w, h)| (*w, *h))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        // Render each unique resolution once and cache the result
        let mut resolution_cache: std::collections::HashMap<(u32, u32), Vec<u8>> =
            std::collections::HashMap::new();

        for (width, height) in unique_resolutions {
            if let Some(frame_data) = video_manager.current_frame_data_scaled(width, height) {
                resolution_cache.insert((width, height), frame_data);
            }
        }

        // Now assign the cached frames to each output
        for (out_idx, out_path, out_width, out_height) in &output_infos {
            if out_path == video_path {
                if let Some(frame_data) = resolution_cache.get(&(*out_width, *out_height)) {
                    updates.push(FrameUpdate {
                        output_index: *out_idx,
                        argb_data: frame_data.clone(),
                        width: *out_width,
                        height: *out_height,
                    });
                }
            }
        }
    }

    let parallel_time = start.elapsed();

    // Apply buffer updates and Wayland operations
    let mut buffers_updated = 0;

    for update in updates {
        let output_data = &mut app_data.outputs[update.output_index];

        log::trace!(
            "Rendering video frame for output {}x{}",
            update.width,
            update.height
        );

        // Video frame is already scaled to monitor size
        let mut final_data = update.argb_data;

        // Apply overlay if present
        apply_overlay_or_warn!(
            super::overlay::apply_overlay_to_frame,
            output_data,
            &mut final_data,
            update.width,
            update.height,
            "video frame"
        );

        // For video frames, reuse the existing buffer when possible (same as shader frames)
        // This avoids creating 60+ new 14MB buffers per second for high-res videos
        if let Some(buffer) = &mut output_data.buffer
            && buffer.width() == update.width
            && buffer.height() == update.height
        {
            if let Err(e) = buffer.write_image_data(&final_data) {
                log::warn!("Failed to reuse video buffer: {}", e);
            } else {
                // Successfully reused buffer - attach and commit

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

                buffers_updated += 1;
                continue;
            }
        }

        // No existing buffer, wrong size, or reuse failed - create new one
        let mut buffer =
            crate::buffer::ShmBuffer::new(app_data.shm.wl_shm(), update.width, update.height, qh)?;

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

        // Replace buffer (move old one to pool for potential reuse)
        output_data.swap_buffer(buffer);
        buffers_updated += 1;
    }

    let total_time = start.elapsed();

    // Log performance stats occasionally (every 100th update with changes)

    if buffers_updated > 0 {
        static mut UPDATE_COUNTER: u32 = 0;

        unsafe {
            UPDATE_COUNTER += 1;

            if UPDATE_COUNTER.is_multiple_of(100) {
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
pub(super) fn update_video_frames(
    _app_data: &mut WallpaperDaemon,
    _qh: &QueueHandle<WallpaperDaemon>,
) -> Result<()> {
    Ok(())
}

/// Update shader frames
pub(super) fn update_shader_frames(
    app_data: &mut WallpaperDaemon,
    qh: &QueueHandle<WallpaperDaemon>,
) -> Result<()> {
    for output_data in &mut app_data.outputs {
        let shader_mgr = match &mut output_data.shader_manager {
            Some(mgr) => mgr,
            None => continue,
        };

        // Check if it's time to render next frame
        if !shader_mgr.should_render() {
            continue;
        }

        let (width, height) = (output_data.width, output_data.height);
        let mut frame_data = shader_mgr.render_frame(width, height)?; // Render shader frame

        // Apply overlay if present
        apply_overlay_or_warn!(
            super::overlay::apply_overlay_to_frame,
            output_data,
            &mut frame_data,
            width,
            height,
            "shader frame"
        );

        // For shader frames, reuse the existing buffer to avoid memory leak
        if let Some(buffer) = &mut output_data.buffer
            && buffer.width() == width
            && buffer.height() == height
        {
            if let Err(e) = buffer.write_image_data(&frame_data) {
                log::warn!("Failed to reuse shader buffer: {}", e);
            } else {
                // Successfully reused buffer
                if let Some(layer_surface) = &output_data.layer_surface {
                    layer_surface
                        .wl_surface()
                        .attach(Some(buffer.buffer()), 0, 0);

                    layer_surface
                        .wl_surface()
                        .damage_buffer(0, 0, width as i32, height as i32);

                    layer_surface.wl_surface().commit();
                }

                continue;
            }
        }

        // Create new buffer if needed
        let mut buffer = crate::buffer::ShmBuffer::new(&app_data.shm.wl_shm(), width, height, qh)?;
        buffer.write_image_data(&frame_data)?;

        // Commit to Wayland
        if let Some(layer_surface) = &output_data.layer_surface {
            layer_surface
                .wl_surface()
                .attach(Some(buffer.buffer()), 0, 0);

            layer_surface
                .wl_surface()
                .damage_buffer(0, 0, width as i32, height as i32);

            layer_surface.wl_surface().commit();
        }

        // Replace buffer directly (no pooling)
        output_data.buffer = Some(buffer);
    }

    Ok(())
}
