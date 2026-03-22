//! Shader uniform data shared across all backends.

use serde::{Deserialize, Serialize};

/// Uniform values injected into every shader each frame.
///
/// WGSL struct layout (must match exactly):
/// ```wgsl
/// struct Uniforms {
///     time:        f32,
///     delta_time:  f32,
///     resolution:  vec2<f32>,
///     mouse:       vec2<f32>,
///     _pad0:       vec2<f32>,
///     audio_bands: array<vec4<f32>, 8>,  // 32 f32s
/// }
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Serialize, Deserialize)]
pub struct ShaderUniforms {
    /// Elapsed time in seconds since the wallpaper started.
    pub time: f32,
    /// Time delta since the last frame in seconds.
    pub delta_time: f32,
    /// Surface resolution `[width, height]` in pixels.
    pub resolution: [f32; 2],
    /// Normalised mouse position `[x, y]` in `0..1`.
    pub mouse: [f32; 2],
    /// Padding to satisfy 16-byte vec4 alignment.
    #[allow(clippy::pedantic)]
    pub _pad0: [f32; 2],
    /// 32 audio frequency magnitudes packed as 8×vec4 (32 f32s).
    /// Layout: bands 0–3 = `[0..4]`, bands 4–7 = `[4..8]`, etc.
    pub audio_bands: [f32; 32],
}

impl Default for ShaderUniforms {
    fn default() -> Self {
        Self {
            time: 0.0,
            delta_time: 0.0,
            resolution: [1920.0, 1080.0],
            mouse: [0.0, 0.0],
            _pad0: [0.0, 0.0],
            audio_bands: [0.0; 32],
        }
    }
}
