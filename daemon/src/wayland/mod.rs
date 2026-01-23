//! Wayland compositor integration module
//!
//! This module is split into logical submodules for better maintainability:
//! - daemon: Core daemon state and event loop
//! - commands: Wallpaper command handlers (set image, video, shader, etc.)
//! - frame_updates: Frame update logic for videos, GIFs, shaders
//! - overlay: Overlay effect management
//! - transitions: Transition animation handling
//! - outputs: Output/monitor and layer surface management
//! - event_handlers: Wayland protocol event handlers
//! - utils: Helper functions and utilities

mod commands;
mod daemon;
mod event_handlers;
mod frame_updates;
mod outputs;
mod overlay;
mod transitions;
mod utils;

// Re-export the main entry point
pub use daemon::run;

// Re-export types that other modules need
pub(crate) use daemon::{FrameUpdate, OutputData, WallpaperDaemon};
