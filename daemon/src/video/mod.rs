//! Video playback module for wallpaper video support
//!
//! This module provides GPU-accelerated video playback using GStreamer with hardware decoding.
//! It consists of several submodules for maintainability:
//!
//! - `pipeline`: GStreamer pipeline setup and configuration
//! - `frames`: Frame data handling and processing
//! - `stats`: Performance statistics and metrics tracking
//! - `manager`: Main VideoManager that coordinates everything
//!
//! # Architecture
//!
//! Video playback uses hardware-accelerated decoding via VA-API:
//! 1. GStreamer decodes video using `vah264dec` (hardware decode)
//! 2. `vapostproc` does hardware scaling + color conversion to BGRA
//! 3. Frames are delivered to `AppSink` callback
//! 4. GPU renderer scales BGRA to target resolution(s)
//! 5. Async GPU readback provides frames to Wayland compositor
//!
//! # Performance
//!
//! - Hardware decode: Minimal CPU usage
//! - Shared source textures: One GPU upload per frame regardless of output count
//! - Resolution caching: Each unique resolution rendered once
//! - Double buffering: No GPU stalls during readback

#[cfg(feature = "video")]
mod frames;
#[cfg(feature = "video")]
mod manager;
#[cfg(feature = "video")]
mod pipeline;
#[cfg(feature = "video")]
mod stats;

#[cfg(feature = "video")]
pub use manager::VideoManager;

// Re-export for backward compatibility during migration
#[cfg(not(feature = "video"))]
pub use manager_stub::VideoManager;

#[cfg(not(feature = "video"))]
mod manager_stub {
    use anyhow::Result;
    use std::path::Path;
    use std::time::Duration;

    /// Stub VideoManager when video feature is disabled
    pub struct VideoManager;

    impl VideoManager {
        pub fn load(
            _path: impl AsRef<Path>,
            _target_width: u32,
            _target_height: u32,
            _scale_mode: common::ScaleMode,
            _muted: bool,
            _target_fps: u32,
            #[cfg(feature = "gpu")] _gpu_renderer: Option<std::sync::Arc<crate::gpu::GpuRenderer>>,
        ) -> Result<Self> {
            anyhow::bail!("Video support not compiled in")
        }

        pub fn play(&mut self) -> Result<()> {
            Ok(())
        }

        pub fn pause(&mut self) -> Result<()> {
            Ok(())
        }

        pub fn current_frame_data(&self) -> Option<Vec<u8>> {
            None
        }

        pub fn update(&mut self) -> bool {
            false
        }

        pub fn dimensions(&self) -> (u32, u32) {
            (0, 0)
        }

        pub fn is_playing(&self) -> bool {
            false
        }

        pub fn set_loop(&mut self, _should_loop: bool) {}

        pub fn frame_duration(&self) -> Duration {
            Duration::from_millis(16)
        }

        pub fn detected_fps(&self) -> Option<f64> {
            None
        }
    }
}
