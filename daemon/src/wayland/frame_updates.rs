use anyhow::Result;
use rayon::prelude::*;
use smithay_client_toolkit::shell::WaylandSurface;
use wayland_client::QueueHandle;

use super::{FrameUpdate, WallpaperDaemon};
use crate::apply_overlay_or_warn;

pub(super) fn update_gif_frames(
    app_data: &mut WallpaperDaemon,
    qh: &QueueHandle<WallpaperDaemon>,
) -> Result<()> {
    use std::time::Instant;
    let start = Instant::now();

    // Phase 1: Parallel - Check for new frames and extract frame data
    let updates: Vec<FrameUpdate> = app_data
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
        let output_data = &mut app_data.outputs[update.output_index];

        // Apply overlay if present
        let mut final_data = update.argb_data;
        apply_overlay_or_warn!(
            super::overlay::apply_overlay_to_frame,
            output_data,
            &mut final_data,
            update.width,
            update.height,
            "GIF frame"
        );

        // Create new buffer and render (no scaling needed!)
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

        // Mark buffer as busy (compositor is using it)
        buffer.mark_busy();

        // Swap buffer (moves old buffer to pool)
        output_data.swap_buffer(buffer);
        output_data.cleanup_buffer_pool();

        buffers_updated += 1;
    }

    let total_time = start.elapsed();

    // Log performance stats occasionally
    if buffers_updated > 0 {
        static mut UPDATE_COUNTER: u32 = 0;
        unsafe {
            UPDATE_COUNTER += 1;
            if UPDATE_COUNTER.is_multiple_of(50) {
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
pub(super) fn update_video_frames(
    app_data: &mut WallpaperDaemon,
    qh: &QueueHandle<WallpaperDaemon>,
) -> Result<()> {
    use std::time::Instant;
    let start = Instant::now();

    // Phase 1: Parallel - Check for new frames and extract frame data
    // We can't parallelize the mutable iteration directly, so we collect indices
    // of outputs that need updates, then process them in parallel
    let updates: Vec<FrameUpdate> = app_data
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
        let output_data = &mut app_data.outputs[update.output_index];

        log::trace!(
            "Rendering video frame for output {}x{}",
            update.width,
            update.height
        );

        // Apply overlay if present
        let mut final_data = update.argb_data;
        apply_overlay_or_warn!(
            super::overlay::apply_overlay_to_frame,
            output_data,
            &mut final_data,
            update.width,
            update.height,
            "video frame"
        );

        // Get or create a buffer (will reuse from pool if available)
        let mut buffer = output_data.get_buffer(&app_data.shm, update.width, update.height, qh)?;
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

        // Mark buffer as busy (compositor is using it)
        buffer.mark_busy();

        // Swap buffer (moves old buffer to pool)
        output_data.swap_buffer(buffer);
        output_data.cleanup_buffer_pool();

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

        let width = output_data.width;
        let height = output_data.height;

        // Render shader frame
        let mut frame_data = shader_mgr.render_frame(width, height)?;

        // Apply overlay if present
        apply_overlay_or_warn!(
            super::overlay::apply_overlay_to_frame,
            output_data,
            &mut frame_data,
            width,
            height,
            "shader frame"
        );

        // Get or create a buffer (will reuse from pool if available)
        let mut buffer = output_data.get_buffer(&app_data.shm, width, height, qh)?;
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

        // Mark buffer as busy (compositor is using it)
        buffer.mark_busy();

        // Swap buffer (moves old buffer to pool)
        output_data.swap_buffer(buffer);
        output_data.cleanup_buffer_pool();
    }

    Ok(())
}
