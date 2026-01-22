//! Common types and utilities for Momoi.
//!
//! This crate defines the shared data structures and IPC protocol used for
//! communication between the daemon (`momoi`) and
//! client (`wwctl`).
//!
//! # IPC Protocol
//!
//! Communication happens over a Unix domain socket using JSON-serialized
//! messages. The client sends [`Command`] variants and receives [`Response`]
//! variants.
//!
//! # Examples
//!
//! ```no_run
//! use common::{Command, Response, ShaderParams};
//!
//! // Create a command to set a shader with parameters
//! let cmd = Command::SetShader {
//!     shader: "plasma".to_string(),
//!     output: None,
//!     transition: None,
//!     params: Some(ShaderParams {
//!         speed: Some(2.0),
//!         color1: Some("FF0000".to_string()),
//!         ..Default::default()
//!     }),
//! };
//!
//! // Serialize for sending over IPC
//! let json = serde_json::to_string(&cmd).unwrap();
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Common error types shared between client and daemon.
///
/// All errors are serializable for transmission over IPC.
#[derive(Error, Debug, Serialize, Deserialize)]
pub enum WallpaperError {
    #[error("IO error: {0}")]
    Io(String),

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("Wayland error: {0}")]
    Wayland(String),

    #[error("Image error: {0}")]
    Image(String),

    #[error("Video error: {0}")]
    Video(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

impl From<std::io::Error> for WallpaperError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

impl From<serde_json::Error> for WallpaperError {
    fn from(e: serde_json::Error) -> Self {
        Self::Ipc(e.to_string())
    }
}

/// Commands sent from client to daemon via IPC.
///
/// Each command represents an action the daemon should perform. Commands are
/// serialized to JSON and sent over a Unix socket.
///
/// # Examples
///
/// ```
/// use common::Command;
///
/// // Set a simple wallpaper
/// let cmd = Command::SetWallpaper {
///     path: "/path/to/image.png".to_string(),
///     output: None,  // Apply to all outputs
///     transition: None,  // Use default transition
///     scale: None,  // Use default scale mode
/// };
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    /// Set wallpaper from an image or video file.
    ///
    /// Supported formats: PNG, JPEG, WebP, GIF, SVG, MP4, WebM
    SetWallpaper {
        /// Path to the wallpaper file (must be absolute)
        path: String,
        /// Target output name (e.g., "DP-1"), or None for all outputs
        output: Option<String>,
        /// Transition effect to use when changing wallpaper
        transition: Option<TransitionType>,
        /// How to scale/fit the image to the output
        scale: Option<ScaleMode>,
    },
    /// Set a solid color background.
    ///
    /// # Format
    /// Color should be in hex format without '#': `"FF0000"` for red
    SetColor {
        /// Hex color code (e.g., "FF0000" for red)
        color: String,
        /// Target output name, or None for all outputs
        output: Option<String>,
    },
    /// Set a procedural shader as wallpaper.
    ///
    /// Available shaders: plasma, waves, gradient, starfield, matrix, raymarching, tunnel
    SetShader {
        /// Name of the shader to use
        shader: String,
        /// Target output name, or None for all outputs
        output: Option<String>,
        /// Transition effect when switching to this shader
        transition: Option<TransitionType>,
        /// Parameters to customize the shader appearance
        params: Option<ShaderParams>,
    },
    /// Set shader overlay effect on current wallpaper.
    ///
    /// Available overlays: vignette, scanlines, film-grain, chromatic, crt, pixelate, tint
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use common::{Command, OverlayParams};
    ///
    /// // Apply vignette with custom strength
    /// let cmd = Command::SetOverlay {
    ///     overlay: "vignette".to_string(),
    ///     params: Some(OverlayParams {
    ///         strength: Some(0.7),
    ///         ..Default::default()
    ///     }),
    ///     output: None,
    /// };
    /// ```
    SetOverlay {
        /// Name of the overlay effect
        overlay: String,
        /// Parameters for the overlay effect
        params: Option<OverlayParams>,
        /// Target output name, or None for all outputs
        output: Option<String>,
    },
    /// Clear shader overlay
    ClearOverlay { output: Option<String> },
    /// Query daemon status
    Query,
    /// Kill the daemon
    Kill,
    /// List available outputs
    ListOutputs,
    /// Ping the daemon
    Ping,
    /// Playlist: Move to next wallpaper
    PlaylistNext,
    /// Playlist: Move to previous wallpaper
    PlaylistPrev,
    /// Playlist: Toggle shuffle mode
    PlaylistToggleShuffle,
    /// Get current resource usage and performance mode
    GetResources,
    /// Set performance mode (performance, balanced, powersave)
    SetPerformanceMode { mode: String },
}

/// Response from daemon to client
#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Ok,
    Error(WallpaperError),
    Status(DaemonStatus),
    Outputs(Vec<OutputInfo>),
    Pong,
    Resources(ResourceStatus),
}

/// Daemon status information
#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub version: String,
    pub uptime_secs: u64,
    pub current_wallpapers: Vec<WallpaperStatus>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WallpaperStatus {
    pub output: String,
    pub wallpaper: WallpaperType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WallpaperType {
    None,
    Color(String),
    Image(String),
    Video(String),
    Shader(String),
}

/// Output (monitor) information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub refresh_rate: Option<u32>,
}

/// Resource usage and performance mode status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceStatus {
    pub performance_mode: String,
    pub memory_mb: u64,
    pub cpu_percent: f32,
    pub on_battery: bool,
    pub battery_percent: Option<u8>,
}

/// Transition effect types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransitionType {
    None,
    Fade {
        duration_ms: u32,
    },
    WipeLeft {
        duration_ms: u32,
    },
    WipeRight {
        duration_ms: u32,
    },
    WipeTop {
        duration_ms: u32,
    },
    WipeBottom {
        duration_ms: u32,
    },
    WipeAngle {
        angle_degrees: f32,
        duration_ms: u32,
    },
    Center {
        duration_ms: u32,
    },
    Outer {
        duration_ms: u32,
    },
    Random {
        duration_ms: u32,
    },
}

impl Default for TransitionType {
    fn default() -> Self {
        Self::Fade { duration_ms: 300 }
    }
}

impl TransitionType {
    pub fn duration_ms(&self) -> u32 {
        match self {
            Self::None => 0,
            Self::Fade { duration_ms }
            | Self::WipeLeft { duration_ms }
            | Self::WipeRight { duration_ms }
            | Self::WipeTop { duration_ms }
            | Self::WipeBottom { duration_ms }
            | Self::WipeAngle { duration_ms, .. }
            | Self::Center { duration_ms }
            | Self::Outer { duration_ms }
            | Self::Random { duration_ms } => *duration_ms,
        }
    }
}

/// Shader parameters for customizing procedural shaders.
///
/// All parameters are optional. If not specified, the shader will use its
/// built-in defaults.
///
/// # Examples
///
/// ```
/// use common::ShaderParams;
///
/// // Create params with custom colors and speed
/// let params = ShaderParams {
///     speed: Some(2.0),
///     color1: Some("FF0000".to_string()),
///     color2: Some("0000FF".to_string()),
///     ..Default::default()
/// };
///
/// // Parse a color to RGB values
/// let (r, g, b) = ShaderParams::parse_color("FF0000").unwrap();
/// assert_eq!(r, 1.0);  // Red component
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShaderParams {
    /// Animation speed multiplier.
    ///
    /// - `1.0` = normal speed
    /// - `2.0` = double speed
    /// - `0.5` = half speed
    pub speed: Option<f32>,
    /// Primary color (hex format like "FF0000")
    pub color1: Option<String>,
    /// Secondary color (hex format like "0000FF")
    pub color2: Option<String>,
    /// Tertiary color (hex format like "00FF00")
    pub color3: Option<String>,
    /// Scale/size parameter (shader-specific meaning)
    pub scale: Option<f32>,
    /// Intensity parameter (0.0-1.0)
    pub intensity: Option<f32>,
    /// Count parameter (e.g., number of objects)
    pub count: Option<u32>,
}

impl ShaderParams {
    /// Create empty params
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse hex color to RGB float values (0.0-1.0)
    pub fn parse_color(hex: &str) -> Option<(f32, f32, f32)> {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return None;
        }

        let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;

        Some((r, g, b))
    }
}

/// Overlay effect parameters for post-processing effects.
///
/// Used to customize overlay effects applied on top of wallpapers.
///
/// # Examples
///
/// ```
/// use common::OverlayParams;
///
/// // Create vignette overlay
/// let params = OverlayParams {
///     strength: Some(0.7),
///     ..Default::default()
/// };
///
/// // Create scanlines overlay
/// let params = OverlayParams {
///     intensity: Some(0.3),
///     line_width: Some(2.0),
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OverlayParams {
    /// General strength parameter (0.0-1.0)
    pub strength: Option<f32>,
    /// Intensity parameter (0.0-1.0)
    pub intensity: Option<f32>,
    /// Line width for scanlines effect
    pub line_width: Option<f32>,
    /// Offset for chromatic aberration (pixels)
    pub offset: Option<f32>,
    /// Curvature for CRT effect (0.0-1.0)
    pub curvature: Option<f32>,
    /// Pixel size for pixelate effect
    pub pixel_size: Option<u32>,
    /// Red component for tint (0.0-1.0)
    pub r: Option<f32>,
    /// Green component for tint (0.0-1.0)
    pub g: Option<f32>,
    /// Blue component for tint (0.0-1.0)
    pub b: Option<f32>,
}

impl OverlayParams {
    /// Create empty overlay params
    pub fn new() -> Self {
        Self::default()
    }

    /// Get tint color as RGB tuple (convenience method)
    pub fn tint_color(&self) -> Option<(f32, f32, f32)> {
        match (self.r, self.g, self.b) {
            (Some(r), Some(g), Some(b)) => Some((r, g, b)),
            _ => None,
        }
    }
}

/// Overlay effect types for post-processing
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OverlayEffect {
    /// Vignette darkening effect
    Vignette,
    /// Horizontal scanlines (CRT-style)
    Scanlines,
    /// Film grain noise
    FilmGrain,
    /// Chromatic aberration (color separation)
    ChromaticAberration,
    /// CRT monitor effect (curved screen + scanlines)
    Crt,
    /// Pixelate/mosaic effect
    Pixelate,
    /// Color tint overlay
    ColorTint,
}

impl OverlayEffect {
    /// Parse overlay effect name from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "vignette" => Some(Self::Vignette),
            "scanlines" => Some(Self::Scanlines),
            "film-grain" | "film_grain" | "filmgrain" => Some(Self::FilmGrain),
            "chromatic" | "chromatic-aberration" | "chromatic_aberration" => {
                Some(Self::ChromaticAberration)
            }
            "crt" => Some(Self::Crt),
            "pixelate" => Some(Self::Pixelate),
            "tint" | "color-tint" | "color_tint" => Some(Self::ColorTint),
            _ => None,
        }
    }

    /// Get the name of the overlay effect
    pub fn name(&self) -> &'static str {
        match self {
            Self::Vignette => "vignette",
            Self::Scanlines => "scanlines",
            Self::FilmGrain => "film-grain",
            Self::ChromaticAberration => "chromatic",
            Self::Crt => "crt",
            Self::Pixelate => "pixelate",
            Self::ColorTint => "tint",
        }
    }
}

/// Image scaling/fitting mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ScaleMode {
    /// Center image without scaling
    Center,
    /// Scale to fill entire output (may crop)
    Fill,
    /// Scale to fit within output (may have letterboxing)
    Fit,
    /// Stretch to fill output (may distort)
    Stretch,
    /// Tile the image
    Tile,
}

impl Default for ScaleMode {
    fn default() -> Self {
        Self::Fill
    }
}

/// IPC socket path helper
pub fn get_socket_path() -> std::path::PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));

    std::path::PathBuf::from(runtime_dir).join("momoi.sock")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_params_default() {
        let params = ShaderParams::default();
        assert!(params.speed.is_none());
        assert!(params.color1.is_none());
        assert!(params.color2.is_none());
        assert!(params.color3.is_none());
        assert!(params.scale.is_none());
        assert!(params.intensity.is_none());
        assert!(params.count.is_none());
    }

    #[test]
    fn test_shader_params_new() {
        let params = ShaderParams::new();
        assert!(params.speed.is_none());
    }

    #[test]
    fn test_parse_color_valid() {
        // Test with # prefix
        let (r, g, b) = ShaderParams::parse_color("#FF0000").unwrap();
        assert!((r - 1.0).abs() < 0.01);
        assert!(g.abs() < 0.01);
        assert!(b.abs() < 0.01);

        // Test without # prefix
        let (r, g, b) = ShaderParams::parse_color("00FF00").unwrap();
        assert!(r.abs() < 0.01);
        assert!((g - 1.0).abs() < 0.01);
        assert!(b.abs() < 0.01);

        // Test blue
        let (r, g, b) = ShaderParams::parse_color("0000FF").unwrap();
        assert!(r.abs() < 0.01);
        assert!(g.abs() < 0.01);
        assert!((b - 1.0).abs() < 0.01);

        // Test white
        let (r, g, b) = ShaderParams::parse_color("FFFFFF").unwrap();
        assert!((r - 1.0).abs() < 0.01);
        assert!((g - 1.0).abs() < 0.01);
        assert!((b - 1.0).abs() < 0.01);

        // Test black
        let (r, g, b) = ShaderParams::parse_color("000000").unwrap();
        assert!(r.abs() < 0.01);
        assert!(g.abs() < 0.01);
        assert!(b.abs() < 0.01);

        // Test gray (128, 128, 128 = 0x808080)
        let (r, g, b) = ShaderParams::parse_color("808080").unwrap();
        assert!((r - 0.502).abs() < 0.01); // 128/255 ≈ 0.502
        assert!((g - 0.502).abs() < 0.01);
        assert!((b - 0.502).abs() < 0.01);
    }

    #[test]
    fn test_parse_color_invalid() {
        // Invalid length
        assert!(ShaderParams::parse_color("FF").is_none());
        assert!(ShaderParams::parse_color("FFFF").is_none());
        assert!(ShaderParams::parse_color("FFFFFFFF").is_none());

        // Invalid hex characters
        assert!(ShaderParams::parse_color("GGGGGG").is_none());
        assert!(ShaderParams::parse_color("ZZZZZZ").is_none());

        // Empty string
        assert!(ShaderParams::parse_color("").is_none());
    }

    #[test]
    fn test_transition_type_duration() {
        assert_eq!(TransitionType::None.duration_ms(), 0);
        assert_eq!(TransitionType::Fade { duration_ms: 500 }.duration_ms(), 500);
        assert_eq!(
            TransitionType::WipeLeft { duration_ms: 1000 }.duration_ms(),
            1000
        );
        assert_eq!(
            TransitionType::WipeRight { duration_ms: 750 }.duration_ms(),
            750
        );
        assert_eq!(
            TransitionType::WipeTop { duration_ms: 600 }.duration_ms(),
            600
        );
        assert_eq!(
            TransitionType::WipeBottom { duration_ms: 800 }.duration_ms(),
            800
        );
        assert_eq!(
            TransitionType::WipeAngle {
                angle_degrees: 45.0,
                duration_ms: 900
            }
            .duration_ms(),
            900
        );
        assert_eq!(
            TransitionType::Center { duration_ms: 400 }.duration_ms(),
            400
        );
        assert_eq!(
            TransitionType::Outer { duration_ms: 1200 }.duration_ms(),
            1200
        );
        assert_eq!(
            TransitionType::Random { duration_ms: 666 }.duration_ms(),
            666
        );
    }

    #[test]
    fn test_transition_type_default() {
        let default = TransitionType::default();
        assert_eq!(default.duration_ms(), 300);
    }

    #[test]
    fn test_scale_mode_default() {
        let default = ScaleMode::default();
        matches!(default, ScaleMode::Fill);
    }

    #[test]
    fn test_command_serialization() {
        // Test SetWallpaper command
        let cmd = Command::SetWallpaper {
            path: "/tmp/test.png".to_string(),
            output: Some("DP-1".to_string()),
            transition: Some(TransitionType::Fade { duration_ms: 500 }),
            scale: Some(ScaleMode::Fill),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: Command = serde_json::from_str(&json).unwrap();
        matches!(deserialized, Command::SetWallpaper { .. });

        // Test SetShader command
        let params = ShaderParams {
            speed: Some(2.0),
            color1: Some("FF0000".to_string()),
            color2: Some("0000FF".to_string()),
            color3: None,
            scale: Some(1.5),
            intensity: Some(0.8),
            count: Some(100),
        };
        let cmd = Command::SetShader {
            shader: "plasma".to_string(),
            output: None,
            transition: None,
            params: Some(params),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: Command = serde_json::from_str(&json).unwrap();
        matches!(deserialized, Command::SetShader { .. });
    }

    #[test]
    fn test_response_serialization() {
        // Test Ok response
        let resp = Response::Ok;
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: Response = serde_json::from_str(&json).unwrap();
        matches!(deserialized, Response::Ok);

        // Test Error response
        let resp = Response::Error(WallpaperError::NotFound("test".to_string()));
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: Response = serde_json::from_str(&json).unwrap();
        matches!(deserialized, Response::Error(_));

        // Test Pong response
        let resp = Response::Pong;
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: Response = serde_json::from_str(&json).unwrap();
        matches!(deserialized, Response::Pong);
    }

    #[test]
    fn test_wallpaper_error_conversion() {
        // Test From<std::io::Error>
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let wall_err: WallpaperError = io_err.into();
        matches!(wall_err, WallpaperError::Io(_));

        // Test From<serde_json::Error>
        let json_err = serde_json::from_str::<Command>("invalid json").unwrap_err();
        let wall_err: WallpaperError = json_err.into();
        matches!(wall_err, WallpaperError::Ipc(_));
    }

    #[test]
    fn test_socket_path() {
        let path = get_socket_path();
        assert!(path.to_str().unwrap().contains("momoi.sock"));
    }
}
