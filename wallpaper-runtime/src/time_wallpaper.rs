//! Time-based wallpaper switcher.
//!
//! Delegates to a `day` or `night` renderer depending on the system clock.
//! Cutover times are configurable; defaults are 07:00 (day) and 20:00 (night).
//!
//! Uses `libc::localtime_r` for correct local-time conversion — already a
//! workspace dependency so no new dep is required.

use render_core::{Frame, FrameStats, RenderError, Renderer, ShaderUniforms, SurfaceDescriptor};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;

/// Configurable day/night cutover times (24-hour clock, local time).
#[derive(Debug, Clone, Copy)]
pub struct TimeConfig {
    /// First hour of day mode (inclusive, 0–23). Default: 7.
    pub day_start_hour: u8,
    /// First hour of night mode (inclusive, 0–23). Default: 20.
    pub night_start_hour: u8,
}

impl Default for TimeConfig {
    fn default() -> Self {
        Self {
            day_start_hour: 7,
            night_start_hour: 20,
        }
    }
}

/// Switches between two renderers based on the system wall clock.
pub struct TimeBasedRenderer {
    day: Box<dyn Renderer>,
    night: Box<dyn Renderer>,
    config: TimeConfig,
    /// Cached active index (0 = day, 1 = night, 255 = uninitialised).
    last_active: u8,
}

impl TimeBasedRenderer {
    /// Create a switcher with the given day and night renderers.
    #[must_use]
    pub fn new(day: Box<dyn Renderer>, night: Box<dyn Renderer>, config: TimeConfig) -> Self {
        Self {
            day,
            night,
            config,
            last_active: 255,
        }
    }

    /// Return the current local hour (0–23) via POSIX `localtime_r`.
    fn current_hour() -> u8 {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs().cast_signed())
            .unwrap_or(0);
        // `secs` is a valid `time_t`; `localtime_r` is thread-safe.
        let mut tm = libc::tm {
            tm_sec: 0,
            tm_min: 0,
            tm_hour: 0,
            tm_mday: 0,
            tm_mon: 0,
            tm_year: 0,
            tm_wday: 0,
            tm_yday: 0,
            tm_isdst: 0,
            tm_gmtoff: 0,
            tm_zone: std::ptr::null(),
        };

        let result = unsafe { libc::localtime_r(&raw const secs, &raw mut tm) };
        if result.is_null() {
            return 0;
        }

        u8::try_from(tm.tm_hour).unwrap_or(0)
    }

    fn is_day(&self) -> bool {
        let h = Self::current_hour();
        h >= self.config.day_start_hour && h < self.config.night_start_hour
    }
}

impl Renderer for TimeBasedRenderer {
    fn init(&mut self, surface: &SurfaceDescriptor) -> Result<(), RenderError> {
        self.day.init(surface)?;
        self.night.init(surface)?;
        Ok(())
    }

    fn render_frame(
        &mut self,
        uniforms: &ShaderUniforms,
        stats: &mut FrameStats,
    ) -> Result<Frame, RenderError> {
        let active = u8::from(!self.is_day());
        if active != self.last_active {
            debug!(
                mode = if active == 0 { "day" } else { "night" },
                "time-based wallpaper transition"
            );
            self.last_active = active;
        }
        if active == 0 {
            self.day.render_frame(uniforms, stats)
        } else {
            self.night.render_frame(uniforms, stats)
        }
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), RenderError> {
        self.day.resize(width, height)?;
        self.night.resize(width, height)?;
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "time-based"
    }
}
