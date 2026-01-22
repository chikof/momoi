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

                // Update buffer with final wallpaper
                let mut buffer = output_data.get_buffer(&app_data.shm, width, height, qh)?;
                buffer.write_image_data(&final_data)?;

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
        let mut buffer = output_data.get_buffer(&app_data.shm, width, height, qh)?;
        buffer.write_image_data(&blended_frame)?;

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
        buffer.mark_busy();

        // Swap buffer (moves old buffer to pool)
        output_data.swap_buffer(buffer);
        output_data.cleanup_buffer_pool();
    }

    Ok(())
}
