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
                apply_overlay_or_warn!(
                    super::overlay::apply_overlay_to_frame,
                    output_data,
                    &mut final_data,
                    width,
                    height,
                    "frame after transition"
                );

                // Update buffer with final wallpaper - reuse if possible
                if let Some(buffer) = &mut output_data.buffer {
                    if buffer.width() == width && buffer.height() == height {
                        buffer.write_image_data(&final_data)?;
                    } else {
                        // Wrong size, create new
                        let mut new_buffer = crate::buffer::ShmBuffer::new(
                            &app_data.shm.wl_shm(),
                            width,
                            height,
                            qh,
                        )?;
                        new_buffer.write_image_data(&final_data)?;
                        output_data.buffer = Some(new_buffer);
                    }
                } else {
                    // No buffer, create new
                    let mut buffer =
                        crate::buffer::ShmBuffer::new(&app_data.shm.wl_shm(), width, height, qh)?;
                    buffer.write_image_data(&final_data)?;
                    output_data.buffer = Some(buffer);
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
                    layer_surface.wl_surface().commit();
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

        // Create/update buffer with blended frame - reuse if possible
        if let Some(buffer) = &mut output_data.buffer {
            if buffer.width() == width && buffer.height() == height {
                buffer.write_image_data(&blended_frame)?;
            } else {
                // Wrong size, create new
                let mut new_buffer =
                    crate::buffer::ShmBuffer::new(&app_data.shm.wl_shm(), width, height, qh)?;
                new_buffer.write_image_data(&blended_frame)?;
                output_data.buffer = Some(new_buffer);
            }
        } else {
            // No buffer, create new
            let mut buffer =
                crate::buffer::ShmBuffer::new(&app_data.shm.wl_shm(), width, height, qh)?;
            buffer.write_image_data(&blended_frame)?;
            output_data.buffer = Some(buffer);
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
            layer_surface.wl_surface().commit();
        }
    }

    Ok(())
}
