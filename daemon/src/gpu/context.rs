/// GPU context management - handles wgpu device/queue initialization
use anyhow::{Context, Result};
use wgpu;

/// GPU context containing device, queue, and adapter info
pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_info: wgpu::AdapterInfo,
    pub limits: wgpu::Limits,
}

impl GpuContext {
    /// Create a new GPU context
    ///
    /// This initializes wgpu with the best available adapter (GPU).
    /// On Linux, this will typically use Vulkan.
    pub async fn new() -> Result<Self> {
        log::info!("Initializing GPU context...");

        // Create wgpu instance
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // Request adapter (GPU)
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .context("Failed to find suitable GPU adapter")?;

        let adapter_info = adapter.get_info();
        log::info!(
            "Selected GPU adapter: {} ({:?})",
            adapter_info.name,
            adapter_info.backend
        );

        // Request device and queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Wallpaper Daemon GPU Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .context("Failed to create GPU device")?;

        let limits = device.limits();

        log::info!("GPU context initialized successfully");
        log::info!("  Backend: {:?}", adapter_info.backend);
        log::info!(
            "  Max Texture Size: {}x{}",
            limits.max_texture_dimension_2d,
            limits.max_texture_dimension_2d
        );

        Ok(Self {
            device,
            queue,
            adapter_info,
            limits,
        })
    }

    /// Get GPU capabilities for reporting
    pub fn capabilities(&self) -> crate::gpu::GpuCapabilities {
        crate::gpu::GpuCapabilities {
            adapter_name: self.adapter_info.name.clone(),
            backend: format!("{:?}", self.adapter_info.backend),
            max_texture_size: self.limits.max_texture_dimension_2d,
            supports_compute: self
                .device
                .features()
                .contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES),
        }
    }
}

impl std::fmt::Debug for GpuContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuContext")
            .field("adapter", &self.adapter_info.name)
            .field("backend", &self.adapter_info.backend)
            .finish()
    }
}
