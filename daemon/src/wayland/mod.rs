//! Wayland compositor integration module
//!
//! This module is split into logical submodules for better maintainability:
//! - daemon: Core daemon orchestration and main entry point
//! - types: Core data structures (WallpaperDaemon, OutputData)
//! - reconnection: Automatic reconnection with exponential backoff
//! - event_loop: Main event loop and periodic task helpers
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
mod event_loop;
mod frame_updates;
mod outputs;
mod overlay;
mod reconnection;
mod transitions;
mod types;
mod utils;

// Re-export the main entry point
pub use daemon::run;

// Re-export types that other modules need
pub(crate) use types::{FrameUpdate, OutputData, WallpaperDaemon};
