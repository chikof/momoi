use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use crate::validate_enum;

/// Main configuration structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralSettings,

    #[serde(default)]
    pub playlist: Option<PlaylistSettings>,

    #[serde(default)]
    pub schedule: Vec<ScheduleEntry>,

    #[serde(default)]
    pub output: Vec<OutputConfig>,

    #[serde(default)]
    pub collection: Vec<Collection>,

    #[serde(default)]
    pub shader_preset: Vec<ShaderPreset>,

    #[serde(default)]
    pub advanced: AdvancedSettings,
}

/// General daemon settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GeneralSettings {
    #[serde(default = "default_log_level")]
    pub log_level: String,

    #[serde(default = "default_transition")]
    pub default_transition: String,

    #[serde(default = "default_duration")]
    pub default_duration: u64,

    #[serde(default = "default_scale")]
    pub default_scale: String,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            default_transition: default_transition(),
            default_duration: default_duration(),
            default_scale: default_scale(),
        }
    }
}

fn default_log_level() -> String {
    "info".to_string()
}
fn default_transition() -> String {
    "fade".to_string()
}
fn default_duration() -> u64 {
    500
}
fn default_scale() -> String {
    "fill".to_string()
}

/// Playlist configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlaylistSettings {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default = "default_interval")]
    pub interval: u64,

    #[serde(default)]
    pub shuffle: bool,

    #[serde(default = "default_transition")]
    pub transition: String,

    #[serde(default = "default_duration")]
    pub transition_duration: u64,

    #[serde(default)]
    pub sources: Vec<String>,

    #[serde(default = "default_extensions")]
    pub extensions: Vec<String>,
}

fn default_interval() -> u64 {
    300
} // 5 minutes
fn default_extensions() -> Vec<String> {
    vec![
        "jpg".to_string(),
        "jpeg".to_string(),
        "png".to_string(),
        "webp".to_string(),
        "gif".to_string(),
        "mp4".to_string(),
        "webm".to_string(),
        "mkv".to_string(),
    ]
}

/// Time-based schedule entry
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScheduleEntry {
    pub name: String,
    pub start_time: String, // Format: "HH:MM"
    pub end_time: String,   // Format: "HH:MM"
    pub wallpaper: String,

    #[serde(default = "default_transition")]
    pub transition: String,

    #[serde(default = "default_duration")]
    pub duration: u64,
}

/// Per-output configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutputConfig {
    pub name: String,

    #[serde(default)]
    pub wallpaper: Option<String>,

    #[serde(default = "default_scale")]
    pub scale: String,

    #[serde(default = "default_transition")]
    pub transition: String,

    #[serde(default = "default_duration")]
    pub duration: u64,

    #[serde(default)]
    pub playlist: bool,

    #[serde(default)]
    pub playlist_sources: Vec<String>,
}

/// Named collection of wallpapers
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Collection {
    pub name: String,

    #[serde(default)]
    pub description: String,

    pub wallpapers: Vec<String>,
}

/// Shader preset configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ShaderPreset {
    /// Preset name
    pub name: String,

    /// Shader type (plasma, waves, matrix, gradient, starfield, raymarching, tunnel)
    pub shader: String,

    /// Description
    #[serde(default)]
    pub description: String,

    /// Animation speed multiplier
    #[serde(default)]
    pub speed: Option<f32>,

    /// Primary color (hex format)
    #[serde(default)]
    pub color1: Option<String>,

    /// Secondary color (hex format)
    #[serde(default)]
    pub color2: Option<String>,

    /// Tertiary color (hex format)
    #[serde(default)]
    pub color3: Option<String>,

    /// Scale parameter
    #[serde(default)]
    pub scale: Option<f32>,

    /// Intensity parameter
    #[serde(default)]
    pub intensity: Option<f32>,

    /// Count parameter
    #[serde(default)]
    pub count: Option<u32>,
}

impl ShaderPreset {
    /// Convert to ShaderParams
    pub fn to_params(&self) -> common::ShaderParams {
        common::ShaderParams {
            speed: self.speed,
            color1: self.color1.clone(),
            color2: self.color2.clone(),
            color3: self.color3.clone(),
            scale: self.scale,
            intensity: self.intensity,
            count: self.count,
        }
    }
}

/// Advanced settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AdvancedSettings {
    #[serde(default = "default_true")]
    pub enable_video: bool,

    #[serde(default = "default_true")]
    pub video_muted: bool,

    #[serde(default = "default_true")]
    pub video_loop: bool,

    #[serde(default = "default_max_fps")]
    pub max_fps: u32,

    #[serde(default)]
    pub cache_limit_mb: u64,

    #[serde(default = "default_true")]
    pub preload_next: bool,

    // Resource management
    #[serde(default = "default_performance_mode")]
    pub performance_mode: String,

    #[serde(default = "default_true")]
    pub auto_battery_mode: bool,

    #[serde(default = "default_true")]
    pub enforce_memory_limits: bool,

    #[serde(default = "default_memory_limit")]
    pub max_memory_mb: usize,

    #[serde(default = "default_cpu_threshold")]
    pub cpu_threshold: f32,
}

impl Default for AdvancedSettings {
    fn default() -> Self {
        Self {
            enable_video: true,
            video_muted: true,
            video_loop: true,
            max_fps: default_max_fps(),
            cache_limit_mb: 500,
            preload_next: true,
            performance_mode: default_performance_mode(),
            auto_battery_mode: true,
            enforce_memory_limits: true,
            max_memory_mb: default_memory_limit(),
            cpu_threshold: default_cpu_threshold(),
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_max_fps() -> u32 {
    60
}
fn default_performance_mode() -> String {
    "balanced".to_string()
}
fn default_memory_limit() -> usize {
    300
}
fn default_cpu_threshold() -> f32 {
    80.0
}

impl Config {
    /// Load configuration from the default location
    pub fn load() -> Result<Self> {
        let config_path = Self::default_config_path()?;
        Self::load_from_path(&config_path)
    }

    /// Load configuration from a specific path
    pub fn load_from_path(path: &Path) -> Result<Self> {
        if !path.exists() {
            log::info!(
                "Config file not found at {}, using defaults",
                path.display()
            );
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        log::info!("Loaded configuration from {}", path.display());
        config.validate()?;

        Ok(config)
    }

    /// Get the default config file path
    pub fn default_config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Failed to get config directory")?
            .join("momoi");

        Ok(config_dir.join("config.toml"))
    }

    /// Validate configuration
    fn validate(&self) -> Result<()> {
        // Validate log level
        match self.general.log_level.as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            _ => anyhow::bail!("Invalid log level: {}", self.general.log_level),
        }

        // Validate transitions
        self.validate_transition(&self.general.default_transition)?;

        if let Some(ref playlist) = self.playlist {
            self.validate_transition(&playlist.transition)?;
        }

        for schedule in &self.schedule {
            self.validate_transition(&schedule.transition)?;
            self.validate_time(&schedule.start_time)?;
            self.validate_time(&schedule.end_time)?;
        }

        for output in &self.output {
            self.validate_transition(&output.transition)?;
            self.validate_scale(&output.scale)?;
        }

        // Validate scale modes
        self.validate_scale(&self.general.default_scale)?;

        Ok(())
    }

    fn validate_transition(&self, transition: &str) -> Result<()> {
        validate_enum!(
            transition,
            "none",
            "fade",
            "wipe-left",
            "wipe-right",
            "wipe-top",
            "wipe-bottom",
            "wipe-angle",
            "center",
            "outer",
            "random"
        )
    }

    fn validate_scale(&self, scale: &str) -> Result<()> {
        validate_enum!(scale, "center", "fill", "fit", "stretch", "tile")
    }

    fn validate_time(&self, time: &str) -> Result<()> {
        let parts: Vec<&str> = time.split(':').collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid time format: {} (expected HH:MM)", time);
        }

        let hour: u32 = parts[0]
            .parse()
            .with_context(|| format!("Invalid hour in time: {}", time))?;
        let minute: u32 = parts[1]
            .parse()
            .with_context(|| format!("Invalid minute in time: {}", time))?;

        if hour >= 24 {
            anyhow::bail!("Invalid hour (must be 0-23): {}", time);
        }
        if minute >= 60 {
            anyhow::bail!("Invalid minute (must be 0-59): {}", time);
        }

        Ok(())
    }

    /// Get output configuration by name
    pub fn get_output_config(&self, output_name: &str) -> Option<&OutputConfig> {
        self.output.iter().find(|o| o.name == output_name)
    }

    /// Get collection by name
    pub fn get_collection(&self, name: &str) -> Option<&Collection> {
        self.collection.iter().find(|c| c.name == name)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralSettings::default(),
            playlist: None,
            schedule: Vec::new(),
            output: Vec::new(),
            collection: Vec::new(),
            shader_preset: Vec::new(),
            advanced: AdvancedSettings::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.general.log_level, "info");
        assert_eq!(config.general.default_transition, "fade");
        assert_eq!(config.general.default_duration, 500);
    }

    #[test]
    fn test_validate_transition() {
        let config = Config::default();
        assert!(config.validate_transition("fade").is_ok());
        assert!(config.validate_transition("wipe-left").is_ok());
        assert!(config.validate_transition("random").is_ok());
        assert!(config.validate_transition("invalid").is_err());
    }

    #[test]
    fn test_validate_time() {
        let config = Config::default();
        assert!(config.validate_time("06:00").is_ok());
        assert!(config.validate_time("23:59").is_ok());
        assert!(config.validate_time("24:00").is_err());
        assert!(config.validate_time("12:60").is_err());
        assert!(config.validate_time("invalid").is_err());
    }

    #[test]
    fn test_shader_preset_to_params() {
        let preset = ShaderPreset {
            name: "test".to_string(),
            shader: "plasma".to_string(),
            description: "Test preset".to_string(),
            speed: Some(2.0),
            color1: Some("FF0000".to_string()),
            color2: Some("00FF00".to_string()),
            color3: Some("0000FF".to_string()),
            scale: Some(1.5),
            intensity: Some(0.8),
            count: Some(100),
        };

        let params = preset.to_params();
        assert_eq!(params.speed, Some(2.0));
        assert_eq!(params.color1, Some("FF0000".to_string()));
        assert_eq!(params.color2, Some("00FF00".to_string()));
        assert_eq!(params.color3, Some("0000FF".to_string()));
        assert_eq!(params.scale, Some(1.5));
        assert_eq!(params.intensity, Some(0.8));
        assert_eq!(params.count, Some(100));
    }

    #[test]
    fn test_shader_preset_partial_params() {
        // Test preset with only some parameters set
        let preset = ShaderPreset {
            name: "minimal".to_string(),
            shader: "starfield".to_string(),
            description: "Minimal preset".to_string(),
            speed: Some(1.5),
            color1: Some("FFFFFF".to_string()),
            color2: None,
            color3: None,
            scale: None,
            intensity: None,
            count: Some(200),
        };

        let params = preset.to_params();
        assert_eq!(params.speed, Some(1.5));
        assert_eq!(params.color1, Some("FFFFFF".to_string()));
        assert!(params.color2.is_none());
        assert!(params.color3.is_none());
        assert!(params.scale.is_none());
        assert!(params.intensity.is_none());
        assert_eq!(params.count, Some(200));
    }

    #[test]
    fn test_shader_preset_empty_params() {
        // Test preset with no parameters (all defaults)
        let preset = ShaderPreset {
            name: "default".to_string(),
            shader: "waves".to_string(),
            description: "Default preset".to_string(),
            speed: None,
            color1: None,
            color2: None,
            color3: None,
            scale: None,
            intensity: None,
            count: None,
        };

        let params = preset.to_params();
        assert!(params.speed.is_none());
        assert!(params.color1.is_none());
        assert!(params.color2.is_none());
        assert!(params.color3.is_none());
        assert!(params.scale.is_none());
        assert!(params.intensity.is_none());
        assert!(params.count.is_none());
    }

    #[test]
    fn test_config_with_presets() {
        // Test parsing config with shader presets
        let toml = r#"
[general]
log_level = "info"

[[shader_preset]]
name = "calm"
shader = "plasma"
description = "Calm plasma"
speed = 0.5
color1 = "1a1a2e"
color2 = "16213e"

[[shader_preset]]
name = "fast"
shader = "starfield"
speed = 3.0
count = 500
"#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.shader_preset.len(), 2);

        let calm = &config.shader_preset[0];
        assert_eq!(calm.name, "calm");
        assert_eq!(calm.shader, "plasma");
        assert_eq!(calm.speed, Some(0.5));
        assert_eq!(calm.color1, Some("1a1a2e".to_string()));

        let fast = &config.shader_preset[1];
        assert_eq!(fast.name, "fast");
        assert_eq!(fast.shader, "starfield");
        assert_eq!(fast.speed, Some(3.0));
        assert_eq!(fast.count, Some(500));
    }
}
