use anyhow::Result;
use smithay_client_toolkit::output::OutputState;
use wayland_client::protocol::wl_output;

use super::OutputData;

/// Set overlay shader for outputs
pub(super) fn set_overlay_shader(
    outputs: &mut [OutputData],
    output_state: &OutputState,
    overlay_name: &str,
    params: crate::overlay_shader::OverlayParams,
    output_filter: Option<&str>,
) -> Result<()> {
    log::info!(
        "Setting overlay shader: {} for output: {:?}",
        overlay_name,
        output_filter
    );

    let overlay = crate::overlay_shader::OverlayShader::from_str(overlay_name, &params)
        .ok_or_else(|| anyhow::anyhow!("Unknown overlay: {}. Available: vignette, scanlines, film-grain, chromatic, crt, pixelate, tint", overlay_name))?;

    for output_data in outputs {
        if !output_data.configured {
            continue;
        }

        // Check output filter
        if let Some(filter) = output_filter
            && let Some(info) = output_state.info(&output_data.output)
            && let Some(name) = &info.name
            && name != filter
            && filter != "all"
        {
            continue;
        }

        let overlay_mgr = crate::overlay_shader::OverlayManager::new(overlay);
        output_data.overlay_manager = Some(overlay_mgr);

        // Get output name for logging
        let output_name = if let Some(info) = output_state.info(&output_data.output) {
            info.name.clone().unwrap_or_else(|| "unknown".to_string())
        } else {
            "unknown".to_string()
        };

        log::info!(
            "Applied overlay '{}' to output '{}' ({}x{})",
            overlay_name,
            output_name,
            output_data.width,
            output_data.height
        );
    }

    Ok(())
}

/// Clear overlay shader for outputs
pub(super) fn clear_overlay_shader(
    outputs: &mut [OutputData],
    output_state: &OutputState,
    output_filter: Option<&str>,
) -> Result<()> {
    log::info!("Clearing overlay shader for output: {:?}", output_filter);

    for output_data in outputs {
        if !output_data.configured {
            continue;
        }

        // Check output filter
        if let Some(filter) = output_filter
            && let Some(info) = output_state.info(&output_data.output)
            && let Some(name) = &info.name
            && name != filter
            && filter != "all"
        {
            continue;
        }

        output_data.overlay_manager = None;
        log::info!("Cleared overlay from output");
    }

    Ok(())
}

/// Apply overlay effect to frame data
/// Strategy: Use CPU overlays for videos (faster due to no GPU roundtrip)
///           Use GPU overlays for static content where possible
#[cfg(feature = "gpu")]
pub(super) fn apply_overlay_to_frame(
    output_data: &mut OutputData,
    frame_data: &mut [u8],
    width: u32,
    height: u32,
) -> Result<()> {
    if let Some(overlay_mgr) = &mut output_data.overlay_manager {
        log::debug!(
            "Applying overlay '{}' to video frame {}x{} (buffer size: {} bytes)",
            overlay_mgr.overlay().name(),
            width,
            height,
            frame_data.len()
        );
        // For now, always use CPU overlay to avoid frame drops
        // TODO: Re-enable GPU overlays when we have full GPU pipeline
        overlay_mgr.apply_overlay(frame_data, width, height)?;
        log::debug!("Overlay applied successfully");
    } else {
        log::trace!("No overlay manager present for this output");
    }
    Ok(())
}

/// Apply overlay effect to frame data using CPU only
#[cfg(not(feature = "gpu"))]
pub(super) fn apply_overlay_to_frame(
    output_data: &mut OutputData,
    frame_data: &mut Vec<u8>,
    width: u32,
    height: u32,
) -> Result<()> {
    if let Some(overlay_mgr) = &mut output_data.overlay_manager {
        log::debug!(
            "Applying overlay '{}' (CPU-only) to frame {}x{} (buffer size: {} bytes)",
            overlay_mgr.overlay().name(),
            width,
            height,
            frame_data.len()
        );
        overlay_mgr.apply_overlay(frame_data, width, height)?;
        log::debug!("Overlay applied successfully (CPU-only)");
    } else {
        log::trace!("No overlay manager present for this output (CPU-only)");
    }

    Ok(())
}
