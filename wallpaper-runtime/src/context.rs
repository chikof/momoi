//! Per-output wallpaper execution context.

use audio_engine::AudioSource;
use overlay_system::OverlayCompositor;
use render_core::{DynRenderer, FrameStats, OutputInfo, ShaderUniforms};
use std::sync::Arc;

/// Everything needed to drive one wallpaper on one output.
pub struct WallpaperContext {
    /// The output this context serves.
    pub output: OutputInfo,
    /// Active renderer (GPU or CPU).
    pub renderer: DynRenderer,
    /// Audio source (`PipeWire` or silent).
    pub audio: Arc<dyn AudioSource>,
    /// Overlay widgets for this output.
    pub overlays: OverlayCompositor,
    /// Accumulated frame statistics.
    pub stats: FrameStats,
    /// Current uniform values.
    pub uniforms: ShaderUniforms,
}

impl WallpaperContext {
    /// Construct a context for the given output.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn new(output: OutputInfo, renderer: DynRenderer, audio: Arc<dyn AudioSource>) -> Self {
        let uniforms = ShaderUniforms {
            resolution: [output.width as f32, output.height as f32],
            ..Default::default()
        };
        Self {
            output,
            renderer,
            audio,
            overlays: OverlayCompositor::default(),
            stats: FrameStats::default(),
            uniforms,
        }
    }
}
