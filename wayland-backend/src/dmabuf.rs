//! `linux-dmabuf-v1` zero-copy GPU frame submission.
//!
//! When the GPU renderer produces a frame into a DRM dma-buf, this module
//! imports it as a `zwp_linux_buffer_params_v1`, creating a `wl_buffer`
//! the compositor can scan out directly — no CPU readback required.

use crate::WaylandError;
use std::os::unix::io::BorrowedFd;
use tracing::{debug, info, warn};
use wayland_client::{Dispatch, QueueHandle, protocol::wl_buffer::WlBuffer};
use wayland_protocols::wp::linux_dmabuf::zv1::client::{
    zwp_linux_buffer_params_v1::{self, ZwpLinuxBufferParamsV1},
    zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1,
};

/// DRM pixel format (fourcc code).
pub type DrmFormat = u32;

/// DRM format modifier (tiling/compression hint).
pub type DrmModifier = u64;

/// Common formats.
pub mod fmt {
    /// ARGB 8-bit per channel, little-endian.
    pub const ARGB8888: u32 = 0x3431_5241; // fourcc "AR24"
    /// XRGB 8-bit per channel (opaque).
    pub const XRGB8888: u32 = 0x3458_5258; // fourcc "XR24"
    /// Linear (no tiling) modifier.
    pub const MOD_LINEAR: u64 = 0;
}

/// Negotiated dmabuf format/modifier pair advertised by the compositor.
#[derive(Debug, Clone, Copy)]
pub struct DmabufFormat {
    /// DRM fourcc format code.
    pub format: DrmFormat,
    /// DRM format modifier.
    pub modifier: DrmModifier,
}

/// Holds the `zwp_linux_dmabuf_v1` global and the list of compositor-supported
/// format/modifier pairs.
pub struct DmabufSession {
    /// The dmabuf protocol object.
    pub dmabuf: ZwpLinuxDmabufV1,
    /// Formats and modifiers the compositor advertised during binding.
    pub formats: Vec<DmabufFormat>,
}

impl DmabufSession {
    /// Wrap an already-bound `ZwpLinuxDmabufV1` global.
    #[must_use]
    pub fn new(dmabuf: ZwpLinuxDmabufV1) -> Self {
        Self {
            dmabuf,
            formats: Vec::new(),
        }
    }

    /// Record a format/modifier pair advertised by the compositor.
    pub fn add_format(&mut self, format: DrmFormat, modifier: DrmModifier) {
        debug!(format, modifier, "dmabuf format advertised");
        self.formats.push(DmabufFormat { format, modifier });
    }

    /// Return `true` if the compositor supports `(format, modifier)`.
    #[must_use]
    pub fn supports(&self, format: DrmFormat, modifier: DrmModifier) -> bool {
        self.formats
            .iter()
            .any(|f| f.format == format && f.modifier == modifier)
    }

    /// Import a dma-buf file descriptor as a `wl_buffer`.
    ///
    /// `fd` must remain valid until the compositor sends a `wl_buffer::release` event.
    ///
    /// # Errors
    /// Returns [`WaylandError::Dmabuf`] if the format is unsupported.
    #[allow(clippy::too_many_arguments)]
    pub fn import_buffer<D>(
        &self,
        qh: &QueueHandle<D>,
        fd: BorrowedFd<'_>,
        width: u32,
        height: u32,
        stride: u32,
        format: DrmFormat,
        modifier: DrmModifier,
    ) -> Result<WlBuffer, WaylandError>
    where
        D: Dispatch<ZwpLinuxBufferParamsV1, DmabufBufferData>
            + Dispatch<WlBuffer, DmabufBufferData>
            + 'static,
    {
        if !self.supports(format, modifier) {
            return Err(WaylandError::Dmabuf(format!(
                "format {format:#010x} / modifier {modifier:#018x} not supported by compositor"
            )));
        }

        let params: ZwpLinuxBufferParamsV1 = self.dmabuf.create_params(qh, DmabufBufferData);

        params.add(
            fd,
            0, // plane index
            0, // offset
            stride,
            (modifier >> 32) as u32,
            u32::try_from(modifier)?,
        );

        // create_immed produces a wl_buffer synchronously (no roundtrip needed).
        let buffer = params.create_immed(
            width.cast_signed(),
            height.cast_signed(),
            format,
            zwp_linux_buffer_params_v1::Flags::empty(),
            qh,
            DmabufBufferData,
        );

        info!(width, height, format, "dmabuf buffer imported");
        Ok(buffer)
    }
}

/// User-data attached to dmabuf `wl_buffer` objects.
#[derive(Debug, Default, Clone)]
pub struct DmabufBufferData;

impl<D> Dispatch<ZwpLinuxBufferParamsV1, DmabufBufferData, D> for DmabufSession
where
    D: Dispatch<ZwpLinuxBufferParamsV1, DmabufBufferData>,
{
    fn event(
        _state: &mut D,
        _proxy: &ZwpLinuxBufferParamsV1,
        event: zwp_linux_buffer_params_v1::Event,
        _data: &DmabufBufferData,
        _conn: &wayland_client::Connection,
        _qh: &QueueHandle<D>,
    ) {
        match event {
            zwp_linux_buffer_params_v1::Event::Created { buffer: _ } => {
                debug!("dmabuf buffer params created");
            }
            zwp_linux_buffer_params_v1::Event::Failed => {
                warn!("dmabuf buffer creation failed");
            }
            _ => {}
        }
    }
}
