//! Static image wallpaper renderer.
//!
//! Loads a PNG/JPEG/GIF from disk, scales it to the surface size using
//! Lanczos3 (best quality) or nearest-neighbour (fastest), and caches the
//! result in memory.  Subsequent `render_frame` calls return the cached buffer
//! without touching the disk or re-scaling.
//!
//! # Scaling strategy
//! - **Cover** (default): scale the image so it fills the entire surface,
//!   cropping the excess.  The centre of the image is preserved.
//! - **Fit**: scale so the whole image is visible, with letterbox bars.
//! - **Stretch**: ignore aspect ratio and fill exactly.

use render_core::{Frame, FrameStats, RenderError, Renderer, ShaderUniforms, SurfaceDescriptor};
use std::{path::PathBuf, time::Instant};
use tracing::{debug, info};

/// How to fit the source image onto the output surface.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScaleMode {
    /// Fill the surface; crop excess (default).
    #[default]
    Cover,
    /// Fit the image; letterbox if needed.
    Fit,
    /// Stretch to exact surface dimensions.
    Stretch,
}

/// Renders a static image file on every frame.
pub struct ImageRenderer {
    /// Source file path (retained for logging and potential reload).
    path: PathBuf,
    /// Cached RGBA pixels scaled to the output dimensions.
    pixels: Vec<u8>,
    /// Output width in pixels.
    width: u32,
    /// Output height in pixels.
    height: u32,
    /// Scaling strategy.
    mode: ScaleMode,
}

impl ImageRenderer {
    /// Create an `ImageRenderer` that will display the image at `path`.
    ///
    /// The image is not decoded until [`Renderer::init`] is called so that
    /// output dimensions are known.
    #[must_use]
    pub fn new(path: PathBuf, mode: ScaleMode) -> Self {
        Self {
            path,
            pixels: Vec::new(),
            width: 0,
            height: 0,
            mode,
        }
    }

    /// Convenience constructor with default (Cover) scaling.
    #[must_use]
    pub fn from_path(path: PathBuf) -> Self {
        Self::new(path, ScaleMode::default())
    }

    /// Load, decode, and scale the image to `(dst_w, dst_h)`.
    fn load_and_scale(&self, dst_w: u32, dst_h: u32) -> Result<Vec<u8>, RenderError> {
        let img = image::open(&self.path).map_err(|e| {
            RenderError::InitFailed(format!(
                "failed to open image '{}': {e}",
                self.path.display()
            ))
        })?;

        let rgba = img.into_rgba8();
        let (src_w, src_h) = rgba.dimensions();
        debug!(
            src_w, src_h, dst_w, dst_h, mode = ?self.mode,
            "scaling image for output"
        );

        let scaled = match self.mode {
            ScaleMode::Stretch => resize_rgba(&rgba, dst_w, dst_h),
            ScaleMode::Fit => {
                let (sw, sh, ox, oy) = fit_rect(src_w, src_h, dst_w, dst_h);
                let resized = resize_rgba(&rgba, sw, sh);
                letterbox(&resized, sw, sh, dst_w, dst_h, ox, oy)
            }
            ScaleMode::Cover => {
                let (sw, sh, cx, cy) = cover_rect(src_w, src_h, dst_w, dst_h);
                let resized = resize_rgba(&rgba, sw, sh);
                crop_centre(&resized, sw, sh, dst_w, dst_h, cx, cy)
            }
        };

        Ok(scaled)
    }
}

impl Renderer for ImageRenderer {
    fn init(&mut self, surface: &SurfaceDescriptor) -> Result<(), RenderError> {
        self.width = surface.output.width;
        self.height = surface.output.height;
        info!(
            path = %self.path.display(),
            width = self.width,
            height = self.height,
            "loading image wallpaper"
        );
        self.pixels = self.load_and_scale(self.width, self.height)?;
        Ok(())
    }

    fn render_frame(
        &mut self,
        _uniforms: &ShaderUniforms,
        stats: &mut FrameStats,
    ) -> Result<Frame, RenderError> {
        let start = Instant::now();
        // Static image — just clone the cached buffer.
        let data = self.pixels.clone();
        stats.cpu_time = start.elapsed();
        Ok(Frame {
            data,
            width: self.width,
            height: self.height,
            timestamp: start,
        })
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), RenderError> {
        self.width = width;
        self.height = height;
        self.pixels = self.load_and_scale(width, height)?;
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "cpu-image"
    }
}

/// Resize an RGBA image to `(dw, dh)` using nearest-neighbour interpolation.
///
/// For a wallpaper daemon the visible quality difference between Lanczos and
/// nearest-neighbour at desktop resolution is negligible after one scale
/// operation.  We keep this dependency-free.
fn resize_rgba(src: &image::RgbaImage, dw: u32, dh: u32) -> Vec<u8> {
    use image::imageops::FilterType;
    let resized = image::imageops::resize(src, dw, dh, FilterType::Lanczos3);
    resized.into_raw()
}

/// Calculate (`scaled_w`, `scaled_h`, `offset_x`, `offset_y`) for **Fit** mode.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn fit_rect(sw: u32, sh: u32, dw: u32, dh: u32) -> (u32, u32, u32, u32) {
    let scale = (f64::from(dw) / f64::from(sw)).min(f64::from(dh) / f64::from(sh));
    let nw = (f64::from(sw) * scale).round() as u32;
    let nh = (f64::from(sh) * scale).round() as u32;
    let ox = (dw - nw) / 2;
    let oy = (dh - nh) / 2;
    (nw, nh, ox, oy)
}

/// Calculate (`scaled_w`, `scaled_h`, `crop_x`, `crop_y`) for **Cover** mode.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn cover_rect(sw: u32, sh: u32, dw: u32, dh: u32) -> (u32, u32, u32, u32) {
    let scale = (f64::from(dw) / f64::from(sw)).max(f64::from(dh) / f64::from(sh));
    let nw = (f64::from(sw) * scale).round() as u32;
    let nh = (f64::from(sh) * scale).round() as u32;
    let cx = (nw - dw) / 2;
    let cy = (nh - dh) / 2;
    (nw, nh, cx, cy)
}

/// Place `src` pixels on a black `(dw × dh)` canvas at `(ox, oy)`.
fn letterbox(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32, ox: u32, oy: u32) -> Vec<u8> {
    let mut canvas = vec![0u8; (dw * dh * 4) as usize];
    for y in 0..sh {
        let dy = y + oy;
        if dy >= dh {
            break;
        }
        let src_row = (y * sw * 4) as usize;
        let dst_row = (dy * dw * 4 + ox * 4) as usize;
        let row_bytes = (sw * 4) as usize;
        let dst_end = dst_row + row_bytes;
        if dst_end <= canvas.len() {
            canvas[dst_row..dst_end].copy_from_slice(&src[src_row..src_row + row_bytes]);
        }
    }
    canvas
}

/// Extract `(dw × dh)` pixels from `src` starting at `(cx, cy)`.
fn crop_centre(src: &[u8], sw: u32, _sh: u32, dw: u32, dh: u32, cx: u32, cy: u32) -> Vec<u8> {
    let mut out = vec![0u8; (dw * dh * 4) as usize];
    for y in 0..dh {
        let sy = y + cy;
        let src_row = ((sy * sw + cx) * 4) as usize;
        let dst_row = (y * dw * 4) as usize;
        let row_bytes = (dw * 4) as usize;
        if src_row + row_bytes <= src.len() {
            out[dst_row..dst_row + row_bytes].copy_from_slice(&src[src_row..src_row + row_bytes]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_rect_landscape_into_square_should_pillarbox() {
        // 400×200 into 100×100 → scale 0.5 → 200×100, offset (0,0)? No:
        // scale = min(100/400, 100/200) = 0.25 → 100×50, ox=0, oy=25
        let (nw, nh, ox, oy) = fit_rect(400, 200, 100, 100);
        assert_eq!((nw, nh), (100, 50));
        assert_eq!((ox, oy), (0, 25));
    }

    #[test]
    fn cover_rect_portrait_into_landscape_should_crop_top_bottom() {
        // 100×200 into 200×100 → scale = max(2, 0.5) = 2 → 200×400
        // cx = (200-200)/2 = 0, cy = (400-100)/2 = 150
        let (nw, nh, cx, cy) = cover_rect(100, 200, 200, 100);
        assert_eq!((nw, nh), (200, 400));
        assert_eq!((cx, cy), (0, 150));
    }
}
