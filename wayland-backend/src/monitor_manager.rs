//! `MonitorManager` — the top-level Wayland event-dispatch state.
//!
//! This module owns the Wayland connection state used by the runtime, including:
//!
//! - registry and global bindings (`wl_registry`, `wl_compositor`, `wl_shm`)
//! - output discovery and tracking
//! - layer-shell surface lifecycle per output
//! - optional `linux-dmabuf` capability detection
//! - CPU presentation via `wl_shm` buffers
//!
//! `Dispatch<WlBuffer, ()>` is intentionally kept as `()` because `ShmBuffer`
//! uses double-buffering and does not rely on explicit `wl_buffer::release`
//! tracking.

use crate::{ShmBuffer, WaylandError, dmabuf::DmabufSession, layer_shell::LayerShellState};
use render_core::OutputInfo;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    globals::GlobalData,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::wlr_layer::{LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    shm::{Shm, ShmHandler},
};
use tracing::{error, info};
use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_buffer, wl_output::WlOutput, wl_shm_pool, wl_surface::WlSurface},
};
use wayland_protocols::wp::linux_dmabuf::zv1::client::zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1;

/// Central Wayland state used as the event-dispatch target for the client queue.
///
/// This owns all protocol state required by the wallpaper runtime:
///
/// - registry/global bindings
/// - output metadata
/// - compositor and shared-memory interfaces
/// - layer-shell surfaces keyed by output name
/// - optional `linux-dmabuf` session for zero-copy GPU paths
///
/// It is passed to the Wayland event queue as the `Dispatch` state object.
pub struct MonitorManager {
    registry_state: RegistryState,
    output_state: OutputState,
    compositor_state: CompositorState,
    shm_state: Shm,
    pub layer_shell: LayerShellState,
    pub outputs: Vec<OutputInfo>,
    pub dmabuf: Option<DmabufSession>,
    pub(crate) conn: Connection,
}

/// A live Wayland session containing both the event queue and its dispatch state.
///
/// `WaylandSession` wraps the [`EventQueue`] and the associated [`MonitorManager`]
/// so callers can drive the Wayland event loop and present frames without needing
/// to manage the lower-level connection pieces separately.
pub struct WaylandSession {
    pub queue: EventQueue<MonitorManager>,
    pub state: MonitorManager,
}

impl WaylandSession {
    /// Connects to the Wayland compositor and initializes all required globals.
    ///
    /// This:
    ///
    /// - connects using `WAYLAND_DISPLAY` / environment defaults
    /// - initializes the registry-backed event queue
    /// - binds `wl_compositor`, `wl_shm`, and `zwlr_layer_shell_v1`
    /// - optionally binds `zwp_linux_dmabuf_v1` if available
    ///
    /// # Errors
    /// - `WaylandError::Connect` if the Wayland connection or registry setup fails
    /// - `WaylandError::GlobalMissing` if a required global cannot be bound
    pub fn connect() -> Result<Self, WaylandError> {
        let conn =
            Connection::connect_to_env().map_err(|e| WaylandError::Connect(e.to_string()))?;
        let (globals, queue) = registry_queue_init::<MonitorManager>(&conn)
            .map_err(|e| WaylandError::Connect(e.to_string()))?;
        let qh = queue.handle();

        let registry_state = RegistryState::new(&globals);
        let output_state = OutputState::new(&globals, &qh);
        let compositor_state = CompositorState::bind(&globals, &qh)
            .map_err(|e| WaylandError::GlobalMissing(format!("wl_compositor: {e}")))?;
        let shm_state = Shm::bind(&globals, &qh)
            .map_err(|e| WaylandError::GlobalMissing(format!("wl_shm: {e}")))?;
        let layer_shell_global = LayerShell::bind(&globals, &qh)
            .map_err(|e| WaylandError::GlobalMissing(format!("zwlr_layer_shell_v1: {e}")))?;

        let dmabuf = globals
            .bind::<ZwpLinuxDmabufV1, MonitorManager, _>(&qh, 3..=4, GlobalData)
            .ok()
            .map(DmabufSession::new);

        if dmabuf.is_some() {
            info!("linux-dmabuf-v1: GPU zero-copy path available");
        } else {
            info!("linux-dmabuf-v1 unavailable — using wl_shm CPU path");
        }

        let state = MonitorManager {
            registry_state,
            output_state,
            compositor_state,
            shm_state,
            layer_shell: LayerShellState::new(layer_shell_global),
            outputs: Vec::new(),
            dmabuf,
            conn,
        };

        Ok(Self { queue, state })
    }

    /// Dispatches any Wayland events that are already buffered locally.
    ///
    /// Unlike [`Self::read_and_dispatch`], this does not poll the Wayland socket
    /// for new data first. It only processes events that have already been read
    /// into the queue.
    ///
    /// # Errors
    /// - `WaylandError::EventLoop` if event dispatch fails
    pub fn dispatch_pending(&mut self) -> Result<(), WaylandError> {
        self.queue
            .dispatch_pending(&mut self.state)
            .map(|_| ())
            .map_err(|e| WaylandError::EventLoop(e.to_string()))
    }

    /// Non-blocking event dispatch: poll the Wayland socket with a zero
    /// timeout, read if data is available, then process buffered events.
    ///
    /// # Errors
    /// - `WaylandError::EventLoop` if dispatching pending events fails
    pub fn read_and_dispatch(&mut self) -> Result<(), WaylandError> {
        use std::os::unix::io::{AsFd, AsRawFd};

        // Get the connection fd from the MonitorManager's stored Connection.
        let raw = self.state.conn.as_fd().as_raw_fd();
        let mut pfd = libc::pollfd {
            fd: raw,
            events: libc::POLLIN,
            revents: 0,
        };
        // pfd is valid; timeout=0 is non-blocking.
        let n = unsafe { libc::poll(&raw mut pfd, 1, 0) };
        if n > 0
            && (pfd.revents & libc::POLLIN) != 0
            && let Some(guard) = self.queue.prepare_read()
        {
            let _ = guard.read();
        }

        let _ = self.queue.flush();

        self.queue
            .dispatch_pending(&mut self.state)
            .map(|_| ())
            .map_err(|e| WaylandError::EventLoop(e.to_string()))
    }

    /// Performs a full Wayland roundtrip.
    ///
    /// This flushes requests to the compositor and blocks until all resulting
    /// events have been processed, making it useful for initial setup and state
    /// synchronization.
    ///
    /// # Errors
    /// - `WaylandError::EventLoop` if the roundtrip fails
    pub fn roundtrip(&mut self) -> Result<(), WaylandError> {
        self.queue
            .roundtrip(&mut self.state)
            .map(|_| ())
            .map_err(|e| WaylandError::EventLoop(e.to_string()))
    }

    /// Allocates a CPU-backed `wl_shm` buffer for a configured output surface.
    ///
    /// The buffer dimensions are taken from the tracked layer-shell surface for
    /// `output_name`. The newly created [`ShmBuffer`] is stored on that surface
    /// and reused for future CPU presentation.
    ///
    /// # Errors
    /// - `WaylandError::ShmAlloc` if the output has no associated surface
    /// - Any `WaylandError` returned by [`ShmBuffer::new`]
    pub fn alloc_shm_for(&mut self, output_name: &str) -> Result<(), WaylandError> {
        let qh = self.queue.handle();

        let (w, h) = {
            let surface = self
                .state
                .layer_shell
                .surfaces
                .get(output_name)
                .ok_or_else(|| {
                    WaylandError::ShmAlloc(format!("no surface for output '{output_name}'"))
                })?;
            (surface.width, surface.height)
        };

        let wl_shm = self.state.shm_state.wl_shm();
        let buf = ShmBuffer::new(wl_shm, &qh, w, h)?;

        let Some(surface) = self.state.layer_shell.surfaces.get_mut(output_name) else {
            return Err(WaylandError::ShmAlloc(format!(
                "surface disappeared for output '{output_name}' during shm allocation"
            )));
        };

        surface.shm = Some(buf);

        info!(
            output = output_name,
            width = w,
            height = h,
            "shm buffer allocated"
        );
        Ok(())
    }

    /// Presents a CPU-rendered RGBA frame to the layer-shell surface for an output.
    ///
    /// If the surface is configured but does not yet have a shared-memory buffer,
    /// this method attempts to allocate one first via [`Self::alloc_shm_for`].
    /// If allocation fails, the frame is dropped and the error is logged.
    ///
    /// `pixels` must contain image data in the format expected by the underlying
    /// [`ShmBuffer`] / surface presentation path.
    pub fn present_cpu(&mut self, output_name: &str, pixels: &[u8]) {
        let needs_alloc = self
            .state
            .layer_shell
            .surfaces
            .get(output_name)
            .is_some_and(|s| s.configured && s.shm.is_none());

        if needs_alloc && let Err(e) = self.alloc_shm_for(output_name) {
            error!(output = output_name, error = %e, "shm alloc failed");
            return;
        }

        if let Some(surface) = self.state.layer_shell.surfaces.get_mut(output_name) {
            surface.present_cpu(pixels);
        }
    }
}

delegate_compositor!(MonitorManager);
delegate_output!(MonitorManager);
delegate_shm!(MonitorManager);
delegate_layer!(MonitorManager);
delegate_registry!(MonitorManager);

impl ProvidesRegistryState for MonitorManager {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

impl CompositorHandler for MonitorManager {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlSurface,
        _: i32,
    ) {
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlSurface,
        _: wayland_client::protocol::wl_output::Transform,
    ) {
    }
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: u32) {}
    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlSurface,
        _: &WlOutput,
    ) {
    }
    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlSurface,
        _: &WlOutput,
    ) {
    }
}

impl ShmHandler for MonitorManager {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm_state
    }
}

impl LayerShellHandler for MonitorManager {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        self.layer_shell.on_closed(layer);
    }
    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        serial: u32,
    ) {
        self.layer_shell.on_configure(layer, &configure, serial);
    }
}

impl OutputHandler for MonitorManager {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, output: WlOutput) {
        let Some(info) = self.output_state.info(&output) else {
            return;
        };
        let Some(mode) = info.modes.iter().find(|m| m.current) else {
            return;
        };

        let output_info = OutputInfo {
            name: info.name.clone().unwrap_or_else(|| "unknown".into()),
            width: mode.dimensions.0.cast_unsigned(),
            height: mode.dimensions.1.cast_unsigned(),
            refresh_mhz: mode.refresh_rate.cast_unsigned(),
            scale: f64::from(info.scale_factor),
        };
        info!(output = %output_info.name, width = output_info.width, "output connected");
        self.outputs.push(output_info.clone());

        let wl_surface = self.compositor_state.create_surface(qh);
        self.layer_shell
            .create_surface(&output, output_info, wl_surface, qh);
    }

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, output: WlOutput) {
        let Some(info) = self.output_state.info(&output) else {
            return;
        };
        let name = info.name.as_deref().unwrap_or("unknown");
        if let Some(existing) = self.outputs.iter_mut().find(|o| o.name == name)
            && let Some(mode) = info.modes.iter().find(|m| m.current)
        {
            existing.width = mode.dimensions.0.cast_unsigned();
            existing.height = mode.dimensions.1.cast_unsigned();
        }
    }

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, output: WlOutput) {
        let Some(info) = self.output_state.info(&output) else {
            return;
        };
        let name = info.name.as_deref().unwrap_or("unknown");
        info!(output = name, "output disconnected");
        self.outputs.retain(|o| o.name != name);
        self.layer_shell.remove_surface(name);
    }
}

// These match what ShmBuffer::new requires: Dispatch<WlShmPool, ()> and
// Dispatch<WlBuffer, ()>.

impl Dispatch<wl_shm_pool::WlShmPool, ()> for MonitorManager {
    fn event(
        _: &mut Self,
        _: &wl_shm_pool::WlShmPool,
        _: wl_shm_pool::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for MonitorManager {
    fn event(
        _: &mut Self,
        _: &wl_buffer::WlBuffer,
        _: wl_buffer::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // wl_buffer::release — double-buffering handles this implicitly.
    }
}

impl Dispatch<ZwpLinuxDmabufV1, GlobalData> for MonitorManager {
    fn event(
        _: &mut Self,
        _: &ZwpLinuxDmabufV1,
        _: wayland_protocols::wp::linux_dmabuf::zv1::client::zwp_linux_dmabuf_v1::Event,
        _: &GlobalData,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
