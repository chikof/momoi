//! # ipc-protocol
//!
//! JSON-over-Unix-socket message types shared between the momoi daemon
//! and the `momoi-ctl` CLI tool.
//!
//! ## Socket path
//! `$XDG_RUNTIME_DIR/momoi.sock`  (fallback: `/tmp/momoi-$UID.sock`)
//!
//! ## Wire format
//! Each message is a single line of JSON followed by `\n`.
//! The daemon replies with a single-line JSON [`Response`] followed by `\n`.

use serde::{Deserialize, Serialize};

/// Commands the CLI sends to the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    /// Reload configuration and all shaders from disk.
    Reload,

    /// Switch a specific output to a new wallpaper by name/path.
    SetWallpaper {
        /// Output name (e.g. `DP-1`). `"*"` targets all outputs.
        output: String,
        /// Wallpaper path or registered shader name.
        wallpaper: String,
    },

    /// Report current daemon status.
    Status,

    /// List all connected outputs and their current wallpaper.
    ListOutputs,

    /// Gracefully stop the daemon.
    Quit,
}

/// Daemon response to a [`Command`].
///
/// Uses adjacently-tagged serde representation so that tuple variants
/// (like `Outputs(Vec<…>)`) serialise correctly as JSON arrays.
///
/// Wire examples:
/// ```json
/// {"status":"ok"}
/// {"status":"status","data":{"version":"0.1.0","active_outputs":2,...}}
/// {"status":"outputs","data":[{"name":"DP-2",...}]}
/// {"status":"error","data":{"message":"unknown output"}}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", content = "data", rename_all = "snake_case")]
pub enum Response {
    /// Command succeeded; optional human-readable message.
    Ok {
        /// Optional informational message.
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },

    /// Command failed.
    Error {
        /// Human-readable error description.
        message: String,
    },

    /// Response to [`Command::Status`].
    Status(DaemonStatus),

    /// Response to [`Command::ListOutputs`].
    Outputs(Vec<OutputStatus>),
}

/// Current runtime status of the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStatus {
    /// Daemon version string.
    pub version: String,
    /// Number of active render threads.
    pub active_outputs: usize,
    /// Frames rendered since start (summed across all outputs).
    pub total_frames: u64,
    /// Whether audio capture is active.
    pub audio_active: bool,
}

/// Per-output status entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputStatus {
    /// Output name (e.g. `DP-2`).
    pub name: String,
    /// Resolution as `"WIDTHxHEIGHT"`.
    pub resolution: String,
    /// Current wallpaper identifier.
    pub wallpaper: String,
    /// Frames per second measured over the last second.
    pub fps: f32,
}

/// Canonical socket path.
///
/// Returns `$XDG_RUNTIME_DIR/momoi.sock` or falls back to
/// `/tmp/momoi-<uid>.sock`.
#[must_use]
pub fn socket_path() -> std::path::PathBuf {
    if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
        std::path::PathBuf::from(runtime).join("momoi.sock")
    } else {
        // On Linux /proc/self/status contains "Uid:\t<ruid> ..." but
        // the simplest portable approach is to fall back to a fixed path
        // under /tmp using the process ID as a unique suffix.
        std::path::PathBuf::from(format!("/tmp/momoi-{}.sock", std::process::id()))
    }
}
