//! `wgpu` render pipeline built from a validated naga module.

use crate::{ShaderError, compiler::CompiledShader};
use std::sync::Arc;

/// A GPU render pipeline bound to a specific compiled shader.
pub struct ShaderPipeline {
    /// The wgpu pipeline.
    pub pipeline: wgpu::RenderPipeline,
    /// Bind group layout for the uniform buffer at binding 0.
    pub uniform_layout: wgpu::BindGroupLayout,
    /// Source shader (retained for diagnostics).
    pub shader: Arc<CompiledShader>,
}

impl ShaderPipeline {
    /// Build a render pipeline from a validated shader.
    ///
    /// # Errors
    /// Returns [`ShaderError::Validation`] if pipeline creation fails.
    pub fn create(
        device: &wgpu::Device,
        shader: Arc<CompiledShader>,
        target_format: wgpu::TextureFormat,
    ) -> Result<Self, ShaderError> {
        let wgpu_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: shader.source.path.as_deref().and_then(|p| p.to_str()),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(&shader.source.code)),
        });

        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[Some(&uniform_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                    format: target_format,
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
            multiview_mask: None, // None = no multiview; NonZero::new(n) for n layers
            cache: None,
        });

        Ok(Self {
            pipeline,
            uniform_layout,
            shader,
        })
    }
}
