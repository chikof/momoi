use anyhow::Result;
use smithay_client_toolkit::shell::{
    WaylandSurface,
    wlr_layer::{Anchor, KeyboardInteractivity, Layer},
};
use wayland_client::{QueueHandle, protocol::wl_output};

use super::{OutputData, WallpaperDaemon};
use crate::WallpaperCommand;

/// Synchronize output information to shared state
pub(super) fn sync_outputs_to_shared_state(app_data: &mut WallpaperDaemon) {
    if let Ok(mut state) = app_data.state.try_lock() {
        state.outputs.clear();

        for output_data in &app_data.outputs {
            if let Some(info) = app_data.output_state.info(&output_data.output) {
                let output_info = common::OutputInfo {
                    name: info.name.clone().unwrap_or_else(|| "Unknown".to_string()),
                    width: output_data.width,
                    height: output_data.height,
                    scale: info.scale_factor as f64,
                    refresh_rate: None,
                };
                log::info!(
                    "Added output to shared state: {} ({}x{})",
                    output_info.name,
                    output_info.width,
                    output_info.height
                );
                state.outputs.push(output_info);
            }
        }
    } else {
        log::warn!("Could not acquire state lock to sync outputs");
    }
}

/// Restore wallpapers from shared state after reconnection
pub(super) fn restore_wallpapers_from_state(
    app_data: &mut WallpaperDaemon,
    qh: &QueueHandle<WallpaperDaemon>,
) -> Result<()> {
    let wallpapers = if let Ok(state) = app_data.state.try_lock() {
        state.wallpapers.clone()
    } else {
        log::warn!("Could not acquire state lock to restore wallpapers");
        return Ok(());
    };

    if wallpapers.is_empty() {
        log::debug!("No wallpapers to restore");
        return Ok(());
    }

    log::info!(
        "Restoring {} wallpaper(s) after reconnection",
        wallpapers.len()
    );

    for (output_name, wallpaper_type) in wallpapers {
        let cmd = match wallpaper_type {
            common::WallpaperType::Image(path) => {
                log::info!("Restoring image wallpaper on {}: {}", output_name, path);
                WallpaperCommand::SetImage {
                    path,
                    output: Some(output_name.clone()),
                    scale: common::ScaleMode::Fill, // Default scale mode
                    transition: None,               // No transition on restore
                }
            }
            common::WallpaperType::Video(path) => {
                log::info!("Restoring video wallpaper on {}: {}", output_name, path);
                WallpaperCommand::SetImage {
                    path,
                    output: Some(output_name.clone()),
                    scale: common::ScaleMode::Fill,
                    transition: None,
                }
            }
            common::WallpaperType::Color(color) => {
                log::info!("Restoring color wallpaper on {}: {}", output_name, color);
                WallpaperCommand::SetColor {
                    color,
                    output: Some(output_name.clone()),
                }
            }
            common::WallpaperType::Shader(shader) => {
                log::info!("Restoring shader wallpaper on {}: {}", output_name, shader);
                WallpaperCommand::SetShader {
                    shader,
                    output: Some(output_name.clone()),
                    transition: None,
                    params: None, // Use default params when restoring
                }
            }
            common::WallpaperType::None => {
                log::debug!("Skipping 'None' wallpaper for {}", output_name);
                continue;
            }
        };

        // Apply the wallpaper
        if let Err(e) = super::commands::handle_wallpaper_command(app_data, cmd, qh) {
            log::error!("Failed to restore wallpaper for {}: {}", output_name, e);
        }
    }

    Ok(())
}

/// Create a layer surface for an output
pub(super) fn create_layer_surface(
    app_data: &mut WallpaperDaemon,
    output: wl_output::WlOutput,
    qh: &QueueHandle<WallpaperDaemon>,
) -> Result<()> {
    let surface = app_data.compositor_state.create_surface(qh);

    let layer_surface = app_data.layer_shell.create_layer_surface(
        qh,
        surface,
        Layer::Background,
        Some("wallpaper"),
        Some(&output),
    );

    // Configure the layer surface
    layer_surface.set_anchor(Anchor::all());
    layer_surface.set_exclusive_zone(-1);
    layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer_surface.commit();

    app_data.outputs.push(OutputData {
        output,
        layer_surface: Some(layer_surface),
        buffer: None,
        buffer_pool: Vec::new(),
        width: 0,
        height: 0,
        scale: 1.0,
        configured: false,
        video_manager: None,
        shader_manager: None,
        overlay_manager: None,
        transition: None,
        pending_wallpaper_data: None,
        #[cfg(feature = "gpu")]
        gpu_renderer: None,
    });

    log::info!("Created layer surface for output");
    Ok(())
}
