//! Output enumeration and change tracking.

use render_core::OutputInfo;
use smithay_client_toolkit::{
    delegate_output, delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
};
use tracing::{debug, info};
use wayland_client::{Connection, QueueHandle, globals::GlobalList, protocol::wl_output::WlOutput};

/// Manages Wayland output enumeration and change events.
pub struct OutputManager {
    registry_state: RegistryState,
    output_state: OutputState,
    /// Currently known outputs, updated by Wayland compositor events.
    pub outputs: Vec<OutputInfo>,
}

impl OutputManager {
    /// Initialise from an already-enumerated global list.
    ///
    /// `qh` must be a `QueueHandle<OutputManager>`.
    #[must_use]
    pub fn new(globals: &GlobalList, qh: &QueueHandle<Self>) -> Self {
        let registry_state = RegistryState::new(globals);
        let output_state = OutputState::new(globals, qh);
        Self {
            registry_state,
            output_state,
            outputs: Vec::new(),
        }
    }
}

// smithay-client-toolkit's delegate_output! macro generates the three
// Dispatch impls (WlOutput, ZxdgOutputV1, ZxdgOutputManagerV1) that
// OutputState::new demands.
delegate_output!(OutputManager);

delegate_registry!(OutputManager);

impl ProvidesRegistryState for OutputManager {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

impl OutputHandler for OutputManager {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, output: WlOutput) {
        if let Some(info) = self.output_state.info(&output) {
            let current_mode = info.modes.iter().find(|m| m.current);
            if let Some(mode) = current_mode {
                let o = OutputInfo {
                    name: info.name.clone().unwrap_or_else(|| "unknown".into()),
                    width: mode.dimensions.0.cast_unsigned(),
                    height: mode.dimensions.1.cast_unsigned(),
                    refresh_mhz: mode.refresh_rate.cast_unsigned(),
                    scale: f64::from(info.scale_factor),
                };
                info!(output = %o.name, width = o.width, height = o.height, "new output");
                self.outputs.push(o);
            }
        }
    }

    fn update_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, output: WlOutput) {
        // Refresh the cached OutputInfo for this output.
        if let Some(info) = self.output_state.info(&output) {
            let name = info.name.as_deref().unwrap_or("unknown");
            debug!(output = name, "output updated");
            if let Some(mode) = info.modes.iter().find(|m| m.current)
                && let Some(existing) = self.outputs.iter_mut().find(|o| o.name == name)
            {
                existing.width = mode.dimensions.0.cast_unsigned();
                existing.height = mode.dimensions.1.cast_unsigned();
                existing.refresh_mhz = mode.refresh_rate.cast_unsigned();
                existing.scale = f64::from(info.scale_factor);
            }
        }
    }

    fn output_destroyed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, output: WlOutput) {
        if let Some(info) = self.output_state.info(&output) {
            let name = info.name.as_deref().unwrap_or("unknown");
            self.outputs.retain(|o| o.name != name);
            info!(output = name, "output removed");
        }
    }
}
