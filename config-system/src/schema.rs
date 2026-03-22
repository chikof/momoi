//! Configuration schema — the TOML data model for momoi.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Top-level daemon configuration (`~/.config/momoi/config.toml`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DaemonConfig {
    /// Target render frames per second (default: 60).
    pub fps: u32,
    /// Prefer GPU rendering over the CPU fallback (default: true).
    pub prefer_gpu: bool,
    /// Per-output wallpaper assignments.
    pub outputs: Vec<OutputConfig>,
    /// Global audio capture settings.
    pub audio: AudioConfig,
    /// Overlay widget configuration.
    pub overlay: OverlayConfig,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            fps: 60,
            prefer_gpu: true,
            outputs: Vec::new(),
            audio: AudioConfig::default(),
            overlay: OverlayConfig::default(),
        }
    }
}

/// Assigns a wallpaper to a specific Wayland output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Output name as reported by the Wayland compositor (e.g. `DP-1`, `HDMI-A-1`).
    /// Use `"*"` to match all outputs.
    pub name: String,
    /// Wallpaper to display on this output.
    pub wallpaper: WallpaperConfig,
}

/// Describes a single wallpaper source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WallpaperConfig {
    /// Display a static image file.
    Image {
        /// Absolute or `~`-prefixed path to the image file.
        path: PathBuf,
    },
    /// Run an animated WGSL or GLSL shader.
    Shader {
        /// Absolute or `~`-prefixed path to the shader source.
        path: PathBuf,
        /// Override the global FPS for this shader only.
        fps: Option<u32>,
    },
    /// Run an audio-reactive shader that receives live FFT data.
    AudioReactive {
        /// Absolute or `~`-prefixed path to the shader source.
        path: PathBuf,
        /// Number of FFT frequency bands (default: 32).
        bands: Option<usize>,
    },
    /// Switch between two wallpapers based on the system clock.
    TimeBased {
        /// Wallpaper shown at night (20:00–07:00 local time).
        night: Box<WallpaperConfig>,
        /// Wallpaper shown during the day (07:00–20:00 local time).
        day: Box<WallpaperConfig>,
    },
}

/// Audio capture settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    /// Enable real-time audio capture.
    pub enabled: bool,
    /// FFT window size in samples; must be a power of two.
    pub fft_size: usize,
    /// `PipeWire` capture target name. `None` selects the system default monitor.
    pub device: Option<String>,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            fft_size: 1024,
            device: None,
        }
    }
}

/// Overlay widget settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OverlayConfig {
    /// Render a digital clock on top of the wallpaper.
    pub clock: bool,
    /// Optional custom text string drawn at a fixed position.
    pub custom_text: Option<String>,
    /// Show live CPU and RAM usage statistics.
    pub system_stats: bool,
}
