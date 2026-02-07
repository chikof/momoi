/// GPU video buffer pool for async readback with double buffering
/// This eliminates GPU stalls by overlapping rendering and readback
use anyhow::Result;

/// State of a staging buffer in the buffer pool
enum BufferState {
    /// Buffer is free and ready to use
    Free,
    /// GPU copy submitted, waiting for completion
    Copying,
    /// Copy complete, map operation in progress
    Mapping(std::sync::mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>),
    /// Map complete, data ready to read (buffer is mapped)
    Mapped,
}

/// Double-buffered staging buffer pool for async GPU readback
/// Allows rendering frame N+1 while reading back frame N
pub struct VideoBufferPool {
    buffers: [(wgpu::Buffer, BufferState); 2],
    current_buffer_idx: usize,
    width: u32,
    height: u32,
    padded_bytes_per_row: u32,
}

impl VideoBufferPool {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let unpadded_bytes_per_row = width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
        let buffer_size = (padded_bytes_per_row * height) as wgpu::BufferAddress;

        // Create two staging buffers for double buffering
        let buffer_0 = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Video Staging Buffer 0"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let buffer_1 = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Video Staging Buffer 1"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Self {
            buffers: [(buffer_0, BufferState::Free), (buffer_1, BufferState::Free)],
            current_buffer_idx: 0,
            width,
            height,
            padded_bytes_per_row,
        }
    }

    /// Start async readback from GPU texture to staging buffer
    /// Returns immediately without waiting for GPU
    pub fn start_readback(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
    ) {
        // Find a free buffer
        let buffer_idx = self.current_buffer_idx;
        let (buffer, state) = &mut self.buffers[buffer_idx];

        // Buffer should be Free (unmapped immediately after read)
        if !matches!(state, BufferState::Free) {
            // Buffer not ready - this shouldn't happen with immediate unmapping
            let state_name = match state {
                BufferState::Free => "Free",
                BufferState::Copying => "Copying",
                BufferState::Mapping(_) => "Mapping",
                BufferState::Mapped => "Mapped (NOT UNMAPPED!)",
            };
            log::error!(
                "Buffer {} not ready for readback (state: {}), skipping frame. This indicates a buffer leak!",
                buffer_idx,
                state_name
            );
            // Check other buffer state too
            let other_idx = (buffer_idx + 1) % 2;
            let other_state_name = match &self.buffers[other_idx].1 {
                BufferState::Free => "Free",
                BufferState::Copying => "Copying",
                BufferState::Mapping(_) => "Mapping",
                BufferState::Mapped => "Mapped (NOT UNMAPPED!)",
            };
            log::error!("Buffer {} state: {}", other_idx, other_state_name);
            return;
        }

        // Start GPU copy
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Video Readback Encoder"),
        });

        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_bytes_per_row),
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

        // Mark as copying
        *state = BufferState::Copying;
        log::trace!("Started GPU copy to buffer {}", buffer_idx);

        // Swap to next buffer for next frame (double buffering: 0→1→0)
        self.current_buffer_idx = (self.current_buffer_idx + 1) % 2;
    }

    /// Try to read frame from the OTHER buffer (previous frame)
    /// With double buffering: if writing to buffer N, read from buffer N-1 (mod 2)
    /// Returns None if GPU hasn't finished yet
    pub fn try_read_frame(&mut self, device: &wgpu::Device) -> Result<Option<Vec<u8>>> {
        // Check the OTHER buffer (one position back in the ring)
        // current=0 → read=1, current=1 → read=0
        let read_buffer_idx = (self.current_buffer_idx + 1) % 2;

        // Check current state - only poll for Mapping state here
        // Copying state will poll inside transition_copying_to_mapping
        let needs_poll = matches!(&self.buffers[read_buffer_idx].1, BufferState::Mapping(_));

        if needs_poll {
            // Poll device ONLY when we need to check for completions
            // Increased timeout to 1ms to ensure GPU operations complete
            let _ = device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_millis(1)), // 1ms timeout
            });
        }

        // Now handle state transitions
        let (_buffer, state) = &mut self.buffers[read_buffer_idx];
        match state {
            BufferState::Free => Ok(None),
            BufferState::Copying => self.transition_copying_to_mapping(device, read_buffer_idx),
            BufferState::Mapping(_) => self.try_read_frame_from_mapping(read_buffer_idx),
            BufferState::Mapped => self.read_mapped_frame(read_buffer_idx),
        }
    }

    fn transition_copying_to_mapping(
        &mut self,
        device: &wgpu::Device,
        buffer_idx: usize,
    ) -> Result<Option<Vec<u8>>> {
        let (buffer, state) = &mut self.buffers[buffer_idx];

        if !matches!(state, BufferState::Copying) {
            return Ok(None);
        }

        // Start async map
        let buffer_slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).ok();
        });

        // CRITICAL: Poll device AFTER map_async to process the async operation
        // map_async just queues the request - it won't execute without a poll
        // Increased timeout to 1ms to ensure GPU operations complete
        let _ = device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_millis(1)), // 1ms timeout
        });

        // Update state to Mapping
        *state = BufferState::Mapping(rx);
        log::trace!("Started buffer mapping for buffer {}", buffer_idx);

        // Try again immediately (might complete instantly after the poll)
        self.try_read_frame_from_mapping(buffer_idx)
    }

    fn try_read_frame_from_mapping(&mut self, buffer_idx: usize) -> Result<Option<Vec<u8>>> {
        let (_buffer, state) = &mut self.buffers[buffer_idx];

        // Extract the receiver temporarily
        let rx = if let BufferState::Mapping(rx) = std::mem::replace(state, BufferState::Free) {
            rx
        } else {
            return Ok(None);
        };

        // Try to receive without blocking
        match rx.try_recv() {
            Ok(Ok(())) => {
                // Mapping complete!
                *state = BufferState::Mapped;
                log::trace!("Buffer {} mapping complete", buffer_idx);
                self.read_mapped_frame(buffer_idx)
            }
            Ok(Err(e)) => {
                *state = BufferState::Free;
                anyhow::bail!("Buffer mapping failed: {:?}", e)
            }
            Err(_) => {
                // Not ready yet, put receiver back
                *state = BufferState::Mapping(rx);
                Ok(None)
            }
        }
    }

    fn read_mapped_frame(&mut self, buffer_idx: usize) -> Result<Option<Vec<u8>>> {
        let (buffer, state) = &mut self.buffers[buffer_idx];

        if !matches!(state, BufferState::Mapped) {
            return Ok(None);
        }

        let buffer_slice = buffer.slice(..);
        let data = buffer_slice.get_mapped_range();

        // Convert RGBA -> ARGB and remove padding
        let mut argb_data = vec![0u8; (self.width * self.height * 4) as usize];

        for row in 0..self.height {
            let src_offset = (row * self.padded_bytes_per_row) as usize;
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

        // Unmap immediately after reading to free buffer for reuse
        // This prevents buffers from getting stuck in Mapped state
        buffer.unmap();
        *state = BufferState::Free;
        log::trace!("Read and unmapped buffer {}", buffer_idx);

        Ok(Some(argb_data))
    }

    /// Blocking read (for fallback compatibility)
    #[allow(dead_code)]
    pub fn read_frame_blocking(&self, device: &wgpu::Device) -> Result<Vec<u8>> {
        let prev_buffer = (self.current_buffer_idx + 1) % 2;
        let (staging_buffer, _) = &self.buffers[prev_buffer];

        // Block until GPU completes
        let _ = device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).ok();
        });

        let _ = device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });

        rx.recv()
            .map_err(|_| anyhow::anyhow!("Failed to receive map result"))?
            .map_err(|e| anyhow::anyhow!("Buffer mapping failed: {:?}", e))?;

        let data = buffer_slice.get_mapped_range();

        // Convert RGBA -> ARGB and remove padding
        let mut argb_data = vec![0u8; (self.width * self.height * 4) as usize];

        for row in 0..self.height {
            let src_offset = (row * self.padded_bytes_per_row) as usize;
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
