//! Monotonic frame timer.

use std::time::{Duration, Instant};

/// Tracks elapsed time and delta between frames.
#[derive(Debug)]
pub struct FrameTimer {
    start: Instant,
    last_frame: Instant,
}

impl FrameTimer {
    /// Create a new timer, starting now.
    #[must_use]
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            start: now,
            last_frame: now,
        }
    }

    /// Advance the timer; returns `(elapsed_secs, delta_secs)`.
    pub fn tick(&mut self) -> (f32, f32) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.start).as_secs_f32();
        let delta = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        (elapsed, delta)
    }

    /// Sleep until the next frame deadline.
    pub fn sleep_until_next_frame(&self, target_fps: u32) {
        let frame_dur = Duration::from_secs_f64(1.0 / f64::from(target_fps));
        let elapsed = self.last_frame.elapsed();
        if let Some(remaining) = frame_dur.checked_sub(elapsed) {
            std::thread::sleep(remaining);
        }
    }
}

impl Default for FrameTimer {
    fn default() -> Self {
        Self::new()
    }
}
