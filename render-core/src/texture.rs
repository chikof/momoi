//! Texture types and pixel formats.

use serde::{Deserialize, Serialize};

/// Supported pixel formats for surfaces and textures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PixelFormat {
    /// 8-bit RGBA, most common.
    Rgba8Unorm,
    /// 8-bit BGRA (preferred by many Wayland compositors).
    Bgra8Unorm,
    /// HDR: 16-bit float per channel.
    Rgba16Float,
}

/// Descriptor for creating a new texture.
#[derive(Debug, Clone)]
pub struct TextureDescriptor {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixel format.
    pub format: PixelFormat,
    /// Human-readable label for debugging.
    pub label: Option<String>,
}

/// An opaque handle to a GPU or CPU texture resource.
/// Backends provide their own concrete implementations.
pub trait Texture: Send + Sync + std::fmt::Debug {
    /// Width in pixels.
    fn width(&self) -> u32;
    /// Height in pixels.
    fn height(&self) -> u32;
    /// Pixel format.
    fn format(&self) -> PixelFormat;
}
