//! `wl_shm` double-buffered pixel buffer.
//!
//! Uses the ORIGINAL `Dispatch<WlBuffer, ()>` user-data type so it works
//! with the unmodified `MonitorManager` from the repo.  Double-buffering
//! (two alternating slots) is sufficient for a wallpaper daemon at typical
//! frame rates — the compositor is done with slot N long before we write
//! slot N again.

use crate::WaylandError;
use std::os::unix::io::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use tracing::{debug, info};
use wayland_client::{
    Dispatch, QueueHandle,
    protocol::{
        wl_buffer::WlBuffer,
        wl_shm::{Format, WlShm},
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
};

/// Double-buffered `wl_shm` pixel buffer for one output surface.
///
/// Allocates a single `memfd` containing two frame slots and alternates
/// which slot is committed on each call to [`present`](ShmBuffer::present).
pub struct ShmBuffer {
    /// Memory-mapped pointer into the memfd.
    mapping: *mut u8,
    /// Total mapping length in bytes (= 2 × stride × height).
    mapping_size: usize,
    /// Row stride in bytes (= width × 4).
    stride: u32,
    /// Single-frame byte count.
    frame_size: usize,
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
    /// The two `wl_buffer` objects.
    buffers: [WlBuffer; 2],
    /// Which slot was last committed (0 or 1).
    front: usize,
    /// Keep the fd alive so the mapping stays valid.
    _fd: OwnedFd,
}

// the raw pointer is only ever accessed through `&mut self` methods.
unsafe impl Send for ShmBuffer {}

impl ShmBuffer {
    /// Allocate a double-buffered shm pool for `width × height` RGBA pixels.
    ///
    /// `D` must implement `Dispatch<WlShmPool, ()>` and `Dispatch<WlBuffer, ()>` —
    /// both are satisfied by the original `MonitorManager`.
    ///
    /// # Errors
    /// Returns [`WaylandError::ShmAlloc`] or [`WaylandError::Io`] on failure.
    pub fn new<D>(
        shm: &WlShm,
        qh: &QueueHandle<D>,
        width: u32,
        height: u32,
    ) -> Result<Self, WaylandError>
    where
        D: Dispatch<WlShmPool, ()> + Dispatch<WlBuffer, ()> + 'static,
    {
        let stride = width * 4;
        let frame_size = (stride * height) as usize;
        let pool_size = frame_size * 2;

        let fd = create_memfd(pool_size)?;

        // fd is a valid anonymous memfd of exactly `pool_size` bytes.
        let mapping = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                pool_size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                std::os::unix::io::AsRawFd::as_raw_fd(&fd),
                0,
            )
        };
        if mapping == libc::MAP_FAILED {
            return Err(WaylandError::Io(std::io::Error::last_os_error()));
        }

        let pool: WlShmPool = shm.create_pool(fd.as_fd(), i32::try_from(pool_size)?, qh, ());

        // Argb8888: the most universally supported wl_shm format.
        // Pixel byte order in memory: B G R A (little-endian ARGB).
        // wgpu renders Rgba8Unorm (R G B A), so we swap R↔B in present().
        let buf0 = pool.create_buffer(
            0,
            width.cast_signed(),
            height.cast_signed(),
            stride.cast_signed(),
            Format::Abgr8888,
            qh,
            (),
        );
        let buf1 = pool.create_buffer(
            i32::try_from(frame_size)?,
            width.cast_signed(),
            height.cast_signed(),
            stride.cast_signed(),
            Format::Abgr8888,
            qh,
            (),
        );
        pool.destroy();

        info!(width, height, pool_bytes = pool_size, "shm pool allocated");
        Ok(Self {
            mapping: mapping.cast(),
            mapping_size: pool_size,
            stride,
            frame_size,
            width,
            height,
            buffers: [buf0, buf1],
            front: 0,
            _fd: fd,
        })
    }

    /// Copy `pixels` into the back slot, converting RGBA→BGRA, then commit.
    ///
    /// `pixels` must be RGBA row-major, at least `width × height × 4` bytes.
    pub fn present(&mut self, surface: &WlSurface, pixels: &[u8]) {
        let expected = self.frame_size;
        if pixels.len() < expected {
            tracing::warn!(
                got = pixels.len(),
                expected,
                "pixel buffer too small — dropping frame"
            );
            return;
        }

        let back = 1 - self.front;
        let offset = back * self.frame_size;

        // offset..offset+expected is within the mapping.
        // Abgr8888 memory layout: byte0=R, byte1=G, byte2=B, byte3=A
        // wgpu Rgba8Unorm:        byte0=R, byte1=G, byte2=B, byte3=A
        // Perfect match — direct memcpy, no conversion.
        unsafe {
            let dst = std::slice::from_raw_parts_mut(self.mapping.add(offset), expected);
            dst.copy_from_slice(&pixels[..expected]);
        }

        surface.attach(Some(&self.buffers[back]), 0, 0);
        surface.damage_buffer(0, 0, self.width.cast_signed(), self.height.cast_signed());
        surface.commit();

        self.front = back;
        debug!(slot = back, "shm frame committed");
    }
}

impl Drop for ShmBuffer {
    fn drop(&mut self) {
        // mapping was created by mmap in new().
        unsafe { libc::munmap(self.mapping.cast(), self.mapping_size) };
        for buf in &self.buffers {
            buf.destroy();
        }
        // _fd drops here, closing the memfd.
    }
}

#[allow(clippy::cast_possible_wrap)]
fn create_memfd(size: usize) -> Result<OwnedFd, WaylandError> {
    // valid syscall arguments.
    let fd = unsafe { libc::memfd_create(c"momoi-shm".as_ptr(), libc::MFD_CLOEXEC) };
    if fd < 0 {
        return Err(WaylandError::Io(std::io::Error::last_os_error()));
    }
    // fd is valid and owned.
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    if unsafe { libc::ftruncate(fd.as_raw_fd(), size as libc::off_t) } < 0 {
        return Err(WaylandError::Io(std::io::Error::last_os_error()));
    }
    Ok(fd)
}
