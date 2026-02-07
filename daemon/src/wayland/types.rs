//! Core types and data structures for the Wayland daemon.
//!
//! This module defines the main daemon state (`WallpaperDaemon`) and per-output
//! data structures (`OutputData`) used throughout the Wayland integration.

use anyhow::Result;
use smithay_client_toolkit::{
    compositor::CompositorState, output::OutputState, registry::RegistryState,
    shell::wlr_layer::LayerShell, shm::Shm,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use wayland_client::{QueueHandle, protocol::wl_output};

use crate::DaemonState;
use crate::wallpaper_manager::WallpaperManager;

/// Main Wayland daemon state.
///
/// Holds all Wayland protocol objects, output data, and shared state for the daemon.
pub struct WallpaperDaemon {
    pub(super) registry_state: RegistryState,
    pub(super) compositor_state: CompositorState,
    pub(super) layer_shell: LayerShell,
    pub(super) output_state: OutputState,
    pub(super) shm: Shm,
    pub(super) outputs: Vec<OutputData>,
    pub(super) wallpaper_manager: WallpaperManager,
    pub(super) state: Arc<Mutex<DaemonState>>,
    pub(super) exit: bool,
    pub(super) resource_monitor: crate::resource_monitor::ResourceMonitor,
    /// Shared GPU renderer (if available and enabled)
    #[cfg(feature = "gpu")]
    pub(super) gpu_renderer: Option<std::sync::Arc<crate::gpu::GpuRenderer>>,
    /// Shared VideoManagers keyed by video file path
    /// Each video is decoded once and GPU-scaled to each output's resolution
    #[cfg(feature = "video")]
    pub(super) video_managers:
        std::collections::HashMap<String, Arc<Mutex<crate::video::VideoManager>>>,
}

/// Per-output data and state.
///
/// Each Wayland output (monitor) has its own OutputData instance containing:
/// - Layer surface for rendering
/// - Buffer management (current + pool of released buffers)
/// - Wallpaper managers (video/shader/overlay)
/// - Transition state
/// - GPU renderer reference
pub struct OutputData {
    pub(super) output: wl_output::WlOutput,
    pub(super) layer_surface: Option<smithay_client_toolkit::shell::wlr_layer::LayerSurface>,
    pub(super) buffer: Option<crate::buffer::ShmBuffer>,
    /// Pool of old buffers waiting to be released by compositor
    pub(super) buffer_pool: Vec<crate::buffer::ShmBuffer>,
    pub(super) width: u32,
    pub(super) height: u32,
    #[allow(dead_code)] // Scale field for future HiDPI support
    pub(super) scale: f64,
    pub(super) configured: bool,
    /// DEPRECATED: Use video_path instead (GPU scaling allows single VideoManager per video)
    #[deprecated]
    #[allow(dead_code)] // Deprecated field kept during migration period
    pub(super) video_manager: Option<crate::video::VideoManager>,
    /// Path to video file (references shared VideoManager in WallpaperDaemon::video_managers)
    #[cfg(feature = "video")]
    pub(super) video_path: Option<String>,
    pub(super) shader_manager: Option<crate::shader_manager::ShaderManager>,
    pub(super) overlay_manager: Option<crate::overlay_shader::OverlayManager>,
    /// Active transition (if any)
    pub(super) transition: Option<crate::transition::Transition>,
    /// Pending new wallpaper content (used during transitions)
    pub(super) pending_wallpaper_data: Option<Vec<u8>>,
    /// GPU renderer for accelerated rendering (optional)
    #[cfg(feature = "gpu")]
    pub(super) gpu_renderer: Option<std::sync::Arc<crate::gpu::GpuRenderer>>,
}

impl Drop for OutputData {
    fn drop(&mut self) {
        log::info!(
            "OutputData::drop - Cleaning up output ({}x{}, buffer_pool: {}, shader: {})",
            self.width,
            self.height,
            self.buffer_pool.len(),
            self.shader_manager.is_some()
        );

        // Clear managers
        self.shader_manager = None;
        self.overlay_manager = None;

        log::info!("OutputData::drop - Cleanup complete");
    }
}

impl OutputData {
    /// Get a buffer from the pool if available and released, or create a new one
    #[allow(dead_code)] // Used by buffer pooling system
    pub(super) fn get_buffer(
        &mut self,
        shm: &Shm,
        width: u32,
        height: u32,
        qh: &QueueHandle<WallpaperDaemon>,
    ) -> Result<crate::buffer::ShmBuffer> {
        // Try to find a released buffer with matching dimensions
        if let Some(index) = self
            .buffer_pool
            .iter()
            .position(|buf| buf.width() == width && buf.height() == height && buf.is_released())
        {
            let buffer = self.buffer_pool.swap_remove(index);
            log::debug!(
                "Reusing buffer from pool ({}x{}, pool size: {})",
                width,
                height,
                self.buffer_pool.len()
            );
            return Ok(buffer);
        }

        // No suitable buffer found, create a new one
        log::debug!("Creating new buffer ({}x{})", width, height);
        crate::buffer::ShmBuffer::new(shm.wl_shm(), width, height, qh)
    }

    /// Move the current buffer to the pool before replacing it
    pub(super) fn swap_buffer(&mut self, new_buffer: crate::buffer::ShmBuffer) {
        if let Some(old_buffer) = self.buffer.take() {
            // Mark the buffer as busy (compositor is still using it)
            // Don't mark as busy here - it's already marked when we called attach()
            self.buffer_pool.push(old_buffer);
            log::debug!(
                "Moved old buffer to pool (pool size: {})",
                self.buffer_pool.len()
            );
        }
        self.buffer = Some(new_buffer);
    }

    /// Clean up released buffers from the pool
    /// Keep at most MAX_POOL_SIZE buffers to avoid unbounded memory growth
    pub(super) fn cleanup_buffer_pool(&mut self) {
        const MAX_POOL_SIZE: usize = 3;

        let initial_size = self.buffer_pool.len();

        if initial_size > MAX_POOL_SIZE {
            // Sort: released buffers first (they can be removed safely)
            // Then remove excess buffers starting with released ones
            let mut to_remove = initial_size - MAX_POOL_SIZE;

            self.buffer_pool.retain(|buf| {
                if to_remove > 0 && buf.is_released() {
                    to_remove -= 1;
                    false // Remove this buffer
                } else {
                    true // Keep this buffer
                }
            });

            let removed = initial_size - self.buffer_pool.len();
            if removed > 0 {
                log::debug!(
                    "Cleaned up {} released buffer(s) from pool (pool size: {} -> {})",
                    removed,
                    initial_size,
                    self.buffer_pool.len()
                );
            }

            // Warn if we still have too many (means they're all busy, possible leak)
            if self.buffer_pool.len() > MAX_POOL_SIZE {
                log::warn!(
                    "Buffer pool has {} busy buffers (max: {}), compositor may not be releasing buffers!",
                    self.buffer_pool.len(),
                    MAX_POOL_SIZE
                );
            }
        }
    }
}

/// Frame data ready for rendering (computed in parallel)
pub struct FrameUpdate {
    pub(super) output_index: usize,
    pub(super) argb_data: Vec<u8>,
    pub(super) width: u32,
    pub(super) height: u32,
}
