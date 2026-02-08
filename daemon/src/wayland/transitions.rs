use anyhow::Result;
use smithay_client_toolkit::shell::WaylandSurface;
use wayland_client::QueueHandle;

use super::WallpaperDaemon;
use crate::apply_overlay_or_warn;

/// Update active transitions
pub(super) fn update_transitions(
    app_data: &mut WallpaperDaemon,
    qh: &QueueHandle<WallpaperDaemon>,
) -> Result<()> {
    for output_data in &mut app_data.outputs {
        // Skip if compositor hasn't signaled readiness for this output
        if !output_data.frame_ready {
            continue;
        }

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
            // Take ownership to avoid cloning — we'll clear it below anyway
            if let Some(mut final_data) = output_data.pending_wallpaper_data.take() {
                let width = output_data.width;
                let height = output_data.height;

                // Apply overlay if present
                apply_overlay_or_warn!(
                    super::overlay::apply_overlay_to_frame,
                    output_data,
                    &mut final_data,
                    width,
                    height,
                    "frame after transition"
                );

                // Update buffer with final wallpaper - reuse if released, otherwise create new
                // IMPORTANT: Only reuse if the compositor has released the buffer (not still reading it)
                let can_reuse = output_data.buffer.as_ref().is_some_and(|buf| {
                    buf.width() == width && buf.height() == height && buf.is_released()
                });

                if can_reuse {
                    // Safe to write directly — compositor is done with this buffer
                    output_data
                        .buffer
                        .as_mut()
                        .unwrap()
                        .write_image_data(&final_data)?;
                } else {
                    // Buffer is busy, wrong size, or missing — create a new one
                    let mut new_buffer =
                        crate::buffer::ShmBuffer::new(&app_data.shm.wl_shm(), width, height, qh)?;
                    new_buffer.write_image_data(&final_data)?;
                    // Move old busy buffer to pool to avoid tearing
                    output_data.swap_buffer(new_buffer);
                }

                // Commit to Wayland
                if let Some(layer_surface) = &output_data.layer_surface
                    && let Some(buffer) = &output_data.buffer
                {
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
            }

            // Clear transition state
            output_data.transition = None;
            output_data.pending_wallpaper_data = None;
            continue;
        }

        // Get the new frame data (pending wallpaper or current content)
        let new_frame = match &output_data.pending_wallpaper_data {
            Some(pending) => pending,
            None => continue, // No pending data, skip this transition update
        };

        // Blend the frames (borrows new_frame, no clone needed)
        // Returns None if GPU is warming up (async readback not ready yet)
        let blended_frame = match transition.blend_frames(new_frame) {
            Some(data) => data,
            None => continue, // GPU warming up, skip this frame
        };

        let width = output_data.width;
        let height = output_data.height;

        // Create/update buffer with blended frame - reuse if released, otherwise create new
        // IMPORTANT: Only reuse if the compositor has released the buffer (not still reading it)
        let can_reuse = output_data
            .buffer
            .as_ref()
            .is_some_and(|buf| buf.width() == width && buf.height() == height && buf.is_released());

        if can_reuse {
            // Safe to write directly — compositor is done with this buffer
            output_data
                .buffer
                .as_mut()
                .unwrap()
                .write_image_data(&blended_frame)?;
        } else {
            // Buffer is busy, wrong size, or missing — create a new one
            let mut new_buffer =
                crate::buffer::ShmBuffer::new(&app_data.shm.wl_shm(), width, height, qh)?;
            new_buffer.write_image_data(&blended_frame)?;
            // Move old busy buffer to pool to avoid tearing
            output_data.swap_buffer(new_buffer);
        }

        // Attach and commit
        if let Some(layer_surface) = &output_data.layer_surface
            && let Some(buffer) = &output_data.buffer
        {
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
    }

    Ok(())
}
