//! Wallpaper command handlers.
//!
//! This module provides handlers for different wallpaper types:
//! - **image**: Static image wallpapers (PNG, JPG, GIF conversion)
//! - **video**: Video wallpapers with hardware decoding
//! - **shader**: Procedural shader wallpapers (plasma, waves, etc.)
//! - **color**: Solid color wallpapers
//!
//! Each submodule handles the specifics of loading, rendering, and applying
//! its wallpaper type to Wayland outputs.

use super::WallpaperDaemon;
use crate::WallpaperCommand;
use anyhow::Result;
use wayland_client::QueueHandle;

mod color;
mod image;
mod shader;
mod video;

pub(in crate::wayland) use color::set_color_wallpaper;
pub(in crate::wayland) use image::set_image_wallpaper;
pub(in crate::wayland) use shader::set_shader_wallpaper;
pub(in crate::wayland) use video::set_video_wallpaper;

/// Remove VideoManagers that are no longer referenced by any output.
///
/// After switching an output away from video (to image/shader/color),
/// the output's `video_path` is cleared. If no remaining output references
/// a given video path, the VideoManager is orphaned and should be dropped
/// to free GStreamer resources.
#[cfg(feature = "video")]
fn cleanup_orphaned_video_managers(app_data: &mut WallpaperDaemon) {
    let referenced_paths: std::collections::HashSet<&String> = app_data
        .outputs
        .iter()
        .filter_map(|o| o.video_path.as_ref())
        .collect();

    let initial_count = app_data.video_managers.len();
    app_data
        .video_managers
        .retain(|path, _| referenced_paths.contains(path));

    let removed = initial_count - app_data.video_managers.len();
    if removed > 0 {
        log::info!(
            "Cleaned up {} orphaned VideoManager(s) ({} remaining)",
            removed,
            app_data.video_managers.len()
        );
    }
}

/// Main command handler dispatcher
pub(super) fn handle_wallpaper_command(
    app_data: &mut WallpaperDaemon,
    cmd: WallpaperCommand,
    qh: &QueueHandle<WallpaperDaemon>,
) -> Result<()> {
    let result = match cmd {
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
    };

    // Clean up VideoManagers no longer referenced by any output
    #[cfg(feature = "video")]
    cleanup_orphaned_video_managers(app_data);

    result
}
