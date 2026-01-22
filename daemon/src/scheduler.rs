use anyhow::{Context, Result};
use chrono::{Local, NaiveTime, Timelike};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::config::ScheduleEntry;

/// Scheduler state for time-based wallpaper switching
#[derive(Debug, Clone)]
pub struct SchedulerState {
    /// Schedule entries
    entries: Vec<ScheduleEntry>,

    /// Last check time
    last_check: Instant,

    /// Check interval (how often to check the schedule)
    check_interval: Duration,

    /// Currently active schedule entry (if any)
    active_entry: Option<String>,
}

impl SchedulerState {
    /// Create a new scheduler from schedule entries
    pub fn new(entries: Vec<ScheduleEntry>) -> Self {
        Self {
            entries,
            last_check: Instant::now(),
            check_interval: Duration::from_secs(60), // Check every minute
            active_entry: None,
        }
    }

    /// Check if it's time to run the scheduler
    pub fn should_check(&self) -> bool {
        self.last_check.elapsed() >= self.check_interval
    }

    /// Check the schedule and return the wallpaper to set (if changed)
    pub fn check(&mut self) -> Option<ScheduledWallpaper> {
        self.last_check = Instant::now();

        let now = Local::now();
        let current_time = now.time();

        // Find the matching schedule entry
        for entry in &self.entries {
            if self.time_in_range(&current_time, &entry.start_time, &entry.end_time) {
                // Check if this is a different entry than the current one
                if self.active_entry.as_ref() != Some(&entry.name) {
                    log::info!(
                        "Schedule activated: '{}' ({} - {})",
                        entry.name,
                        entry.start_time,
                        entry.end_time
                    );

                    self.active_entry = Some(entry.name.clone());

                    return Some(ScheduledWallpaper {
                        path: PathBuf::from(shellexpand::tilde(&entry.wallpaper).to_string()),
                        transition: entry.transition.clone(),
                        duration: entry.duration,
                        schedule_name: entry.name.clone(),
                    });
                }

                // Still within the same schedule entry
                return None;
            }
        }

        // No schedule entry matches, clear active entry
        if self.active_entry.is_some() {
            log::info!("No active schedule entry");
            self.active_entry = None;
        }

        None
    }

    /// Check if current time is within a time range
    fn time_in_range(&self, current: &NaiveTime, start: &str, end: &str) -> bool {
        let start_time = match Self::parse_time(start) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("Failed to parse start time '{}': {}", start, e);
                return false;
            }
        };

        let end_time = match Self::parse_time(end) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("Failed to parse end time '{}': {}", end, e);
                return false;
            }
        };

        // Handle ranges that cross midnight
        if start_time <= end_time {
            // Normal range (e.g., 06:00 - 18:00)
            current >= &start_time && current < &end_time
        } else {
            // Range crosses midnight (e.g., 22:00 - 06:00)
            current >= &start_time || current < &end_time
        }
    }

    /// Parse time string in HH:MM format
    fn parse_time(time_str: &str) -> Result<NaiveTime> {
        let parts: Vec<&str> = time_str.split(':').collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid time format: {} (expected HH:MM)", time_str);
        }

        let hour: u32 = parts[0]
            .parse()
            .with_context(|| format!("Invalid hour in time: {}", time_str))?;
        let minute: u32 = parts[1]
            .parse()
            .with_context(|| format!("Invalid minute in time: {}", time_str))?;

        NaiveTime::from_hms_opt(hour, minute, 0)
            .with_context(|| format!("Invalid time: {}", time_str))
    }

    /// Get the currently active schedule entry name
    pub fn active_entry(&self) -> Option<&str> {
        self.active_entry.as_deref()
    }

    /// Get all schedule entries
    pub fn entries(&self) -> &[ScheduleEntry] {
        &self.entries
    }

    /// Force an immediate check (resets the timer)
    pub fn force_check(&mut self) {
        self.last_check = Instant::now() - self.check_interval;
    }
}

/// Wallpaper from a schedule entry
#[derive(Debug, Clone)]
pub struct ScheduledWallpaper {
    pub path: PathBuf,
    pub transition: String,
    pub duration: u64,
    pub schedule_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_time() {
        assert!(SchedulerState::parse_time("06:00").is_ok());
        assert!(SchedulerState::parse_time("23:59").is_ok());
        assert!(SchedulerState::parse_time("24:00").is_err());
        assert!(SchedulerState::parse_time("12:60").is_err());
        assert!(SchedulerState::parse_time("invalid").is_err());
    }

    #[test]
    fn test_time_in_range() {
        let scheduler = SchedulerState::new(Vec::new());

        // Normal range (morning)
        let current = NaiveTime::from_hms_opt(8, 0, 0).unwrap();
        assert!(scheduler.time_in_range(&current, "06:00", "12:00"));
        assert!(!scheduler.time_in_range(&current, "12:00", "18:00"));

        // Range crossing midnight
        let current = NaiveTime::from_hms_opt(23, 30, 0).unwrap();
        assert!(scheduler.time_in_range(&current, "22:00", "06:00"));

        let current = NaiveTime::from_hms_opt(2, 0, 0).unwrap();
        assert!(scheduler.time_in_range(&current, "22:00", "06:00"));

        let current = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
        assert!(!scheduler.time_in_range(&current, "22:00", "06:00"));
    }

    #[test]
    fn test_should_check() {
        let scheduler = SchedulerState::new(Vec::new());

        // Should not check immediately after creation
        assert!(!scheduler.should_check());
    }
}
