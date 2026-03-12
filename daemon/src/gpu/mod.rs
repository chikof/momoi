/// GPU-accelerated rendering module using wgpu
///
/// This module provides GPU rendering capabilities as an alternative to
/// CPU-based shared memory rendering. It offers significant performance
/// improvements for:
/// - Image scaling and compositing
/// - Video frame rendering
/// - Shader effects
/// - Transitions
///
/// Architecture:
/// - `context`: wgpu device/queue management
/// - `renderer`: High-level rendering interface
/// - `render_ops`: Specialized rendering operations (shaders, images, blending)
/// - `pipeline_builder`: Render pipeline creation utilities
/// - `texture`: Texture upload and management
/// - `video_buffer_pool`: Async GPU readback for video frames
pub mod context;
mod pipeline_builder;
pub mod render_ops;
pub mod renderer;
pub mod texture;
mod video_buffer_pool;

pub use context::GpuContext;
pub use renderer::GpuRenderer;
pub use texture::GpuTexture;
pub use video_buffer_pool::VideoBufferPool;

/// Check if GPU rendering is available on this system
pub fn is_available() -> bool {
    // Try to create a wgpu instance
    wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    // If we got here, wgpu is available
    true
}

/// GPU rendering capabilities
#[derive(Debug, Clone)]
pub struct GpuCapabilities {
    pub adapter_name: String,
    pub backend: String,
    pub max_texture_size: u32,
    pub supports_compute: bool,
}

impl GpuCapabilities {
    pub fn log_info(&self) {
        info!("GPU Capabilities:");
        info!("  Adapter: {}", self.adapter_name);
        info!("  Backend: {}", self.backend);
        info!(
            "  Max Texture Size: {}x{}",
            self.max_texture_size, self.max_texture_size
        );
        info!(
            "  Compute Shaders: {}",
            if self.supports_compute { "Yes" } else { "No" }
        );
    }
}
