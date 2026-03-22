//! Surface and output descriptors.

use serde::{Deserialize, Serialize};

/// Physical display output information supplied by the Wayland compositor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputInfo {
    /// Compositor-assigned name (e.g. `DP-1`, `HDMI-A-1`).
    pub name: String,
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
    /// Display refresh rate in mHz (e.g. `60000` = 60 Hz).
    pub refresh_mhz: u32,
    /// Display scale factor (`HiDPI`).
    pub scale: f64,
}

/// Parameters used when creating a rendering surface.
#[derive(Debug, Clone)]
pub struct SurfaceDescriptor {
    /// The output this surface belongs to.
    pub output: OutputInfo,
    /// Pixel format to use for the surface buffer.
    pub format: crate::PixelFormat,
}
