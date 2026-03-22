//! # wayland-backend
//!
//! TODO: doc

pub mod dmabuf;
pub mod error;
pub mod layer_shell;
pub mod monitor_manager;
pub mod output;
pub mod shm;

pub use dmabuf::DmabufSession;
pub use error::WaylandError;
pub use layer_shell::{LayerShellState, WallpaperSurface};
pub use monitor_manager::{MonitorManager, WaylandSession};
pub use output::OutputManager;
pub use shm::ShmBuffer;
