//! Video wallpaper handler.
//!
//! Handles video wallpapers with support for:
//! - Hardware-accelerated decoding (VA-API)
//! - Shared VideoManager per video path (decode once, scale per output)
//! - GPU-accelerated scaling for different output resolutions
//! - Configurable target FPS limiting
//! - Multi-output synchronization

use super::super::WallpaperDaemon;
use crate::config::default_max_video_fps;
use anyhow::Result;
use wayland_client::QueueHandle;

#[cfg(feature = "video")]
pub(in crate::wayland) fn set_video_wallpaper(
    app_data: &mut WallpaperDaemon,
    path: &str,
    output_filter: Option<&str>,
    scale: common::ScaleMode,
    _transition: Option<common::TransitionType>,
    _qh: &QueueHandle<WallpaperDaemon>,
) -> Result<()> {
    // TODO: Implement transitions for videos
    // For now, we just apply immediately
    log::info!(
        "Setting video wallpaper: {} for output: {:?}",
        path,
        output_filter
    );

    // Calculate max resolution needed BEFORE iterating (to avoid borrow issues)
    let (max_width, max_height) = app_data
        .outputs
        .iter()
        .filter(|o| o.configured && o.width > 0 && o.height > 0)
        .map(|o| (o.width, o.height))
        .max()
        .unwrap_or((1920, 1080)); // Default if no configured outputs

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

        // Clear any old managers (can't have both video and shader)
        output_data.shader_manager = None;

        // Get target FPS from config
        let target_fps = if let Ok(state_guard) = app_data.state.try_lock() {
            state_guard
                .config
                .as_ref()
                .map(|c| c.advanced.max_video_fps)
                .unwrap_or(default_max_video_fps())
        } else {
            default_max_video_fps()
        };

        // Single VideoManager per video path with GPU scaling
        // Decode at maximum resolution needed, GPU scales down for smaller outputs
        let path_key = path.to_string();

        if !app_data.video_managers.contains_key(&path_key) {
            // Calculate max resolution needed across ALL configured outputs

            log::info!(
                "Creating shared VideoManager for {} (decode at {}x{}, will GPU scale to all outputs)",
                path,
                max_width,
                max_height
            );

            let mut video_manager = crate::video::VideoManager::load(
                path,
                max_width,
                max_height,
                scale,
                true,
                target_fps,
                #[cfg(feature = "gpu")]
                app_data.gpu_renderer.clone(),
            )?;

            // Start playback

            video_manager.play()?;

            // Store in shared HashMap (keyed by path only)

            app_data.video_managers.insert(
                path_key.clone(),
                std::sync::Arc::new(tokio::sync::Mutex::new(video_manager)),
            );
        } else {
            log::info!(
                "Reusing existing VideoManager for {} (GPU will scale to {}x{})",
                path,
                width,
                height
            );
        }

        // Set video path reference for this output

        output_data.video_path = Some(path_key);

        log::info!("Set video wallpaper for output {}x{}", width, height);
    }

    // Update shared state

    if let Ok(mut state) = app_data.state.try_lock() {
        let wallpaper_type = common::WallpaperType::Video(path.to_string());

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

#[cfg(not(feature = "video"))]
pub(in crate::wayland) fn set_video_wallpaper(
    _app_data: &mut WallpaperDaemon,
    _path: &str,
    _output_filter: Option<&str>,
    _scale: common::ScaleMode,
    _transition: Option<common::TransitionType>,
    _qh: &QueueHandle<WallpaperDaemon>,
) -> Result<()> {
    anyhow::bail!("Video support not compiled in. Build with --features video")
}
