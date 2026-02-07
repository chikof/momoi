//! Video playback statistics tracking
//!
//! This module handles performance metrics for video playback including:
//! - Frame rates (source and rendered)
//! - Drop rates
//! - GPU cache statistics (when profiling is enabled)

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Tracks video playback statistics
pub struct VideoStats {
    /// Number of frames successfully rendered
    pub(super) frames_rendered: u64,

    /// Number of frames dropped by GStreamer (queue overflow)
    pub(super) frames_dropped: Arc<AtomicU64>,

    /// Detected video FPS from stream metadata
    pub(super) detected_fps: Option<f64>,

    /// Last time stats were logged
    pub(super) last_stats_log: Instant,

    /// GPU texture cache hits (profiling only)
    #[cfg(feature = "profiling")]
    pub(super) gpu_cache_hits: u64,

    /// GPU texture cache misses (profiling only)
    #[cfg(feature = "profiling")]
    pub(super) gpu_cache_misses: u64,
}

impl VideoStats {
    /// Create new statistics tracker
    pub fn new(detected_fps: Option<f64>) -> Self {
        Self {
            frames_rendered: 0,
            frames_dropped: Arc::new(AtomicU64::new(0)),
            detected_fps,
            last_stats_log: Instant::now(),
            #[cfg(feature = "profiling")]
            gpu_cache_hits: 0,
            #[cfg(feature = "profiling")]
            gpu_cache_misses: 0,
        }
    }

    /// Get clone of frames_dropped counter for GStreamer callback
    pub fn frames_dropped_handle(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.frames_dropped)
    }

    /// Increment rendered frame counter
    pub fn increment_rendered(&mut self) {
        self.frames_rendered += 1;
    }

    /// Record GPU cache hit (profiling only)
    #[cfg(feature = "profiling")]
    pub fn record_cache_hit(&mut self) {
        self.gpu_cache_hits += 1;
    }

    /// Record GPU cache miss (profiling only)
    #[cfg(feature = "profiling")]
    pub fn record_cache_miss(&mut self) {
        self.gpu_cache_misses += 1;
    }

    /// Get current drop rate as percentage
    pub fn drop_rate(&self) -> f64 {
        let dropped = self.frames_dropped.load(Ordering::Relaxed);
        let total = self.frames_rendered + dropped;
        if total == 0 {
            0.0
        } else {
            (dropped as f64 / total as f64) * 100.0
        }
    }

    /// Log statistics if interval has elapsed
    pub fn maybe_log_stats(&mut self, interval: Duration) {
        if self.last_stats_log.elapsed() < interval {
            return;
        }

        let dropped = self.frames_dropped.load(Ordering::Relaxed);
        let total = self.frames_rendered + dropped;
        let drop_rate = self.drop_rate();

        log::info!(
            "Video stats ({:.2} fps): {} rendered, {} dropped of {} total ({:.1}% drop rate)",
            self.detected_fps.unwrap_or(0.0),
            self.frames_rendered,
            dropped,
            total,
            drop_rate
        );

        #[cfg(feature = "profiling")]
        {
            let total_accesses = self.gpu_cache_hits + self.gpu_cache_misses;
            let hit_rate = if total_accesses > 0 {
                (self.gpu_cache_hits as f64 / total_accesses as f64) * 100.0
            } else {
                0.0
            };
            log::debug!(
                "GPU texture cache: {} hits, {} misses ({:.1}% hit rate)",
                self.gpu_cache_hits,
                self.gpu_cache_misses,
                hit_rate
            );
        }

        self.last_stats_log = Instant::now();
    }

    /// Reset statistics counters
    #[allow(dead_code)] // Part of public API for stats management
    pub fn reset(&mut self) {
        self.frames_rendered = 0;
        self.frames_dropped.store(0, Ordering::Relaxed);
        #[cfg(feature = "profiling")]
        {
            self.gpu_cache_hits = 0;
            self.gpu_cache_misses = 0;
        }
        self.last_stats_log = Instant::now();
    }
}
