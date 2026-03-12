//! Frame blending operations for transitions.
//!
//! This module handles GPU-accelerated blending of two frames for smooth
//! transitions between wallpapers (fade, wipe, etc.).

use crate::gpu::renderer::BlendResources;
use crate::gpu::{GpuContext, GpuTexture, VideoBufferPool};
use anyhow::Result;
use wgpu;
use wgpu::util::DeviceExt;

/// Blend uniform data layout, matching WGSL struct alignment.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct BlendUniforms {
    progress: f32,
    transition_type: u32,
    width: f32,
    height: f32,
}

/// Blend two ARGB frames for GPU-accelerated transitions (allocates fresh resources).
///
/// This is the original implementation kept as a fallback.
/// For the 60fps transition hot path, use `blend_frames_cached` instead.
#[allow(dead_code)] // Kept as non-cached fallback API
pub fn blend_frames(
    context: &GpuContext,
    blend_pipeline: &wgpu::RenderPipeline,
    blend_bind_group_layout: &wgpu::BindGroupLayout,
    texture_bind_group_layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    old_frame: &[u8],
    new_frame: &[u8],
    width: u32,
    height: u32,
    progress: f32,
    transition_type: u32,
) -> Result<Vec<u8>> {
    // Convert ARGB -> RGBA for GPU
    let mut old_rgba = vec![0u8; old_frame.len()];
    let mut new_rgba = vec![0u8; new_frame.len()];

    for (src, dst) in old_frame.chunks_exact(4).zip(old_rgba.chunks_exact_mut(4)) {
        dst[0] = src[2]; // R
        dst[1] = src[1]; // G
        dst[2] = src[0]; // B
        dst[3] = src[3]; // A
    }
    for (src, dst) in new_frame.chunks_exact(4).zip(new_rgba.chunks_exact_mut(4)) {
        dst[0] = src[2]; // R
        dst[1] = src[1]; // G
        dst[2] = src[0]; // B
        dst[3] = src[3]; // A
    }

    // Create textures from both frames
    let old_texture = GpuTexture::from_rgba(
        &context.device,
        &context.queue,
        texture_bind_group_layout,
        sampler,
        width,
        height,
        &old_rgba,
    )?;

    let new_texture = GpuTexture::from_rgba(
        &context.device,
        &context.queue,
        texture_bind_group_layout,
        sampler,
        width,
        height,
        &new_rgba,
    )?;

    // Create output texture
    let target_texture = GpuTexture::create_render_target(
        &context.device,
        texture_bind_group_layout,
        sampler,
        width,
        height,
    )?;

    let uniforms = BlendUniforms {
        progress,
        transition_type,
        width: width as f32,
        height: height as f32,
    };

    let uniform_buffer = context
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Blend Uniforms"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

    // Create bind group
    let bind_group = context
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blend Bind Group"),
            layout: blend_bind_group_layout,
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
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

    // Render blend
    let mut encoder = context
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
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        render_pass.set_pipeline(blend_pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    context.queue.submit(std::iter::once(encoder.finish()));

    target_texture.read_to_argb(&context.device, &context.queue)
}

/// Blend two ARGB frames using cached GPU resources.
///
/// Eliminates per-frame GPU allocations by reusing:
/// - Source textures for old/new frames (data updated via `queue.write_texture`)
/// - Render target texture
/// - Uniform buffer (updated via `queue.write_buffer`)
/// - Double-buffered staging pool (async readback, no GPU stalls)
///
/// Input frames are uploaded directly as BGRA textures (Wayland ARGB is BGRA
/// in memory on little-endian), eliminating the two per-frame CPU swizzle passes.
///
/// Returns the PREVIOUS frame's blended data while the current frame renders async.
/// Returns `None` on the very first call (no previous frame yet).
///
/// The bind group must be recreated each frame because it references texture views
/// that are tied to specific textures (wgpu bind groups are immutable).
#[allow(clippy::too_many_arguments)]
pub fn blend_frames_cached(
    context: &GpuContext,
    blend_pipeline: &wgpu::RenderPipeline,
    blend_bind_group_layout: &wgpu::BindGroupLayout,
    texture_bind_group_layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    blend_resources: &tokio::sync::Mutex<std::collections::HashMap<(u32, u32), BlendResources>>,
    old_frame: &[u8],
    new_frame: &[u8],
    width: u32,
    height: u32,
    progress: f32,
    transition_type: u32,
) -> Result<Option<Vec<u8>>> {
    let uniforms = BlendUniforms {
        progress,
        transition_type,
        width: width as f32,
        height: height as f32,
    };

    let mut resources_map = blend_resources.blocking_lock();
    let key = (width, height);

    let resources = resources_map.entry(key).or_insert_with(|| {
        info!(
            "Creating cached blend resources for {}x{} (2 source textures + render target + async staging pool)",
            width,
            height
        );

        // Create BGRA source textures (Wayland ARGB is BGRA in memory)
        let old_texture = GpuTexture::from_bgra(
            &context.device,
            &context.queue,
            texture_bind_group_layout,
            sampler,
            width,
            height,
            old_frame,
        )
        .expect("Failed to create blend old texture");

        let new_texture = GpuTexture::from_bgra(
            &context.device,
            &context.queue,
            texture_bind_group_layout,
            sampler,
            width,
            height,
            new_frame,
        )
        .expect("Failed to create blend new texture");

        let target_texture = GpuTexture::create_render_target(
            &context.device,
            texture_bind_group_layout,
            sampler,
            width,
            height,
        )
        .expect("Failed to create blend render target");

        let uniform_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Blend Uniforms (cached)"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        // Create bind group (can be cached because texture views and uniform buffer
        // are the same objects across frames — only their contents change)
        let bind_group = context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Blend Bind Group (cached)"),
                layout: blend_bind_group_layout,
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
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            });

        // Create double-buffered staging pool for async readback
        let buffer_pool = VideoBufferPool::new(&context.device, width, height);

        BlendResources {
            old_texture,
            new_texture,
            target_texture,
            uniform_buffer,
            bind_group,
            buffer_pool,
        }
    });

    // Update source textures with frame data (BGRA direct upload, no CPU swizzle)
    context.queue.write_texture(
        resources.old_texture.texture.as_image_copy(),
        old_frame,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    context.queue.write_texture(
        resources.new_texture.texture.as_image_copy(),
        new_frame,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    // Update uniform buffer (no allocation)
    context.queue.write_buffer(
        &resources.uniform_buffer,
        0,
        bytemuck::cast_slice(&[uniforms]),
    );

    // Render blend using cached bind group
    let mut encoder = context
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Blend Encoder"),
        });
    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Blend Pass"),
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

        render_pass.set_pipeline(blend_pipeline);
        render_pass.set_bind_group(0, &resources.bind_group, &[]);
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
