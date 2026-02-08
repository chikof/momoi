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

    if output_infos.is_empty() {
        log::trace!("No outputs with video_path set");
        return Ok(());
    }
    log::trace!(
        "Video frame update: {} output(s) with video_path, {} shared manager(s)",
        output_infos.len(),
        app_data.video_managers.len()
    );

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
        // This handles video looping and error detection.
        // Returns true when a new frame is available; skip GPU work if no new frame.
        let has_new_frame = video_manager.update();

        if !has_new_frame {
            log::trace!("No new frame from VideoManager for {}", video_path);
            continue;
        }

        log::debug!("New video frame available for {}", video_path);

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
                log::debug!(
                    "Got scaled video frame for {}x{}: {} bytes",
                    width,
                    height,
                    frame_data.len()
                );
                resolution_cache.insert((width, height), frame_data);
            } else {
                log::debug!(
                    "current_frame_data_scaled returned None for {}x{}",
                    width,
                    height
                );
            }
        }

        // Now assign the cached frames to each output
        // Track how many outputs still need each resolution so we can move
        // (instead of clone) the data for the last consumer.
        let mut resolution_remaining: std::collections::HashMap<(u32, u32), usize> =
            std::collections::HashMap::new();
        for (_, out_path, out_width, out_height) in &output_infos {
            if out_path == video_path {
                if resolution_cache.contains_key(&(*out_width, *out_height)) {
                    *resolution_remaining
                        .entry((*out_width, *out_height))
                        .or_insert(0) += 1;
                }
            }
        }

        for (out_idx, out_path, out_width, out_height) in &output_infos {
            if out_path == video_path {
                let key = (*out_width, *out_height);
                let remaining = resolution_remaining.get_mut(&key);
                let frame_data = match remaining {
                    Some(count) => {
                        *count -= 1;
                        if *count == 0 {
                            // Last consumer: move the data out (no clone)
                            resolution_cache.remove(&key)
                        } else {
                            // More consumers remain: must clone
                            resolution_cache.get(&key).cloned()
                        }
                    }
                    None => None,
                };

                if let Some(argb_data) = frame_data {
                    updates.push(FrameUpdate {
                        output_index: *out_idx,
                        argb_data,
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

    log::debug!(
        "Video frame updates ready: {} update(s) to apply",
        updates.len()
    );

    for update in updates {
        let output_data = &mut app_data.outputs[update.output_index];

        // Skip if compositor hasn't signaled readiness for this output
        if !output_data.frame_ready {
            log::trace!(
                "Skipping video frame for output {}x{}: frame_ready=false",
                update.width,
                update.height
            );
            continue;
        }

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
        // IMPORTANT: Only reuse if the compositor has released the buffer (not still reading it)
        if let Some(buffer) = &mut output_data.buffer
            && buffer.width() == update.width
            && buffer.height() == update.height
            && buffer.is_released()
        {
            if let Err(e) = buffer.write_image_data(&final_data) {
                log::warn!("Failed to reuse video buffer: {}", e);
            } else {
                // Successfully reused buffer - mark busy before attaching
                // to prevent the next iteration from overwriting while compositor reads
                buffer.mark_busy();

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

                    // Request next frame callback before commit
                    // frame() must come BEFORE commit() per Wayland protocol
                    let wl_surface = layer_surface.wl_surface();
                    wl_surface.frame(qh, wl_surface.clone());
                    output_data.frame_ready = false;

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

            // Request next frame callback before commit
            // frame() must come BEFORE commit() per Wayland protocol
            let wl_surface = layer_surface.wl_surface();
            wl_surface.frame(qh, wl_surface.clone());
            output_data.frame_ready = false;

            layer_surface.wl_surface().commit();
        }

        // Replace buffer (move old one to pool for potential reuse)
        output_data.swap_buffer(buffer);
        buffers_updated += 1;
    }

    let total_time = start.elapsed();

    // Log performance stats occasionally (every 100th update with changes)

    if buffers_updated > 0 {
        use std::sync::atomic::{AtomicU32, Ordering};
        static UPDATE_COUNTER: AtomicU32 = AtomicU32::new(0);

        let count = UPDATE_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
        if count.is_multiple_of(100) {
            log::debug!(
                "Video frame update: {} outputs in {:.2}ms (parallel: {:.2}ms, sequential: {:.2}ms)",
                buffers_updated,
                total_time.as_secs_f64() * 1000.0,
                parallel_time.as_secs_f64() * 1000.0,
                (total_time - parallel_time).as_secs_f64() * 1000.0
            );
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
        // Skip if compositor hasn't signaled readiness for this output
        if !output_data.frame_ready {
            continue;
        }

        let shader_mgr = match &mut output_data.shader_manager {
            Some(mgr) => mgr,
            None => continue,
        };

        // Check if it's time to render next frame
        if !shader_mgr.should_render() {
            continue;
        }

        let (width, height) = (output_data.width, output_data.height);
        let mut frame_data = match shader_mgr.render_frame(width, height)? {
            Some(data) => data,
            None => continue, // Async GPU readback warming up, skip this frame
        };

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
        // IMPORTANT: Only reuse if the compositor has released the buffer (not still reading it)
        if let Some(buffer) = &mut output_data.buffer
            && buffer.width() == width
            && buffer.height() == height
            && buffer.is_released()
        {
            if let Err(e) = buffer.write_image_data(&frame_data) {
                log::warn!("Failed to reuse shader buffer: {}", e);
            } else {
                // Successfully reused buffer - mark busy before attaching
                // to prevent the next iteration from overwriting while compositor reads
                buffer.mark_busy();

                if let Some(layer_surface) = &output_data.layer_surface {
                    layer_surface
                        .wl_surface()
                        .attach(Some(buffer.buffer()), 0, 0);

                    layer_surface
                        .wl_surface()
                        .damage_buffer(0, 0, width as i32, height as i32);

                    // Request next frame callback before commit
                    // frame() must come BEFORE commit() per Wayland protocol
                    let wl_surface = layer_surface.wl_surface();
                    wl_surface.frame(qh, wl_surface.clone());
                    output_data.frame_ready = false;

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

            // Request next frame callback before commit
            // frame() must come BEFORE commit() per Wayland protocol
            let wl_surface = layer_surface.wl_surface();
            wl_surface.frame(qh, wl_surface.clone());
            output_data.frame_ready = false;

            layer_surface.wl_surface().commit();
        }

        // Replace buffer (move old busy buffer to pool to avoid tearing)
        output_data.swap_buffer(buffer);
    }

    Ok(())
}
