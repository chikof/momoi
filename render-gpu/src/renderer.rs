//! GPU renderer implementation using `wgpu` 23+.

use crate::uniform_buffer::UniformBuffer;
use render_core::{Frame, FrameStats, RenderError, Renderer, ShaderUniforms, SurfaceDescriptor};
use shader_engine::ShaderRegistry;
use std::time::Instant;
use tracing::{debug, info, warn};

/// `wgpu`-backed renderer — one instance per Wayland output surface.
pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_texture: Option<wgpu::Texture>,
    readback_buffer: Option<wgpu::Buffer>,
    pipeline: Option<wgpu::RenderPipeline>,
    uniform_layout: Option<wgpu::BindGroupLayout>,
    uniforms: Option<UniformBuffer>,
    width: u32,
    height: u32,
    registry: ShaderRegistry,
    active_shader: Option<String>,
}

impl GpuRenderer {
    /// Create a GPU renderer backed by the given shader registry.
    ///
    /// # Errors
    /// Returns [`RenderError::InitFailed`] if no suitable wgpu adapter is found.
    pub async fn new(registry: ShaderRegistry) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::default();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| RenderError::InitFailed(e.to_string()))?;

        info!(adapter = adapter.get_info().name, "GPU adapter selected");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|e: wgpu::RequestDeviceError| RenderError::InitFailed(e.to_string()))?;

        Ok(Self {
            device,
            queue,
            render_texture: None,
            readback_buffer: None,
            pipeline: None,
            uniform_layout: None,
            uniforms: None,
            width: 0,
            height: 0,
            registry,
            active_shader: None,
        })
    }

    /// Activate a registered shader by name, rebuilding the pipeline.
    ///
    /// # Errors
    /// Returns [`RenderError::Shader`] if the name is not registered.
    pub fn set_shader(&mut self, name: &str) -> Result<(), RenderError> {
        let entry = self
            .registry
            .get_by_name(name)
            .ok_or_else(|| RenderError::Shader {
                name: name.into(),
                detail: "not found in registry".into(),
            })?;

        let wgpu_module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(name),
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(&entry.source)),
            });

        let uniform_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("uniforms_bgl"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("layout"),
                bind_group_layouts: &[Some(&uniform_layout)],
                immediate_size: 0,
            });

        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("shader_pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &wgpu_module,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &wgpu_module,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });

        let ub = UniformBuffer::new(&self.device, &uniform_layout);
        self.pipeline = Some(pipeline);
        self.uniform_layout = Some(uniform_layout);
        self.uniforms = Some(ub);
        self.active_shader = Some(name.into());
        debug!(shader = name, "GPU pipeline rebuilt");
        Ok(())
    }

    fn create_render_targets(&mut self) {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render_target"),
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        // Rows must be padded to a 256-byte boundary for wgpu buffer copies.
        let bytes_per_row = (self.width * 4).next_multiple_of(256);
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(bytes_per_row * self.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        self.render_texture = Some(texture);
        self.readback_buffer = Some(readback);
    }
}

impl Renderer for GpuRenderer {
    fn init(&mut self, surface: &SurfaceDescriptor) -> Result<(), RenderError> {
        self.width = surface.output.width;
        self.height = surface.output.height;
        self.create_render_targets();
        info!(
            width = self.width,
            height = self.height,
            "GPU renderer initialised"
        );
        Ok(())
    }

    fn render_frame(
        &mut self,
        uniforms: &ShaderUniforms,
        stats: &mut FrameStats,
    ) -> Result<Frame, RenderError> {
        let start = Instant::now();

        let texture = self
            .render_texture
            .as_ref()
            .ok_or_else(|| RenderError::InitFailed("renderer not initialised".into()))?;

        let (Some(pipeline), Some(ub)) = (&self.pipeline, &self.uniforms) else {
            warn!("no shader active, returning black frame");
            return Ok(Frame {
                data: vec![0u8; (self.width * self.height * 4) as usize],
                width: self.width,
                height: self.height,
                timestamp: start,
            });
        };

        ub.update(&self.queue, uniforms);

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame_encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &ub.bind_group, &[]);
            pass.draw(0..3, 0..1); // Full-screen triangle — no vertex buffer needed.
        }

        let readback = self.readback_buffer.as_ref().unwrap();
        let bytes_per_row = (self.width * 4).next_multiple_of(256);

        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit([encoder.finish()]);

        // Synchronous pixel readback - acceptable at wallpaper cadence (≤120 fps).
        let (tx, rx) = std::sync::mpsc::channel();
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });

        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        rx.recv()
            .map_err(|_| RenderError::FrameSubmission("readback channel closed".into()))?
            .map_err(|e| RenderError::FrameSubmission(e.to_string()))?;

        // Strip 256-byte row padding added by wgpu.
        let actual_bpr = (self.width * 4) as usize;
        let padded_bpr = bytes_per_row as usize;
        let mut data = Vec::with_capacity(actual_bpr * self.height as usize);
        {
            let mapped = slice.get_mapped_range();
            for row in 0..self.height as usize {
                data.extend_from_slice(&mapped[row * padded_bpr..row * padded_bpr + actual_bpr]);
            }
        }
        readback.unmap();

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
        self.create_render_targets();
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "wgpu"
    }
}
