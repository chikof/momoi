use smithay_client_toolkit::{
    compositor::CompositorHandler,
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::ProvidesRegistryState,
    registry_handlers,
    shell::{
        WaylandSurface,
        wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    },
    shm::{Shm, ShmHandler},
};
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    protocol::{wl_buffer, wl_output, wl_shm_pool, wl_surface},
};

use super::WallpaperDaemon;

impl CompositorHandler for WallpaperDaemon {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
        // Handle scale factor changes
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
        // Handle transform changes
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        // Handle frame callbacks for animations
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for WallpaperDaemon {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        log::info!("New output detected");
        if let Err(e) = super::outputs::create_layer_surface(self, output, qh) {
            log::error!("Failed to create layer surface: {}", e);
        }
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if let Some(info) = self.output_state.info(&output) {
            log::info!(
                "Output updated: {:?} - {}x{} @ {}",
                info.name,
                info.logical_size.map(|(w, _)| w).unwrap_or(0),
                info.logical_size.map(|(_, h)| h).unwrap_or(0),
                info.scale_factor,
            );

            // Update our shared state with output info
            if let Ok(mut state) = self.state.try_lock() {
                let output_info = common::OutputInfo {
                    name: info.name.clone().unwrap_or_else(|| "Unknown".to_string()),
                    width: info.logical_size.map(|(w, _)| w as u32).unwrap_or(0),
                    height: info.logical_size.map(|(_, h)| h as u32).unwrap_or(0),
                    scale: info.scale_factor as f64,
                    refresh_rate: None,
                };

                // Update or add output info
                if let Some(existing) = state
                    .outputs
                    .iter_mut()
                    .find(|o| o.name == output_info.name)
                {
                    *existing = output_info;
                    log::debug!("Updated existing output in shared state: {}", existing.name);
                } else {
                    log::info!("Added new output to shared state: {}", output_info.name);
                    state.outputs.push(output_info);
                }
            } else {
                log::warn!("Could not acquire state lock to update output info");
            }
        } else {
            log::warn!("Output updated but no info available");
        }
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        log::info!("Output destroyed");
        self.outputs.retain(|o| o.output != output);
    }
}

impl LayerShellHandler for WallpaperDaemon {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        log::info!("Layer surface closed");
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let (width, height) = configure.new_size;
        log::info!("Layer surface configured: {}x{}", width, height);

        // Find the output data for this layer surface
        let output_data = self
            .outputs
            .iter_mut()
            .find(|o| o.layer_surface.as_ref() == Some(layer));

        if let Some(output_data) = output_data {
            output_data.width = width;
            output_data.height = height;
            output_data.configured = true;

            // Assign shared GPU renderer to this output if available
            #[cfg(feature = "gpu")]
            if output_data.gpu_renderer.is_none() {
                output_data.gpu_renderer = self.gpu_renderer.clone();
            }

            // Create a buffer and render a default dark gray color
            if width > 0 && height > 0 {
                match crate::buffer::ShmBuffer::new(self.shm.wl_shm(), width, height, qh) {
                    Ok(mut buffer) => {
                        // Fill with dark gray (#1e1e1e)
                        buffer.fill_color(0x1e, 0x1e, 0x1e, 0xff);

                        // Attach buffer and commit
                        layer.wl_surface().attach(Some(buffer.buffer()), 0, 0);
                        layer.wl_surface().commit();

                        // Mark buffer as busy (compositor is using it)
                        buffer.mark_busy();

                        // Swap buffer (moves old buffer to pool)
                        output_data.swap_buffer(buffer);
                        output_data.cleanup_buffer_pool();

                        log::info!("Rendered default color to output");
                    }
                    Err(e) => {
                        log::error!("Failed to create buffer: {}", e);
                        layer.wl_surface().commit();
                    }
                }
            } else {
                layer.wl_surface().commit();
            }
        }
    }
}

impl ProvidesRegistryState for WallpaperDaemon {
    fn registry(&mut self) -> &mut smithay_client_toolkit::registry::RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState];
}

impl ShmHandler for WallpaperDaemon {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

// Implement Dispatch for wl_buffer to handle release events
impl Dispatch<wl_buffer::WlBuffer, std::sync::Arc<std::sync::Mutex<crate::buffer::BufferState>>> for WallpaperDaemon {
    fn event(
        _state: &mut Self,
        _proxy: &wl_buffer::WlBuffer,
        event: <wl_buffer::WlBuffer as wayland_client::Proxy>::Event,
        data: &std::sync::Arc<std::sync::Mutex<crate::buffer::BufferState>>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wayland_client::protocol::wl_buffer::Event::Release => {
                // Compositor is done with this buffer, mark it as available for reuse
                if let Ok(mut state) = data.lock() {
                    state.busy = false;
                    log::debug!("Buffer released by compositor");
                }
            }
            _ => {}
        }
    }
}

// Implement Dispatch for wl_shm_pool (no-op, we don't handle pool events)
impl Dispatch<wl_shm_pool::WlShmPool, ()> for WallpaperDaemon {
    fn event(
        _state: &mut Self,
        _proxy: &wl_shm_pool::WlShmPool,
        _event: <wl_shm_pool::WlShmPool as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        // No pool events to handle
    }
}

delegate_compositor!(WallpaperDaemon);
delegate_output!(WallpaperDaemon);
delegate_layer!(WallpaperDaemon);
delegate_shm!(WallpaperDaemon);
delegate_registry!(WallpaperDaemon);
