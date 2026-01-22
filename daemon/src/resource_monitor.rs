use anyhow::Result;
use std::fs;
use std::time::{Duration, Instant};
use sysinfo::{ProcessesToUpdate, System};

/// Performance mode for resource management
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceMode {
    /// No restrictions, full quality
    Performance,
    /// Moderate throttling, good balance (default)
    Balanced,
    /// Aggressive optimization for battery life
    PowerSave,
}

impl Default for PerformanceMode {
    fn default() -> Self {
        PerformanceMode::Balanced
    }
}

impl PerformanceMode {
    /// Parse from string (case-insensitive)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "performance" => Some(PerformanceMode::Performance),
            "balanced" => Some(PerformanceMode::Balanced),
            "powersave" | "power-save" | "power_save" => Some(PerformanceMode::PowerSave),
            _ => None,
        }
    }

    /// Get video frame rate limit for this mode
    pub fn video_fps_limit(&self) -> u32 {
        match self {
            PerformanceMode::Performance => 60,
            PerformanceMode::Balanced => 30,
            PerformanceMode::PowerSave => 15,
        }
    }

    /// Get GIF frame rate limit for this mode
    pub fn gif_fps_limit(&self) -> u32 {
        match self {
            PerformanceMode::Performance => 50,
            PerformanceMode::Balanced => 30,
            PerformanceMode::PowerSave => 10,
        }
    }

    /// Get memory limit in MB for frame caches
    pub fn memory_limit_mb(&self) -> usize {
        match self {
            PerformanceMode::Performance => 500,
            PerformanceMode::Balanced => 300,
            PerformanceMode::PowerSave => 150,
        }
    }
}

/// Resource usage statistics
#[derive(Debug, Clone)]
pub struct ResourceStats {
    /// Current memory usage in bytes
    pub memory_bytes: u64,
    /// Current CPU usage percentage (0-100)
    pub cpu_percent: f32,
    /// Whether system is on battery power
    pub on_battery: bool,
    /// Battery percentage (0-100), None if not available
    pub battery_percent: Option<u8>,
}

/// Resource monitor configuration
#[derive(Debug, Clone)]
pub struct ResourceConfig {
    /// Enable automatic performance mode switching based on battery
    pub auto_battery_mode: bool,
    /// Enable memory usage limits
    pub enforce_memory_limits: bool,
    /// Maximum memory usage in MB (0 = unlimited)
    pub max_memory_mb: usize,
    /// CPU usage threshold to trigger quality reduction (0-100)
    pub cpu_threshold: f32,
}

impl Default for ResourceConfig {
    fn default() -> Self {
        ResourceConfig {
            auto_battery_mode: true,
            enforce_memory_limits: true,
            max_memory_mb: 300,
            cpu_threshold: 80.0,
        }
    }
}

/// Resource monitor that tracks system resources
pub struct ResourceMonitor {
    /// System information tracker
    system: System,
    /// Current performance mode
    mode: PerformanceMode,
    /// Configuration
    config: ResourceConfig,
    /// Last check time
    last_check: Instant,
    /// Check interval
    check_interval: Duration,
    /// Our process PID
    pid: sysinfo::Pid,
}

impl ResourceMonitor {
    /// Create a new resource monitor
    pub fn new(config: ResourceConfig) -> Self {
        let pid = sysinfo::Pid::from(std::process::id() as usize);

        // Use System::new() instead of new_all() to avoid loading all processes
        ResourceMonitor {
            system: System::new(),
            mode: PerformanceMode::default(),
            config,
            last_check: Instant::now(),
            check_interval: Duration::from_secs(5),
            pid,
        }
    }

    /// Get current performance mode
    pub fn mode(&self) -> PerformanceMode {
        self.mode
    }

    /// Set performance mode manually
    pub fn set_mode(&mut self, mode: PerformanceMode) {
        if self.mode != mode {
            log::info!("Performance mode changed: {:?} -> {:?}", self.mode, mode);
            self.mode = mode;
        }
    }

    /// Check if we should update resource stats
    pub fn should_check(&self) -> bool {
        self.last_check.elapsed() >= self.check_interval
    }

    /// Update resource statistics and adjust performance mode if needed
    pub fn update(&mut self) -> Result<ResourceStats> {
        self.last_check = Instant::now();

        // Refresh ONLY our process stats to avoid file descriptor leak
        // refresh_all() opens /proc/*/stat for every process on the system
        self.system
            .refresh_processes(ProcessesToUpdate::Some(&[self.pid]), false);

        // Get our process stats
        let memory_bytes = self
            .system
            .process(self.pid)
            .map(|p| p.memory())
            .unwrap_or(0);

        let cpu_percent = self
            .system
            .process(self.pid)
            .map(|p| p.cpu_usage())
            .unwrap_or(0.0);

        // Check battery status
        let (on_battery, battery_percent) = self.check_battery();

        // Auto-adjust performance mode based on battery
        if self.config.auto_battery_mode {
            let new_mode = if on_battery {
                if battery_percent.unwrap_or(100) < 20 {
                    PerformanceMode::PowerSave
                } else {
                    PerformanceMode::Balanced
                }
            } else {
                PerformanceMode::Performance
            };

            if new_mode != self.mode {
                log::info!(
                    "Auto-switching performance mode: {:?} -> {:?} (battery: {}%, on_battery: {})",
                    self.mode,
                    new_mode,
                    battery_percent.unwrap_or(0),
                    on_battery
                );
                self.mode = new_mode;
            }
        }

        let stats = ResourceStats {
            memory_bytes,
            cpu_percent,
            on_battery,
            battery_percent,
        };

        log::debug!(
            "Resource stats: mem={}MB cpu={:.1}% battery={}%{}",
            memory_bytes / 1024 / 1024,
            cpu_percent,
            battery_percent.unwrap_or(0),
            if on_battery { " (on battery)" } else { "" }
        );

        Ok(stats)
    }

    /// Check battery status via upower
    fn check_battery(&self) -> (bool, Option<u8>) {
        // Try to read battery status from /sys/class/power_supply
        let battery_path = "/sys/class/power_supply/BAT0";

        // Check if on battery
        let on_battery = fs::read_to_string(format!("{}/status", battery_path))
            .ok()
            .map(|s| s.trim().to_lowercase() == "discharging")
            .unwrap_or(false);

        // Get battery percentage
        let battery_percent = fs::read_to_string(format!("{}/capacity", battery_path))
            .ok()
            .and_then(|s| s.trim().parse::<u8>().ok());

        (on_battery, battery_percent)
    }

    /// Check if we're over memory limit
    pub fn is_over_memory_limit(&self, current_bytes: u64) -> bool {
        if !self.config.enforce_memory_limits || self.config.max_memory_mb == 0 {
            return false;
        }

        let limit_bytes = (self.config.max_memory_mb as u64) * 1024 * 1024;
        current_bytes > limit_bytes
    }

    /// Get current memory limit for the active performance mode
    pub fn current_memory_limit_mb(&self) -> usize {
        if self.config.max_memory_mb > 0 {
            self.config.max_memory_mb.min(self.mode.memory_limit_mb())
        } else {
            self.mode.memory_limit_mb()
        }
    }
}
