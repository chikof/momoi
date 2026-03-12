//! Image rendering and scaling operations.
//!
//! This module handles GPU-accelerated image rendering with bilinear filtering
//! for high-quality scaling from source to target dimensions.

use crate::gpu::{GpuContext, GpuTexture};
use anyhow::Result;
use wgpu;

/// Render and scale an image using GPU acceleration
///
/// Takes RGBA image data and scales it to the target dimensions.
/// This is MUCH faster than CPU scaling, especially for large images.
///
/// # Arguments
/// * `context` - GPU context
/// * `scale_pipeline` - The scaling shader pipeline
/// * `texture_bind_group_layout` - Bind group layout for textures
/// * `sampler` - Texture sampler
/// * `image_data` - RGBA8 image data
/// * `src_width` - Source image width
/// * `src_height` - Source image height
/// * `dst_width` - Target width
/// * `dst_height` - Target height
///
/// # Returns
/// ARGB8 buffer suitable for Wayland shared memory
pub fn render_image(
    context: &GpuContext,
    scale_pipeline: &wgpu::RenderPipeline,
    texture_bind_group_layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    image_data: &[u8],
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
) -> Result<Vec<u8>> {
    // Upload source image to GPU
    let source_texture = GpuTexture::from_rgba(
        &context.device,
        &context.queue,
        texture_bind_group_layout,
        sampler,
        src_width,
        src_height,
        image_data,
    )?;

    // Create render target at destination size
    let target_texture = GpuTexture::create_render_target(
        &context.device,
        texture_bind_group_layout,
        sampler,
        dst_width,
        dst_height,
    )?;

    // Render scaled image
    let mut encoder = context
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
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        render_pass.set_pipeline(scale_pipeline);
        render_pass.set_bind_group(0, &source_texture.bind_group, &[]);
        render_pass.draw(0..3, 0..1); // Full-screen triangle
    }

    context.queue.submit(std::iter::once(encoder.finish()));

    // Read back to CPU as ARGB
    target_texture.read_to_argb(&context.device, &context.queue)
}

/// Render an ARGB image (Wayland format) with GPU scaling
///
/// Convenience method that converts ARGB -> RGBA -> GPU -> ARGB
#[allow(dead_code)] // Alternative API for ARGB image rendering
pub fn render_image_argb(
    context: &GpuContext,
    scale_pipeline: &wgpu::RenderPipeline,
    texture_bind_group_layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
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

        rgba_data[offset] = image_data[offset + 2]; //     R
        rgba_data[offset + 1] = image_data[offset + 1]; // G
        rgba_data[offset + 2] = image_data[offset]; //     B
        rgba_data[offset + 3] = image_data[offset + 3]; // A
    }

    render_image(
        context,
        scale_pipeline,
        texture_bind_group_layout,
        sampler,
        &rgba_data,
        src_width,
        src_height,
        dst_width,
        dst_height,
    )
}
