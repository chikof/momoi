use anyhow::{Context, Result};
use wgpu;

/// Represents a GPU texture with its bind group for shader access
pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
    pub width: u32,
    pub height: u32,
}

impl GpuTexture {
    /// Create a new GPU texture from RGBA8 image data
    pub fn from_rgba(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bind_group_layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Result<Self> {
        // Validate data size
        let expected_size = (width * height * 4) as usize;
        if data.len() != expected_size {
            anyhow::bail!(
                "Invalid texture data size: expected {} bytes ({}x{} RGBA), got {} bytes",
                expected_size,
                width,
                height,
                data.len()
            );
        }

        // Create texture
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("GPU Image Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload data to GPU
        queue.write_texture(
            texture.as_image_copy(),
            data,
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

        // Create texture view
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create bind group for shader access
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GPU Image Bind Group"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

        Ok(Self {
            texture,
            view,
            bind_group,
            width,
            height,
        })
    }

    /// Create a new GPU texture from BGRA8 video data (GStreamer format)
    /// This is optimized for video - uploads BGRA directly without CPU conversion
    pub fn from_bgra(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bind_group_layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Result<Self> {
        // Validate data size
        let expected_size = (width * height * 4) as usize;
        if data.len() != expected_size {
            anyhow::bail!(
                "Invalid texture data size: expected {} bytes ({}x{} BGRA), got {} bytes",
                expected_size,
                width,
                height,
                data.len()
            );
        }

        // Create texture with BGRA format (native for video)
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("GPU Video Texture (BGRA)"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload BGRA data directly to GPU (no CPU conversion needed!)
        queue.write_texture(
            texture.as_image_copy(),
            data,
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

        // Create texture view
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create bind group for shader access
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GPU Video Bind Group"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

        Ok(Self {
            texture,
            view,
            bind_group,
            width,
            height,
        })
    }

    /// Create a new GPU texture from ARGB8 image data (Wayland format)
    /// Converts ARGB -> RGBA for GPU upload
    #[allow(dead_code)] // Alternative API for ARGB texture creation
    pub fn from_argb(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bind_group_layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Result<Self> {
        // Convert ARGB -> RGBA
        let mut rgba_data = vec![0u8; data.len()];
        for i in 0..(data.len() / 4) {
            let offset = i * 4;
            rgba_data[offset + 0] = data[offset + 2]; // R
            rgba_data[offset + 1] = data[offset + 1]; // G
            rgba_data[offset + 2] = data[offset + 0]; // B
            rgba_data[offset + 3] = data[offset + 3]; // A
        }

        Self::from_rgba(
            device,
            queue,
            bind_group_layout,
            sampler,
            width,
            height,
            &rgba_data,
        )
    }

    /// Create an empty render target texture
    pub fn create_render_target(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("GPU Render Target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GPU Render Target Bind Group"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

        Ok(Self {
            texture,
            view,
            bind_group,
            width,
            height,
        })
    }

    /// Read texture data back to CPU as ARGB8 (Wayland format)
    pub fn read_to_argb(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Vec<u8>> {
        // Calculate aligned bytes per row (must be multiple of 256)
        let unpadded_bytes_per_row = self.width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;

        let buffer_size = (padded_bytes_per_row * self.height) as wgpu::BufferAddress;

        // Create staging buffer for GPU -> CPU copy
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GPU Texture Read Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Copy texture to buffer
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("GPU Texture Read Encoder"),
        });

        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &staging_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        queue.submit(Some(encoder.finish()));

        // Map buffer and read data
        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });

        let _ = device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });

        rx.recv()
            .context("Failed to receive buffer mapping result")?
            .context("Failed to map GPU buffer")?;

        let data = buffer_slice.get_mapped_range();

        // Convert RGBA -> ARGB and remove padding
        let mut argb_data = vec![0u8; (self.width * self.height * 4) as usize];

        for row in 0..self.height {
            let src_offset = (row * padded_bytes_per_row) as usize;
            let dst_offset = (row * self.width * 4) as usize;

            for col in 0..self.width {
                let src_pixel = src_offset + (col * 4) as usize;
                let dst_pixel = dst_offset + (col * 4) as usize;

                argb_data[dst_pixel + 0] = data[src_pixel + 2]; // B
                argb_data[dst_pixel + 1] = data[src_pixel + 1]; // G
                argb_data[dst_pixel + 2] = data[src_pixel + 0]; // R
                argb_data[dst_pixel + 3] = data[src_pixel + 3]; // A
            }
        }

        drop(data);
        staging_buffer.unmap();

        Ok(argb_data)
    }
}
