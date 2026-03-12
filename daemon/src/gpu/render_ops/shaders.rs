//! Shader rendering operations for procedural wallpapers.
//!
//! This module handles rendering of GPU-accelerated procedural shaders like
//! plasma, waves, gradient, starfield, matrix, raymarching, and tunnel effects.

use crate::gpu::renderer::ShaderResources;
use crate::gpu::{GpuContext, GpuTexture};
use anyhow::Result;
use wgpu;
use wgpu::util::DeviceExt;

/// Shader uniform data layout, matching WGSL struct alignment.
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
    count: f32,
}

/// Build ShaderUniforms from shader params
fn build_uniforms(
    width: u32,
    height: u32,
    time: f32,
    params: &common::ShaderParams,
) -> ShaderUniforms {
    let color1 = params
        .color1
        .as_ref()
        .and_then(|c| common::ShaderParams::parse_color(c))
        .unwrap_or((1.0, 0.0, 0.0));
    let color2 = params
        .color2
        .as_ref()
        .and_then(|c| common::ShaderParams::parse_color(c))
        .unwrap_or((0.0, 0.0, 1.0));
    let color3 = params
        .color3
        .as_ref()
        .and_then(|c| common::ShaderParams::parse_color(c))
        .unwrap_or((0.0, 1.0, 0.0));

    ShaderUniforms {
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
    }
}

/// Render a procedural shader effect (allocates fresh resources each call).
///
/// This is the original implementation kept for non-hot-path callers.
/// For the 60fps shader hot path, use `render_shader_cached` instead.
#[allow(dead_code)] // Kept as non-cached fallback API
pub fn render_shader(
    context: &GpuContext,
    pipeline: &wgpu::RenderPipeline,
    shader_uniform_layout: &wgpu::BindGroupLayout,
    texture_bind_group_layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    width: u32,
    height: u32,
    time: f32,
    params: &common::ShaderParams,
) -> Result<Vec<u8>> {
    let uniforms = build_uniforms(width, height, time, params);

    let uniform_buffer = context
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shader Uniforms"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

    let uniform_bind_group = context
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shader Uniform Bind Group"),
            layout: shader_uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

    let target_texture = GpuTexture::create_render_target(
        &context.device,
        texture_bind_group_layout,
        sampler,
        width,
        height,
    )?;

    let mut encoder = context
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
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, &uniform_bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    context.queue.submit(std::iter::once(encoder.finish()));

    target_texture.read_to_argb(&context.device, &context.queue)
}

/// Render a procedural shader effect using cached GPU resources.
///
/// Eliminates per-frame GPU allocations by reusing:
/// - Render target texture (keyed by resolution)
/// - Uniform buffer (updated via `queue.write_buffer`)
/// - Bind group (always references the same uniform buffer)
/// - Double-buffered staging pool (async readback, no GPU stalls)
///
/// Returns the PREVIOUS frame's data while the current frame renders async.
/// Returns `None` on the very first call (no previous frame yet).
///
/// This is the hot-path variant called at 30-60fps for shader wallpapers.
#[allow(clippy::too_many_arguments)]
pub fn render_shader_cached(
    context: &GpuContext,
    pipeline: &wgpu::RenderPipeline,
    shader_uniform_layout: &wgpu::BindGroupLayout,
    texture_bind_group_layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    shader_resources: &tokio::sync::Mutex<std::collections::HashMap<(u32, u32), ShaderResources>>,
    width: u32,
    height: u32,
    time: f32,
    params: &common::ShaderParams,
) -> Result<Option<Vec<u8>>> {
    let uniforms = build_uniforms(width, height, time, params);

    // Acquire lock and ensure resources exist for this resolution
    let mut resources_map = shader_resources.blocking_lock();
    let key = (width, height);

    let resources = resources_map.entry(key).or_insert_with(|| {
        info!(
            "Creating cached shader resources for {}x{} (render target + uniform buffer + async staging pool)",
            width,
            height
        );

        let target_texture = GpuTexture::create_render_target(
            &context.device,
            texture_bind_group_layout,
            sampler,
            width,
            height,
        )
        .expect("Failed to create shader render target");

        let uniform_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Shader Uniforms (cached)"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let uniform_bind_group = context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Shader Uniform Bind Group (cached)"),
                layout: shader_uniform_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                }],
            });

        // Create double-buffered staging pool for async readback
        let buffer_pool = crate::gpu::VideoBufferPool::new(&context.device, width, height);

        ShaderResources {
            target_texture,
            uniform_buffer,
            uniform_bind_group,
            buffer_pool,
        }
    });

    // Update uniform buffer contents (no allocation, just a GPU memcpy)
    context.queue.write_buffer(
        &resources.uniform_buffer,
        0,
        bytemuck::cast_slice(&[uniforms]),
    );

    // Render using cached resources
    let mut encoder = context
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Shader Render Encoder"),
        });
    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Shader Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &resources.target_texture.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, &resources.uniform_bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    context.queue.submit(std::iter::once(encoder.finish()));

    // Start async readback of current frame into staging buffer (non-blocking)
    resources.buffer_pool.start_readback(
        &context.device,
        &context.queue,
        &resources.target_texture.texture,
    );

    // Try to read the PREVIOUS frame from the other staging buffer (non-blocking)
    resources.buffer_pool.try_read_frame(&context.device)
}
