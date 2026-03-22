//! Core overlay widget trait and geometry types.

use crate::OverlayError;

/// Rectangular region on the output surface.
#[derive(Debug, Clone, Copy, Default)]
pub struct WidgetRect {
    /// Left edge in pixels.
    pub x: u32,
    /// Top edge in pixels.
    pub y: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// Every overlay element must implement this trait.
pub trait OverlayWidget: Send + Sync {
    /// Refresh internal state. Called once per frame before [`render`](Self::render).
    fn update(&mut self);

    /// Produce RGBA pixel data for the widget's bounding box.
    ///
    /// `width` and `height` match the dimensions returned by [`bounds`](Self::bounds).
    ///
    /// # Errors
    /// Returns [`OverlayError::Render`] on rasterisation failure.
    fn render(&self, width: u32, height: u32) -> Result<Vec<u8>, OverlayError>;

    /// Current bounding box in surface-pixel coordinates.
    fn bounds(&self) -> WidgetRect;

    /// Human-readable widget name (used for logging).
    fn name(&self) -> &'static str;
}
