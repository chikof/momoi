use crate::validate_enum;
use anyhow::{Context, Result};
use forgeconf::forgeconf;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Main configuration structure
#[derive(Debug, Clone, Serialize)]
#[forgeconf]
pub struct Config {
    #[field(name = "general")]
    pub general: GeneralSettings,

    #[field(default = None)]
    pub playlist: Option<PlaylistSettings>,

    #[field(default = vec![])]
    pub schedule: Vec<ScheduleEntry>,

    #[field(default = vec![])]
    pub output: Vec<OutputConfig>,

    #[field(default = vec![])]
    pub collection: Vec<Collection>,

    #[field(default = vec![])]
    pub shader_preset: Vec<ShaderPreset>,

    #[field(name = "advanced")]
    pub advanced: AdvancedSettings,
}

/// General daemon settings
#[derive(Debug, Clone, Serialize)]
#[forgeconf]
pub struct GeneralSettings {
    #[field(default = "info".into())]
    pub log_level: String,

    #[field(default = "fade".into())]
    pub default_transition: String,

    #[field(default = 500)]
    pub default_duration: u64,

    #[field(default = "fill".into())]
    pub default_scale: String,
}

// impl Default for GeneralSettings {
//     fn default() -> Self {
//         Self {
//             log_level: default_log_level(),
//             default_transition: default_transition(),
//             default_duration: default_duration(),
//             default_scale: default_scale(),
//         }
//     }
// }
//
// fn default_log_level() -> String {
//     "info".to_string()
// }
//
// fn default_transition() -> String {
//     "fade".to_string()
// }
//
// fn default_duration() -> u64 {
//     500
// }
//
// fn default_scale() -> String {
//     "fill".to_string()
// }

/// Playlist configuration
#[derive(Debug, Clone, Serialize)]
#[forgeconf]
pub struct PlaylistSettings {
    #[field(default = false)]
    pub enabled: bool,

    #[field(default = default_interval())]
    pub interval: u64,

    #[field(default = false)]
    pub shuffle: bool,

    #[field(default = "fade".into())]
    pub transition: String,

    #[field(default = 500)]
    pub transition_duration: u64,

    #[field(default = vec![])]
    pub sources: Vec<String>,

    #[field(default = default_extensions())]
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
#[forgeconf]
#[derive(Debug, Clone, Serialize)]
pub struct ScheduleEntry {
    pub name: String,
    pub start_time: String, // Format: "HH:MM"
    pub end_time: String,   // Format: "HH:MM"
    pub wallpaper: String,

    #[field(name = "transition")]
    pub transition: String,

    #[field(name = "duration")]
    pub duration: u64,
}

/// Per-output configuration
#[forgeconf]
#[derive(Debug, Clone, Serialize)]
pub struct OutputConfig {
    pub name: String,

    #[field(default = None)]
    pub wallpaper: Option<String>,

    #[field(default = "fill".into())]
    pub scale: String,

    #[field(default = "fade".into())]
    pub transition: String,

    #[field(default = 500)]
    pub duration: u64,

    #[field(default = false)]
    pub playlist: bool,

    #[field(default = vec![])]
    pub playlist_sources: Vec<String>,
}

/// Named collection of wallpapers
#[forgeconf]
#[derive(Debug, Clone, Serialize)]
pub struct Collection {
    pub name: String,

    #[field(default = String::new())]
    pub description: String,

    pub wallpapers: Vec<String>,
}

/// Shader preset configuration
#[forgeconf]
#[derive(Debug, Clone, Serialize)]
pub struct ShaderPreset {
    /// Preset name
    pub name: String,

    /// Shader type (plasma, waves, matrix, gradient, starfield, raymarching, tunnel)
    pub shader: String,

    /// Description
    #[field(default = String::new())]
    pub description: String,

    /// Animation speed multiplier
    #[field(default = None)]
    pub speed: Option<f32>,

    /// Primary color (hex format)
    #[field(default = None)]
    pub color1: Option<String>,

    /// Secondary color (hex format)
    #[field(default = None)]
    pub color2: Option<String>,

    /// Tertiary color (hex format)
    #[field(default = None)]
    pub color3: Option<String>,

    /// Scale parameter
    #[field(default = None)]
    pub scale: Option<f32>,

    /// Intensity parameter
    #[field(default = None)]
    pub intensity: Option<f32>,

    /// Count parameter
    #[field(default = None)]
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
#[forgeconf]
#[derive(Debug, Clone, Serialize)]
pub struct AdvancedSettings {
    #[field(default = true)]
    pub enable_video: bool,

    #[field(default = true)]
    pub video_muted: bool,

    #[field(default = true)]
    pub video_loop: bool,

    #[field(default = 60)]
    pub max_fps: u32,

    #[field(default = 500)]
    pub cache_limit_mb: u64,

    #[field(default = true)]
    pub preload_next: bool,

    // Resource management
    #[field(default = "balanced".into())]
    pub performance_mode: String,

    #[field(default = true)]
    pub auto_battery_mode: bool,

    #[field(default = true)]
    pub enforce_memory_limits: bool,

    #[field(default = 300)]
    pub max_memory_mb: usize,

    #[field(default = 80.0)]
    pub cpu_threshold: f32,

    // Reconnection settings
    #[field(default = false)]
    pub enable_reconnection: bool,

    #[field(default = 10)]
    pub max_reconnection_retries: u32,

    #[field(default = 1_000)]
    pub initial_reconnection_backoff_ms: u64,

    #[field(default = 10_000)]
    pub max_reconnection_backoff_ms: u64,

    #[field(default = default_max_video_fps())]
    pub max_video_fps: u32,
}

pub fn default_max_video_fps() -> u32 {
    30
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
            // Create a default config using forgeconf loader with no sources
            let config = Self::loader()
                .load()
                .context("Failed to create default configuration")?;

            config.validate()?;

            return Ok(config);
        }

        log::info!("Loading configuration from {}", path.display());

        // Use forgeconf's ConfigFile to load from the specified path
        let config_file =
            forgeconf::ConfigFile::new(path.to_str().context("Invalid UTF-8 in path")?);
        let config = Self::loader()
            .add_source(config_file)
            .load()
            .with_context(|| format!("Failed to load config from {}", path.display()))?;

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

    /// Create a default configuration (primarily for testing)
    #[cfg(test)]
    fn default_for_testing() -> Self {
        Self::loader()
            .load()
            .expect("Failed to create default config")
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::loader()
            .load()
            .expect("Failed to create default config")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default_for_testing();
        assert_eq!(config.general.log_level, "info");
        assert_eq!(config.general.default_transition, "fade");
        assert_eq!(config.general.default_duration, 500);
    }

    #[test]
    fn test_validate_transition() {
        let config = Config::default_for_testing();
        assert!(config.validate_transition("fade").is_ok());
        assert!(config.validate_transition("wipe-left").is_ok());
        assert!(config.validate_transition("random").is_ok());
        assert!(config.validate_transition("invalid").is_err());
    }

    #[test]
    fn test_validate_time() {
        let config = Config::default_for_testing();
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
        // Test parsing config with shader presets using forgeconf
        let toml_content = r#"
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

        // Parse using forgeconf's parse_toml
        use forgeconf::parse_toml;
        let node = parse_toml(toml_content).expect("Failed to parse TOML");

        // Convert to Config using FromNode
        use forgeconf::FromNode;
        let config = Config::from_node(&node, "").expect("Failed to convert to Config");

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
