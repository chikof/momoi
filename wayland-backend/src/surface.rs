//! `zwlr_layer_surface_v1` wrapper for wallpaper surfaces.

use render_core::OutputInfo;
use tracing::info;

/// A layer-shell surface bound to a specific Wayland output.
///
/// Positioned at the `background` layer so it sits behind all other windows.
///
/// # Notes
/// The actual protocol objects are managed by `smithay-client-toolkit`.
/// This struct is intentionally a thin handle; most protocol work happens
/// in the Wayland event loop that the daemon drives.
#[derive(Debug)]
pub struct LayerSurface {
    /// The output this surface covers.
    pub output: OutputInfo,
    /// Whether the surface has received its first configure event.
    pub configured: bool,
    /// Current surface width (may differ from `output.width` during resize).
    pub width: u32,
    /// Current surface height.
    pub height: u32,
}

impl LayerSurface {
    /// Create a handle for the given output.
    /// The caller is responsible for creating the `wlr_layer_surface` protocol object.
    #[must_use]
    pub fn new(output: OutputInfo) -> Self {
        let (width, height) = (output.width, output.height);
        info!(output = %output.name, width, height, "layer surface created");
        Self {
            output,
            configured: false,
            width,
            height,
        }
    }

    /// Called when the compositor sends a `configure` event.
    pub fn configure(&mut self, width: u32, height: u32) {
        self.width = if width == 0 { self.output.width } else { width };
        self.height = if height == 0 {
            self.output.height
        } else {
            height
        };
        self.configured = true;
    }
}
