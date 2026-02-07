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
