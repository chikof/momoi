//! Manages the wgpu uniform buffer and bind group for shader uniforms.

use render_core::ShaderUniforms;

/// Owns a wgpu `Buffer` pre-loaded with [`ShaderUniforms`] and its bind group.
pub struct UniformBuffer {
    /// The GPU-side uniform buffer.
    pub buffer: wgpu::Buffer,
    /// Bind group referencing `buffer` at binding 0.
    pub bind_group: wgpu::BindGroup,
}

impl UniformBuffer {
    /// Allocate a new uniform buffer and create its bind group.
    #[must_use]
    pub fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        use wgpu::util::DeviceExt;
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniforms"),
            contents: bytemuck::bytes_of(&ShaderUniforms::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniforms_bg"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        Self { buffer, bind_group }
    }

    /// Upload updated uniforms to the GPU.
    pub fn update(&self, queue: &wgpu::Queue, uniforms: &ShaderUniforms) {
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(uniforms));
    }
}
