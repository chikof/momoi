use std::os::fd::AsFd;
use std::sync::{Arc, Mutex};
use wayland_client::protocol::{wl_buffer, wl_shm, wl_shm_pool};
use wayland_client::{Dispatch, QueueHandle};

/// Buffer state tracked by Wayland compositor
#[derive(Debug, Clone)]
pub struct BufferState {
    /// Whether the buffer is currently in use by the compositor
    pub busy: bool,
}

impl BufferState {
    pub fn new() -> Self {
        Self { busy: true }
    }
}

/// Helper for creating and managing shared memory buffers for Wayland
pub struct ShmBuffer {
    pool: wl_shm_pool::WlShmPool,
    buffer: wl_buffer::WlBuffer,
    mmap: memmap2::MmapMut,
    width: u32,
    height: u32,
    /// Shared state for tracking buffer usage
    pub state: Arc<Mutex<BufferState>>,
}

impl ShmBuffer {
    pub fn new<D>(
        shm: &wl_shm::WlShm,
        width: u32,
        height: u32,
        qh: &QueueHandle<D>,
    ) -> anyhow::Result<Self>
    where
        D: Dispatch<wl_shm_pool::WlShmPool, ()>
            + Dispatch<wl_buffer::WlBuffer, Arc<Mutex<BufferState>>>
            + 'static,
    {
        let stride = width * 4; // 4 bytes per pixel (ARGB8888)
        let size = stride * height;

        // Create a temporary file for shared memory
        let file = tempfile::tempfile()?;
        file.set_len(size as u64)?;

        // Memory map the file
        let mmap = unsafe { memmap2::MmapMut::map_mut(&file)? };

        // Create Wayland shm pool
        let pool = shm.create_pool(file.as_fd(), size as i32, qh, ());

        // Create shared state for tracking buffer usage
        let state = Arc::new(Mutex::new(BufferState::new()));

        // Create buffer from pool with state as user data
        let buffer = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            stride as i32,
            wl_shm::Format::Argb8888,
            qh,
            state.clone(),
        );

        Ok(Self {
            pool,
            buffer,
            mmap,
            width,
            height,
            state,
        })
    }

    /// Check if the buffer is safe to reuse (not busy)
    pub fn is_released(&self) -> bool {
        self.state.lock().map(|s| !s.busy).unwrap_or(false)
    }

    /// Mark buffer as busy (attached to surface)
    pub fn mark_busy(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.busy = true;
        }
    }

    pub fn fill_color(&mut self, r: u8, g: u8, b: u8, a: u8) {
        let color = u32::from_ne_bytes([b, g, r, a]); // ARGB8888 format

        for chunk in self.mmap.chunks_exact_mut(4) {
            chunk.copy_from_slice(&color.to_ne_bytes());
        }
    }

    /// Write image data to the buffer
    /// Data must be in ARGB8888 format (BGRA byte order)
    pub fn write_image_data(&mut self, data: &[u8]) -> anyhow::Result<()> {
        if data.len() != self.mmap.len() {
            anyhow::bail!(
                "Image data size mismatch: expected {}, got {}",
                self.mmap.len(),
                data.len()
            );
        }

        self.mmap.copy_from_slice(data);
        Ok(())
    }

    /// Read the current buffer data
    /// Returns a copy of the buffer data in ARGB8888 format
    pub fn read_data(&self) -> anyhow::Result<Vec<u8>> {
        Ok(self.mmap.to_vec())
    }

    pub fn buffer(&self) -> &wl_buffer::WlBuffer {
        &self.buffer
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

impl Drop for ShmBuffer {
    fn drop(&mut self) {
        self.buffer.destroy();
        self.pool.destroy();
    }
}

/// Parse a hex color string (e.g., "#FF5733" or "FF5733") to RGBA
pub fn parse_hex_color(color: &str) -> Option<(u8, u8, u8, u8)> {
    let color = color.trim_start_matches('#');

    if color.len() != 6 && color.len() != 8 {
        return None;
    }

    let r = u8::from_str_radix(&color[0..2], 16).ok()?;
    let g = u8::from_str_radix(&color[2..4], 16).ok()?;
    let b = u8::from_str_radix(&color[4..6], 16).ok()?;
    let a = if color.len() == 8 {
        u8::from_str_radix(&color[6..8], 16).ok()?
    } else {
        255
    };

    Some((r, g, b, a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_color() {
        assert_eq!(parse_hex_color("#FF5733"), Some((255, 87, 51, 255)));
        assert_eq!(parse_hex_color("FF5733"), Some((255, 87, 51, 255)));
        assert_eq!(parse_hex_color("#FF573380"), Some((255, 87, 51, 128)));
        assert_eq!(parse_hex_color("000000"), Some((0, 0, 0, 255)));
        assert_eq!(parse_hex_color("FFFFFF"), Some((255, 255, 255, 255)));
        assert_eq!(parse_hex_color("invalid"), None);
    }
}
