//! CPU software renderer — animated gradient fallback.
//!
//! Renders every pixel on the CPU each frame. Intentionally simple: the goal
//! is a working fallback, not a high-quality effect.
//!
//! The gradient is HSV-based and responds to:
//! - `uniforms.time`           — drives slow colour cycling
//! - `uniforms.audio_bands[0]` — bass energy brightens the image

use render_core::{Frame, FrameStats, RenderError, Renderer, ShaderUniforms, SurfaceDescriptor};
use std::time::Instant;
use tracing::info;

/// Software renderer producing RGBA pixels on the CPU.
pub struct CpuRenderer {
    width: u32,
    height: u32,
    /// Pre-allocated pixel buffer reused every frame (avoids allocation hot-path).
    buffer: Vec<u8>,
}

impl Default for CpuRenderer {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            buffer: Vec::new(),
        }
    }
}

impl Renderer for CpuRenderer {
    fn init(&mut self, surface: &SurfaceDescriptor) -> Result<(), RenderError> {
        self.width = surface.output.width;
        self.height = surface.output.height;
        self.buffer = vec![0u8; (self.width * self.height * 4) as usize];
        info!(
            width = self.width,
            height = self.height,
            "CPU renderer initialised"
        );
        Ok(())
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn render_frame(
        &mut self,
        uniforms: &ShaderUniforms,
        stats: &mut FrameStats,
    ) -> Result<Frame, RenderError> {
        let start = Instant::now();
        let t = uniforms.time;
        // audio_bands is [f32; 32] — index 0 is the lowest frequency band (bass).
        let bass = uniforms.audio_bands[0];

        for y in 0..self.height {
            let ny = y as f32 / self.height as f32;
            for x in 0..self.width {
                let nx = x as f32 / self.width as f32;

                // Animated HSV gradient reacting to time and bass.
                let hue = (nx + t * 0.05 + bass * 0.3).fract();
                let sat = 0.7_f32 + ny * 0.3;
                let val = 0.5_f32 + 0.5 * (t * 0.8 + ny * std::f32::consts::PI).sin().abs();

                let (r, g, b) = hsv_to_rgb(hue, sat, val);
                let idx = ((y * self.width + x) * 4) as usize;
                self.buffer[idx] = (r * 255.0) as u8;
                self.buffer[idx + 1] = (g * 255.0) as u8;
                self.buffer[idx + 2] = (b * 255.0) as u8;
                self.buffer[idx + 3] = 255;
            }
        }

        stats.cpu_time = start.elapsed();

        Ok(Frame {
            data: self.buffer.clone(),
            width: self.width,
            height: self.height,
            timestamp: start,
        })
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), RenderError> {
        self.width = width;
        self.height = height;
        self.buffer = vec![0u8; (width * height * 4) as usize];
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "cpu/software"
    }
}

/// Convert HSV → RGB, all values in `0.0..=1.0`.
#[allow(
    clippy::many_single_char_names,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let i = (h * 6.0).floor() as u32;
    let f = h * 6.0 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use render_core::PixelFormat;

    #[test]
    fn init_sets_correct_buffer_size() {
        let mut r = CpuRenderer::default();
        let surface = SurfaceDescriptor {
            output: render_core::OutputInfo {
                name: "test".into(),
                width: 8,
                height: 4,
                refresh_mhz: 60_000,
                scale: 1.0,
            },
            format: PixelFormat::Rgba8Unorm,
        };
        r.init(&surface).unwrap();
        assert_eq!(r.buffer.len(), 8 * 4 * 4);
    }

    #[test]
    fn render_frame_fills_buffer() {
        let mut r = CpuRenderer::default();
        let surface = SurfaceDescriptor {
            output: render_core::OutputInfo {
                name: "test".into(),
                width: 4,
                height: 4,
                refresh_mhz: 60_000,
                scale: 1.0,
            },
            format: PixelFormat::Rgba8Unorm,
        };
        r.init(&surface).unwrap();
        let mut stats = FrameStats::default();
        let frame = r
            .render_frame(&ShaderUniforms::default(), &mut stats)
            .unwrap();
        assert_eq!(frame.data.len(), 4 * 4 * 4);
        // Alpha channel should always be 255.
        assert!(frame.data.iter().skip(3).step_by(4).all(|&a| a == 255));
    }

    #[test]
    fn hsv_pure_red_returns_correct_rgb() {
        let (r, g, b) = hsv_to_rgb(0.0, 1.0, 1.0);
        assert!((r - 1.0).abs() < 1e-5);
        assert!(g.abs() < 1e-5);
        assert!(b.abs() < 1e-5);
    }
}
