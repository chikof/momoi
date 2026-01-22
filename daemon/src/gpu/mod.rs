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
/// - `pipeline`: Render pipeline creation
/// - `texture`: Texture upload and management
pub mod context;
pub mod renderer;
pub mod texture;

pub use context::GpuContext;
pub use renderer::GpuRenderer;
pub use texture::GpuTexture;

/// Check if GPU rendering is available on this system
pub fn is_available() -> bool {
    // Try to create a wgpu instance
    wgpu::Instance::new(wgpu::InstanceDescriptor {
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
        log::info!("GPU Capabilities:");
        log::info!("  Adapter: {}", self.adapter_name);
        log::info!("  Backend: {}", self.backend);
        log::info!(
            "  Max Texture Size: {}x{}",
            self.max_texture_size,
            self.max_texture_size
        );
        log::info!(
            "  Compute Shaders: {}",
            if self.supports_compute { "Yes" } else { "No" }
        );
    }
}
