//! Layer-shell surface management.
//!
//! `WallpaperSurface` is a plain data struct — no Wayland dispatch here.
//! All protocol dispatch lives in `MonitorManager`, which is the single
//! top-level state type passed to every SCTK delegation macro.

use crate::ShmBuffer;
use render_core::OutputInfo;
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerSurface, LayerSurfaceConfigure,
};
use std::collections::HashMap;
use tracing::{info, warn};
use wayland_client::{
    QueueHandle,
    protocol::{wl_output::WlOutput, wl_surface::WlSurface},
};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1;

/// One fully-configured background layer surface for a single output.
pub struct WallpaperSurface {
    /// SCTK layer surface handle.
    pub layer_surface: LayerSurface,
    /// Underlying `wl_surface` used for buffer attachment.
    pub wl_surface: WlSurface,
    /// Output metadata.
    pub output: OutputInfo,
    /// True once the compositor has sent its first `configure`.
    pub configured: bool,
    /// Current configured width in pixels.
    pub width: u32,
    /// Current configured height in pixels.
    pub height: u32,
    /// CPU shm buffer — `None` until after first configure.
    pub shm: Option<ShmBuffer>,
}

impl WallpaperSurface {
    /// Write pixels to the shm back-buffer and commit the surface.
    ///
    /// Silently drops the frame if the surface hasn't been configured yet.
    pub fn present_cpu(&mut self, pixels: &[u8]) {
        if !self.configured {
            warn!(output = %self.output.name, "present before configure, dropping frame");
            return;
        }
        if let Some(shm) = &mut self.shm {
            shm.present(&self.wl_surface, pixels);
        }
    }
}

/// Thin wrapper around the `LayerShell` global and the surface map.
pub struct LayerShellState {
    pub(crate) shell: LayerShell,
    pub surfaces: HashMap<String, WallpaperSurface>,
}

impl LayerShellState {
    #[must_use]
    pub fn new(shell: LayerShell) -> Self {
        Self {
            shell,
            surfaces: HashMap::new(),
        }
    }

    pub fn create_surface<D>(
        &mut self,
        wl_output: &WlOutput,
        output_info: OutputInfo,
        wl_surface: WlSurface,
        qh: &QueueHandle<D>,
    ) where
        D: smithay_client_toolkit::shell::wlr_layer::LayerShellHandler
            + 'static
            + wayland_client::Dispatch<
                zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
                smithay_client_toolkit::shell::wlr_layer::LayerSurfaceData,
            >,
    {
        let layer_surface = self.shell.create_layer_surface(
            qh,
            wl_surface.clone(),
            Layer::Background,
            Some("wallpaper"),
            Some(wl_output),
        );

        layer_surface.set_size(0, 0);
        layer_surface.set_anchor(Anchor::all());
        layer_surface.set_exclusive_zone(-1);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);

        // First commit triggers the configure event from the compositor.
        wl_surface.commit();

        info!(output = %output_info.name, "layer surface created, awaiting configure");

        self.surfaces.insert(
            output_info.name.clone(),
            WallpaperSurface {
                layer_surface,
                wl_surface,
                output: output_info,
                configured: false,
                width: 0,
                height: 0,
                shm: None,
            },
        );
    }

    /// Handle a configure event for `layer`.
    pub fn on_configure(
        &mut self,
        layer: &LayerSurface,
        configure: &LayerSurfaceConfigure,
        _serial: u32,
    ) -> Option<String> {
        let surface = self
            .surfaces
            .values_mut()
            .find(|s| &s.layer_surface == layer)?;

        let new_w = if configure.new_size.0 == 0 {
            surface.output.width
        } else {
            configure.new_size.0
        };
        let new_h = if configure.new_size.1 == 0 {
            surface.output.height
        } else {
            configure.new_size.1
        };

        let size_changed = !surface.configured || surface.width != new_w || surface.height != new_h;
        surface.width = new_w;
        surface.height = new_h;
        surface.configured = true;

        if size_changed {
            surface.shm = None;
        }

        // Do NOT ack here — SCTK already did it.
        // Do NOT commit here — the next frame submission will commit.

        info!(
            output = %surface.output.name,
            width = new_w,
            height = new_h,
            "surface configured ✓"
        );

        if size_changed {
            Some(surface.output.name.clone())
        } else {
            None
        }
    }

    pub fn on_closed(&mut self, layer: &LayerSurface) {
        self.surfaces.retain(|name, s| {
            if &s.layer_surface == layer {
                info!(output = %name, "layer surface closed by compositor");
                false
            } else {
                true
            }
        });
    }

    pub fn remove_surface(&mut self, output_name: &str) {
        if self.surfaces.remove(output_name).is_some() {
            info!(output = output_name, "layer surface removed");
        }
    }
}
