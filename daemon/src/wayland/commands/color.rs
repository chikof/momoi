//! Color wallpaper handler.
//!
//! Handles solid color wallpapers with support for:
//! - Hex color parsing (#RRGGBB, #RRGGBBAA)
//! - Output filtering (specific output or all)
//! - Direct buffer filling (no GPU required)

use super::super::WallpaperDaemon;
use anyhow::Result;
use smithay_client_toolkit::shell::WaylandSurface;
use wayland_client::QueueHandle;

pub(in crate::wayland) fn set_color_wallpaper(
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
