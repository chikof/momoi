//! Shader wallpaper handler.
//!
//! Handles procedural shader wallpapers with support for:
//! - Built-in shaders (plasma, waves, matrix, gradient, starfield)
//! - Custom shader parameters (speed, colors, intensity, etc.)
//! - Shader preset loading from config
//! - GPU-accelerated rendering

use super::super::WallpaperDaemon;
use anyhow::Result;
use wayland_client::QueueHandle;

pub(in crate::wayland) fn set_shader_wallpaper(
    app_data: &mut WallpaperDaemon,
    shader_name: &str,
    output_filter: Option<&str>,
    _transition: Option<common::TransitionType>,
    mut params: Option<common::ShaderParams>,
    _qh: &QueueHandle<WallpaperDaemon>,
) -> Result<()> {
    log::info!(
        "Setting shader wallpaper: {} for output: {:?}",
        shader_name,
        output_filter
    );

    // Check if params contains a preset marker
    if let Some(ref p) = params
        && let Some(ref color1) = p.color1
        && let Some(preset_name) = color1.strip_prefix("preset:")
    {
        // Look up preset in config
        if let Ok(state) = app_data.state.try_lock() {
            if let Some(config) = &state.config {
                if let Some(preset) = config.shader_preset.iter().find(|p| p.name == preset_name) {
                    log::info!("Using shader preset: {}", preset_name);
                    params = Some(preset.to_params());
                } else {
                    log::warn!("Shader preset '{}' not found in config", preset_name);
                    params = None;
                }
            } else {
                log::warn!("Cannot use preset '{}': no config loaded", preset_name);
                params = None;
            }
        }
    }

    // Parse shader type

    let shader = crate::shader_manager::BuiltinShader::from_str(shader_name).ok_or_else(|| {
        anyhow::anyhow!(
            "Unknown shader: {}. Available: plasma, waves, matrix, gradient, starfield",
            shader_name
        )
    })?;

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

        // Create shader manager for this output
        let shader_mgr = crate::shader_manager::ShaderManager::new(
            shader,
            width,
            height,
            params.clone(),
            #[cfg(feature = "gpu")]
            output_data.gpu_renderer.clone(),
        );

        output_data.shader_manager = Some(shader_mgr);

        log::info!(
            "Applied shader '{}' to output {}x{}",
            shader_name,
            width,
            height
        );
    }

    // Update shared state
    if let Ok(mut state) = app_data.state.try_lock() {
        let wallpaper_type = common::WallpaperType::Shader(shader_name.to_string());

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
