use crate::gpu::{GpuContext, GpuTexture};
/// High-level GPU renderer for wallpaper content
use anyhow::Result;
use wgpu;
use wgpu::util::DeviceExt;

/// GPU renderer for wallpaper content
pub struct GpuRenderer {
    context: GpuContext,
    /// Render pipeline for simple texture blitting
    blit_pipeline: wgpu::RenderPipeline,
    /// Render pipeline for image scaling
    scale_pipeline: wgpu::RenderPipeline,
    /// Render pipeline for plasma shader
    plasma_pipeline: wgpu::RenderPipeline,
    /// Render pipeline for waves shader
    waves_pipeline: wgpu::RenderPipeline,
    /// Render pipeline for gradient shader
    gradient_pipeline: wgpu::RenderPipeline,
    /// Render pipeline for starfield shader
    starfield_pipeline: wgpu::RenderPipeline,
    /// Render pipeline for matrix shader
    matrix_pipeline: wgpu::RenderPipeline,
    /// Render pipeline for raymarching shader
    raymarching_pipeline: wgpu::RenderPipeline,
    /// Render pipeline for tunnel shader
    tunnel_pipeline: wgpu::RenderPipeline,
    /// Render pipeline for overlay effects
    overlay_pipeline: wgpu::RenderPipeline,
    /// Render pipeline for blending two textures (transitions)
    blend_pipeline: wgpu::RenderPipeline,
    /// Bind group layout for blend shader (2 textures + uniforms)
    blend_bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group layout for textures
    texture_bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group layout for shader uniforms
    shader_uniform_layout: wgpu::BindGroupLayout,
    /// Bind group layout for overlay shader (texture + sampler + uniforms)
    overlay_bind_group_layout: wgpu::BindGroupLayout,
    /// Sampler for texture sampling
    sampler: wgpu::Sampler,
}

impl GpuRenderer {
    /// Create a new GPU renderer
    pub async fn new() -> Result<Self> {
        let context = GpuContext::new().await?;

        // Create bind group layout for textures
        let texture_bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Texture Bind Group Layout"),
                    entries: &[
                        // Texture
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        // Sampler
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

        // Create sampler
        let sampler = context.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Texture Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create simple blit shader for proof-of-concept
        let shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Blit Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/blit.wgsl").into()),
            });

        let pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Blit Pipeline Layout"),
                    bind_group_layouts: &[&texture_bind_group_layout],
                    push_constant_ranges: &[],
                });

        let blit_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Blit Pipeline"),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: "vs_main",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: "fs_main",
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba8UnormSrgb,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 1,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview: None,
                });

        // Create scaling shader pipeline
        let scale_shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Scale Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/scale.wgsl").into()),
            });

        let scale_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Scale Pipeline"),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &scale_shader,
                        entry_point: "vs_main",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &scale_shader,
                        entry_point: "fs_main",
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba8UnormSrgb,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 1,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview: None,
                });

        // Create bind group layout for shader uniforms (time, resolution, etc.)
        let shader_uniform_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Shader Uniform Layout"),
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

        // Create plasma shader pipeline
        let plasma_shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Plasma Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/plasma.wgsl").into()),
            });

        let plasma_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Plasma Pipeline Layout"),
                    bind_group_layouts: &[&shader_uniform_layout],
                    push_constant_ranges: &[],
                });

        let plasma_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Plasma Pipeline"),
                    layout: Some(&plasma_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &plasma_shader,
                        entry_point: "vs_main",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &plasma_shader,
                        entry_point: "fs_main",
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba8UnormSrgb,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 1,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview: None,
                });

        // Create waves shader pipeline
        let waves_shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Waves Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/waves.wgsl").into()),
            });

        let waves_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Waves Pipeline"),
                    layout: Some(&plasma_pipeline_layout), // Reuse same layout
                    vertex: wgpu::VertexState {
                        module: &waves_shader,
                        entry_point: "vs_main",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &waves_shader,
                        entry_point: "fs_main",
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba8UnormSrgb,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 1,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview: None,
                });

        // Create gradient shader pipeline
        let gradient_shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Gradient Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/gradient.wgsl").into()),
            });

        let gradient_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Gradient Pipeline"),
                    layout: Some(&plasma_pipeline_layout), // Reuse same layout
                    vertex: wgpu::VertexState {
                        module: &gradient_shader,
                        entry_point: "vs_main",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &gradient_shader,
                        entry_point: "fs_main",
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba8UnormSrgb,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 1,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview: None,
                });

        // Create starfield shader pipeline
        let starfield_shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Starfield Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/starfield.wgsl").into()),
            });

        let starfield_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Starfield Pipeline"),
                    layout: Some(&plasma_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &starfield_shader,
                        entry_point: "vs_main",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &starfield_shader,
                        entry_point: "fs_main",
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba8UnormSrgb,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 1,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview: None,
                });

        // Create matrix shader pipeline
        let matrix_shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Matrix Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/matrix.wgsl").into()),
            });

        let matrix_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Matrix Pipeline"),
                    layout: Some(&plasma_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &matrix_shader,
                        entry_point: "vs_main",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &matrix_shader,
                        entry_point: "fs_main",
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba8UnormSrgb,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 1,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview: None,
                });

        // Create raymarching shader pipeline
        let raymarching_shader =
            context
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("Raymarching Shader"),
                    source: wgpu::ShaderSource::Wgsl(
                        include_str!("shaders/raymarching.wgsl").into(),
                    ),
                });

        let raymarching_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Raymarching Pipeline"),
                    layout: Some(&plasma_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &raymarching_shader,
                        entry_point: "vs_main",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &raymarching_shader,
                        entry_point: "fs_main",
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba8UnormSrgb,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 1,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview: None,
                });

        // Create tunnel shader pipeline
        let tunnel_shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Tunnel Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/tunnel.wgsl").into()),
            });

        let tunnel_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Tunnel Pipeline"),
                    layout: Some(&plasma_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &tunnel_shader,
                        entry_point: "vs_main",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &tunnel_shader,
                        entry_point: "fs_main",
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba8UnormSrgb,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 1,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview: None,
                });

        // Create overlay bind group layout (texture + sampler + uniforms)
        let overlay_bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Overlay Bind Group Layout"),
                    entries: &[
                        // Base texture
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        // Sampler
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                        // Uniforms
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                    ],
                });

        // Create overlay shader pipeline
        let overlay_shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Overlay Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/overlay.wgsl").into()),
            });

        let overlay_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Overlay Pipeline Layout"),
                    bind_group_layouts: &[&overlay_bind_group_layout],
                    push_constant_ranges: &[],
                });

        let overlay_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Overlay Pipeline"),
                    layout: Some(&overlay_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &overlay_shader,
                        entry_point: "vs_main",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &overlay_shader,
                        entry_point: "fs_main",
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba8UnormSrgb,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 1,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview: None,
                });

        // Create blend bind group layout (for transitions with 2 textures)
        let blend_bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Blend Bind Group Layout"),
                    entries: &[
                        // Uniforms
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        // Old texture
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        // New texture
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        // Sampler
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

        // Create blend shader pipeline
        let blend_shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Blend Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/blend.wgsl").into()),
            });

        let blend_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Blend Pipeline Layout"),
                    bind_group_layouts: &[&blend_bind_group_layout],
                    push_constant_ranges: &[],
                });

        let blend_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Blend Pipeline"),
                    layout: Some(&blend_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &blend_shader,
                        entry_point: "vs_main",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &blend_shader,
                        entry_point: "fs_main",
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba8UnormSrgb,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 1,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview: None,
                });

        log::info!("GPU renderer initialized");

        Ok(Self {
            context,
            blit_pipeline,
            scale_pipeline,
            plasma_pipeline,
            waves_pipeline,
            gradient_pipeline,
            starfield_pipeline,
            matrix_pipeline,
            raymarching_pipeline,
            tunnel_pipeline,
            overlay_pipeline,
            blend_pipeline,
            blend_bind_group_layout,
            texture_bind_group_layout,
            shader_uniform_layout,
            overlay_bind_group_layout,
            sampler,
        })
    }

    /// Render a solid color to a buffer (proof-of-concept)
    ///
    /// This creates a simple colored texture on the GPU and renders it
    /// to an output buffer that can be used with shared memory.
    pub fn render_solid_color(&self, width: u32, height: u32, color: [u8; 4]) -> Result<Vec<u8>> {
        log::debug!(
            "GPU rendering {}x{} solid color: {:?}",
            width,
            height,
            color
        );

        // Create output texture
        let output_texture = self
            .context
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Output Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });

        let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create command encoder
        let mut encoder =
            self.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        // Clear to the specified color
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: color[0] as f64 / 255.0,
                            g: color[1] as f64 / 255.0,
                            b: color[2] as f64 / 255.0,
                            a: color[3] as f64 / 255.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        // Copy texture to buffer for CPU readback
        // Note: bytes_per_row must be aligned to COPY_BYTES_PER_ROW_ALIGNMENT (256)
        let bytes_per_row = width * 4;
        let aligned_bytes_per_row = (bytes_per_row + 255) & !255; // Align to 256
        let buffer_size = (aligned_bytes_per_row * height) as u64;

        let output_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Output Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &output_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &output_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(aligned_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        // Submit commands
        self.context.queue.submit(std::iter::once(encoder.finish()));

        // Map buffer and read data
        let buffer_slice = output_buffer.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });

        // Wait for mapping to complete
        self.context.device.poll(wgpu::Maintain::Wait);
        pollster::block_on(receiver).unwrap()?;

        // Get mapped data
        let data = buffer_slice.get_mapped_range();

        // Convert RGBA to ARGB for Wayland, accounting for padding
        let mut argb_data = vec![0u8; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let src_offset = (y * aligned_bytes_per_row + x * 4) as usize;
                let dst_offset = ((y * width + x) * 4) as usize;

                // Convert RGBA -> ARGB
                argb_data[dst_offset] = data[src_offset + 3]; // A
                argb_data[dst_offset + 1] = data[src_offset]; // R
                argb_data[dst_offset + 2] = data[src_offset + 1]; // G
                argb_data[dst_offset + 3] = data[src_offset + 2]; // B
            }
        }

        drop(data);
        output_buffer.unmap();

        log::debug!("GPU render complete, {} bytes", argb_data.len());
        Ok(argb_data)
    }

    /// Get GPU context reference
    pub fn context(&self) -> &GpuContext {
        &self.context
    }

    /// Render and scale an image using GPU acceleration
    ///
    /// Takes RGBA image data and scales it to the target dimensions.
    /// This is MUCH faster than CPU scaling, especially for large images.
    ///
    /// # Arguments
    /// * `image_data` - RGBA8 image data
    /// * `src_width` - Source image width
    /// * `src_height` - Source image height
    /// * `dst_width` - Target width
    /// * `dst_height` - Target height
    ///
    /// # Returns
    /// ARGB8 buffer suitable for Wayland shared memory
    pub fn render_image(
        &self,
        image_data: &[u8],
        src_width: u32,
        src_height: u32,
        dst_width: u32,
        dst_height: u32,
    ) -> Result<Vec<u8>> {
        log::debug!(
            "GPU rendering image: {}x{} -> {}x{}",
            src_width,
            src_height,
            dst_width,
            dst_height
        );

        // Upload source image to GPU
        let source_texture = GpuTexture::from_rgba(
            &self.context.device,
            &self.context.queue,
            &self.texture_bind_group_layout,
            &self.sampler,
            src_width,
            src_height,
            image_data,
        )?;

        // Create render target at destination size
        let target_texture = GpuTexture::create_render_target(
            &self.context.device,
            &self.texture_bind_group_layout,
            &self.sampler,
            dst_width,
            dst_height,
        )?;

        // Render scaled image
        let mut encoder =
            self.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Image Render Encoder"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Image Scale Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_texture.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.scale_pipeline);
            render_pass.set_bind_group(0, &source_texture.bind_group, &[]);
            render_pass.draw(0..3, 0..1); // Full-screen triangle
        }

        self.context.queue.submit(std::iter::once(encoder.finish()));

        // Read back to CPU as ARGB
        target_texture.read_to_argb(&self.context.device, &self.context.queue)
    }

    /// Render an ARGB image (Wayland format) with GPU scaling
    ///
    /// Convenience method that converts ARGB -> RGBA -> GPU -> ARGB
    pub fn render_image_argb(
        &self,
        image_data: &[u8],
        src_width: u32,
        src_height: u32,
        dst_width: u32,
        dst_height: u32,
    ) -> Result<Vec<u8>> {
        // Convert ARGB -> RGBA for GPU
        let mut rgba_data = vec![0u8; image_data.len()];
        for i in 0..(image_data.len() / 4) {
            let offset = i * 4;
            rgba_data[offset + 0] = image_data[offset + 2]; // R
            rgba_data[offset + 1] = image_data[offset + 1]; // G
            rgba_data[offset + 2] = image_data[offset + 0]; // B
            rgba_data[offset + 3] = image_data[offset + 3]; // A
        }

        self.render_image(&rgba_data, src_width, src_height, dst_width, dst_height)
    }

    /// Get bind group layout for texture operations
    pub fn texture_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.texture_bind_group_layout
    }

    /// Get sampler for texture operations
    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    /// Render a procedural shader effect
    ///
    /// # Arguments
    /// * `shader_type` - Which shader to render (plasma, waves, etc.)
    /// * `width` - Output width
    /// * `height` - Output height
    /// * `time` - Animation time in seconds
    ///
    /// # Returns
    /// ARGB8 buffer suitable for Wayland shared memory
    pub fn render_shader(
        &self,
        shader_type: &str,
        width: u32,
        height: u32,
        time: f32,
        params: &common::ShaderParams,
    ) -> Result<Vec<u8>> {
        log::debug!(
            "GPU rendering {} shader: {}x{} at time {:.2}s",
            shader_type,
            width,
            height,
            time
        );

        // Select pipeline
        let pipeline = match shader_type {
            "plasma" => &self.plasma_pipeline,
            "waves" => &self.waves_pipeline,
            "gradient" => &self.gradient_pipeline,
            "starfield" => &self.starfield_pipeline,
            "matrix" => &self.matrix_pipeline,
            "raymarching" => &self.raymarching_pipeline,
            "tunnel" => &self.tunnel_pipeline,
            _ => anyhow::bail!("Unknown shader type: {}", shader_type),
        };

        // Parse colors
        let color1 = params
            .color1
            .as_ref()
            .and_then(|c| common::ShaderParams::parse_color(c))
            .unwrap_or((1.0, 0.0, 0.0)); // Default red
        let color2 = params
            .color2
            .as_ref()
            .and_then(|c| common::ShaderParams::parse_color(c))
            .unwrap_or((0.0, 0.0, 1.0)); // Default blue
        let color3 = params
            .color3
            .as_ref()
            .and_then(|c| common::ShaderParams::parse_color(c))
            .unwrap_or((0.0, 1.0, 0.0)); // Default green

        // Create uniform buffer with shader parameters
        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct ShaderUniforms {
            time: f32,
            width: f32,
            height: f32,
            speed: f32,
            color1_r: f32,
            color1_g: f32,
            color1_b: f32,
            scale: f32,
            color2_r: f32,
            color2_g: f32,
            color2_b: f32,
            intensity: f32,
            color3_r: f32,
            color3_g: f32,
            color3_b: f32,
            count: f32, // Using f32 since WGSL requires alignment
        }

        let uniforms = ShaderUniforms {
            time,
            width: width as f32,
            height: height as f32,
            speed: params.speed.unwrap_or(1.0),
            color1_r: color1.0,
            color1_g: color1.1,
            color1_b: color1.2,
            scale: params.scale.unwrap_or(1.0),
            color2_r: color2.0,
            color2_g: color2.1,
            color2_b: color2.2,
            intensity: params.intensity.unwrap_or(1.0),
            color3_r: color3.0,
            color3_g: color3.1,
            color3_b: color3.2,
            count: params.count.unwrap_or(100) as f32,
        };

        let uniform_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Shader Uniforms"),
                    contents: bytemuck::cast_slice(&[uniforms]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

        // Create bind group for uniforms
        let uniform_bind_group =
            self.context
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Shader Uniform Bind Group"),
                    layout: &self.shader_uniform_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    }],
                });

        // Create render target
        let target_texture = GpuTexture::create_render_target(
            &self.context.device,
            &self.texture_bind_group_layout,
            &self.sampler,
            width,
            height,
        )?;

        // Render shader
        let mut encoder =
            self.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Shader Render Encoder"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Shader Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_texture.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(0, &uniform_bind_group, &[]);
            render_pass.draw(0..3, 0..1); // Full-screen triangle
        }

        self.context.queue.submit(std::iter::once(encoder.finish()));

        // Read back to CPU as ARGB
        target_texture.read_to_argb(&self.context.device, &self.context.queue)
    }

    /// Blend two ARGB frames for GPU-accelerated transitions
    ///
    /// # Arguments
    /// * `old_frame` - Previous frame (ARGB8)
    /// * `new_frame` - New frame (ARGB8)
    /// * `width` - Frame width
    /// * `height` - Frame height
    /// * `progress` - Transition progress (0.0 to 1.0)
    /// * `transition_type` - Type of transition (0=fade, 1=wipe_left, etc.)
    ///
    /// # Returns
    /// Blended ARGB8 buffer
    pub fn blend_frames(
        &self,
        old_frame: &[u8],
        new_frame: &[u8],
        width: u32,
        height: u32,
        progress: f32,
        transition_type: u32,
    ) -> Result<Vec<u8>> {
        log::debug!(
            "GPU blending frames: {}x{} progress={:.2}",
            width,
            height,
            progress
        );

        // Convert ARGB -> RGBA for GPU
        let mut old_rgba = vec![0u8; old_frame.len()];
        let mut new_rgba = vec![0u8; new_frame.len()];

        for i in 0..(old_frame.len() / 4) {
            let offset = i * 4;
            old_rgba[offset + 0] = old_frame[offset + 2]; // R
            old_rgba[offset + 1] = old_frame[offset + 1]; // G
            old_rgba[offset + 2] = old_frame[offset + 0]; // B
            old_rgba[offset + 3] = old_frame[offset + 3]; // A

            new_rgba[offset + 0] = new_frame[offset + 2]; // R
            new_rgba[offset + 1] = new_frame[offset + 1]; // G
            new_rgba[offset + 2] = new_frame[offset + 0]; // B
            new_rgba[offset + 3] = new_frame[offset + 3]; // A
        }

        // Create textures from both frames
        let old_texture = GpuTexture::from_rgba(
            &self.context.device,
            &self.context.queue,
            &self.texture_bind_group_layout,
            &self.sampler,
            width,
            height,
            &old_rgba,
        )?;

        let new_texture = GpuTexture::from_rgba(
            &self.context.device,
            &self.context.queue,
            &self.texture_bind_group_layout,
            &self.sampler,
            width,
            height,
            &new_rgba,
        )?;

        // Create output texture
        let target_texture = GpuTexture::create_render_target(
            &self.context.device,
            &self.texture_bind_group_layout,
            &self.sampler,
            width,
            height,
        )?;

        // Create uniform buffer
        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct BlendUniforms {
            progress: f32,
            transition_type: u32,
            width: f32,
            height: f32,
        }

        let uniforms = BlendUniforms {
            progress,
            transition_type,
            width: width as f32,
            height: height as f32,
        };

        let uniform_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Blend Uniforms"),
                    contents: bytemuck::cast_slice(&[uniforms]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

        // Create bind group
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Blend Bind Group"),
                layout: &self.blend_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&old_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&new_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });

        // Render blend
        let mut encoder =
            self.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Blend Encoder"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Blend Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_texture.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.blend_pipeline);
            render_pass.set_bind_group(0, &bind_group, &[]);
            render_pass.draw(0..3, 0..1); // Full-screen triangle
        }

        self.context.queue.submit(std::iter::once(encoder.finish()));

        // Read back to CPU as ARGB
        target_texture.read_to_argb(&self.context.device, &self.context.queue)
    }

    /// Render an overlay effect on top of a base texture
    ///
    /// # Arguments
    /// * `base_frame` - Base wallpaper frame (ARGB8)
    /// * `width` - Frame width
    /// * `height` - Frame height
    /// * `overlay_effect` - Type of overlay effect
    /// * `overlay_params` - Effect parameters
    /// * `time` - Animation time in seconds
    ///
    /// # Returns
    /// ARGB8 buffer with overlay applied
    pub fn render_with_overlay(
        &self,
        base_frame: &[u8],
        width: u32,
        height: u32,
        overlay_effect: common::OverlayEffect,
        overlay_params: &common::OverlayParams,
        time: f32,
    ) -> Result<Vec<u8>> {
        // Convert ARGB -> RGBA for GPU
        let mut rgba_data = vec![0u8; base_frame.len()];
        for i in 0..(base_frame.len() / 4) {
            let offset = i * 4;
            rgba_data[offset + 0] = base_frame[offset + 2]; // R
            rgba_data[offset + 1] = base_frame[offset + 1]; // G
            rgba_data[offset + 2] = base_frame[offset + 0]; // B
            rgba_data[offset + 3] = base_frame[offset + 3]; // A
        }

        // Upload base texture to GPU
        let base_texture = GpuTexture::from_rgba(
            &self.context.device,
            &self.context.queue,
            &self.texture_bind_group_layout,
            &self.sampler,
            width,
            height,
            &rgba_data,
        )?;

        // Create render target
        let target_texture = GpuTexture::create_render_target(
            &self.context.device,
            &self.texture_bind_group_layout,
            &self.sampler,
            width,
            height,
        )?;

        // Map overlay effect to shader effect type
        let effect_type = match overlay_effect {
            common::OverlayEffect::Vignette => 0.0,
            common::OverlayEffect::Scanlines => 1.0,
            common::OverlayEffect::FilmGrain => 2.0,
            common::OverlayEffect::ChromaticAberration => 3.0,
            common::OverlayEffect::Crt => 4.0,
            common::OverlayEffect::Pixelate => 5.0,
            common::OverlayEffect::ColorTint => 6.0,
        };

        // Create uniform buffer with overlay parameters
        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct OverlayUniforms {
            time: f32,
            width: f32,
            height: f32,
            effect_type: f32,
            param1: f32,
            param2: f32,
            param3: f32,
            param4: f32,
            color_r: f32,
            color_g: f32,
            color_b: f32,
            _padding: f32,
        }

        let uniforms = OverlayUniforms {
            time,
            width: width as f32,
            height: height as f32,
            effect_type,
            param1: overlay_params.strength.unwrap_or(0.0),
            param2: overlay_params.intensity.unwrap_or(0.0),
            param3: overlay_params.line_width.unwrap_or(1.0),
            param4: overlay_params.curvature.unwrap_or(0.0),
            color_r: overlay_params.r.unwrap_or(1.0),
            color_g: overlay_params.g.unwrap_or(1.0),
            color_b: overlay_params.b.unwrap_or(1.0),
            _padding: 0.0,
        };

        let uniform_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Overlay Uniforms"),
                    contents: bytemuck::cast_slice(&[uniforms]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

        // Create bind group
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Overlay Bind Group"),
                layout: &self.overlay_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&base_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                ],
            });

        // Render overlay
        let mut encoder =
            self.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Overlay Encoder"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Overlay Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_texture.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.overlay_pipeline);
            render_pass.set_bind_group(0, &bind_group, &[]);
            render_pass.draw(0..3, 0..1); // Full-screen triangle
        }

        self.context.queue.submit(std::iter::once(encoder.finish()));

        // Read back to CPU as ARGB
        target_texture.read_to_argb(&self.context.device, &self.context.queue)
    }
}

impl std::fmt::Debug for GpuRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuRenderer")
            .field("context", &self.context)
            .finish()
    }
}
