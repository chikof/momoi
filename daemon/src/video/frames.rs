//! Frame data handling and processing
//!
//! This module manages video frame data, including:
//! - Current frame storage (lock-free via ArcSwap)
//! - Frame availability signaling (generation counter)
//! - GPU rendering cache
//! - Profiling timestamps

use arc_swap::ArcSwap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "profiling")]
use std::sync::Mutex;
#[cfg(feature = "profiling")]
use std::time::Instant;

/// Manages video frame data and state
///
/// Uses lock-free `ArcSwap` for frame passing between GStreamer callback thread
/// and render thread, eliminating mutex contention and per-frame clones.
pub struct FrameHandler {
    /// Current frame data (BGRA from GStreamer) - lock-free swap
    pub(super) current_frame: Arc<ArcSwap<Vec<u8>>>,

    /// Monotonically increasing generation counter - replaces bool flag
    /// Incremented each time a new frame is stored. Readers compare their
    /// last-seen generation to detect new frames atomically.
    pub(super) frame_generation: Arc<AtomicU64>,

    /// Cached rendered frame (for async GPU readback fallback) - lock-free swap
    cached_frame: ArcSwap<Vec<u8>>,

    /// When GStreamer delivered the current frame (profiling only)
    #[cfg(feature = "profiling")]
    pub(super) gstreamer_frame_time: Arc<Mutex<Option<Instant>>>,
}

impl FrameHandler {
    /// Create new frame handler
    pub fn new() -> Self {
        Self {
            current_frame: Arc::new(ArcSwap::from_pointee(Vec::new())),
            frame_generation: Arc::new(AtomicU64::new(0)),
            cached_frame: ArcSwap::from_pointee(Vec::new()),
            #[cfg(feature = "profiling")]
            gstreamer_frame_time: Arc::new(Mutex::new(None)),
        }
    }

    /// Get handle to current_frame ArcSwap for GStreamer callback
    pub fn current_frame_handle(&self) -> Arc<ArcSwap<Vec<u8>>> {
        Arc::clone(&self.current_frame)
    }

    /// Get handle to generation counter for GStreamer callback
    pub fn generation_handle(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.frame_generation)
    }

    /// Get clone of gstreamer_frame_time for profiling
    #[cfg(feature = "profiling")]
    pub fn frame_time_handle(&self) -> Arc<Mutex<Option<Instant>>> {
        Arc::clone(&self.gstreamer_frame_time)
    }

    /// Get the current frame generation number
    pub fn generation(&self) -> u64 {
        self.frame_generation.load(Ordering::Acquire)
    }

    /// Get current frame data (BGRA format) - lock-free, no clone of underlying data
    ///
    /// Returns an `Arc<Vec<u8>>` so the caller shares ownership without copying.
    /// The Arc is cheap to clone (just a ref count bump).
    pub fn current_frame_bgra(&self) -> Option<Arc<Vec<u8>>> {
        let frame = self.current_frame.load();
        if frame.is_empty() {
            None
        } else {
            Some(Arc::clone(&frame))
        }
    }

    /// Get cached rendered frame - lock-free, no clone
    pub fn get_cached_frame(&self) -> Option<Arc<Vec<u8>>> {
        let cached = self.cached_frame.load();
        if cached.is_empty() {
            None
        } else {
            Some(Arc::clone(&cached))
        }
    }

    /// Update cached rendered frame (accepts Arc to avoid cloning)
    pub fn update_cached_frame(&self, frame: Arc<Vec<u8>>) {
        self.cached_frame.store(frame);
    }

    /// Clear cached frame
    pub fn clear_cached_frame(&self) {
        self.cached_frame.store(Arc::new(Vec::new()));
    }

    /// Get profiling timestamp for current frame
    #[cfg(feature = "profiling")]
    pub fn get_frame_timestamp(&self) -> Option<Instant> {
        self.gstreamer_frame_time.lock().ok()?.clone()
    }
}

impl Default for FrameHandler {
    fn default() -> Self {
        Self::new()
    }
}
