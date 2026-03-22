//! Composites all active overlay widgets onto a frame buffer.

use crate::{OverlayError, OverlayWidget};
use tracing::debug;

/// Manages a collection of widgets and alpha-blends them onto each frame.
#[derive(Default)]
pub struct OverlayCompositor {
    widgets: Vec<Box<dyn OverlayWidget>>,
}

impl OverlayCompositor {
    /// Add a widget to the compositor.
    pub fn add<W: OverlayWidget + 'static>(&mut self, widget: W) {
        debug!(widget = widget.name(), "overlay widget added");
        self.widgets.push(Box::new(widget));
    }

    /// Refresh all widgets (call once per frame before [`composite`](Self::composite)).
    pub fn update_all(&mut self) {
        for w in &mut self.widgets {
            w.update();
        }
    }

    /// Alpha-blend all widgets onto `frame_data` (RGBA row-major pixels).
    ///
    /// # Errors
    /// Returns the first [`OverlayError`] encountered.
    pub fn composite(
        &self,
        frame_data: &mut [u8],
        frame_width: u32,
        frame_height: u32,
    ) -> Result<(), OverlayError> {
        for widget in &self.widgets {
            let bounds = widget.bounds();
            let pixels = widget.render(bounds.width, bounds.height)?;

            for wy in 0..bounds.height {
                let fy = bounds.y + wy;
                if fy >= frame_height {
                    break;
                }
                for wx in 0..bounds.width {
                    let fx = bounds.x + wx;
                    if fx >= frame_width {
                        continue;
                    }
                    let wi = ((wy * bounds.width + wx) * 4) as usize;
                    let fi = ((fy * frame_width + fx) * 4) as usize;
                    if wi + 3 >= pixels.len() || fi + 3 >= frame_data.len() {
                        continue;
                    }
                    let alpha = f32::from(pixels[wi + 3]) / 255.0;
                    for ch in 0..3usize {
                        let bg = f32::from(frame_data[fi + ch]);
                        let fg = f32::from(pixels[wi + ch]);
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        {
                            frame_data[fi + ch] = (bg.mul_add(1.0 - alpha, fg * alpha)) as u8;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
