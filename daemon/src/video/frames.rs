//! Frame data handling and processing
//!
//! This module manages video frame data, including:
//! - Current frame storage
//! - Frame availability signaling
//! - GPU rendering cache
//! - Profiling timestamps

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Manages video frame data and state
pub struct FrameHandler {
    /// Current frame data (BGRA from GStreamer)
    pub(super) current_frame: Arc<Mutex<Option<Vec<u8>>>>,

    /// Cached rendered frame (for async GPU readback fallback)
    pub(super) cached_frame: Arc<Mutex<Option<Vec<u8>>>>,

    /// Flag indicating a new frame is available
    pub(super) new_frame_available: Arc<AtomicBool>,

    /// When GStreamer delivered the current frame (profiling only)
    #[cfg(feature = "profiling")]
    pub(super) gstreamer_frame_time: Arc<Mutex<Option<Instant>>>,
}

impl FrameHandler {
    /// Create new frame handler
    pub fn new() -> Self {
        Self {
            current_frame: Arc::new(Mutex::new(None)),
            cached_frame: Arc::new(Mutex::new(None)),
            new_frame_available: Arc::new(AtomicBool::new(false)),
            #[cfg(feature = "profiling")]
            gstreamer_frame_time: Arc::new(Mutex::new(None)),
        }
    }

    /// Get clone of current_frame for GStreamer callback
    pub fn current_frame_handle(&self) -> Arc<Mutex<Option<Vec<u8>>>> {
        Arc::clone(&self.current_frame)
    }

    /// Get clone of new_frame_available for GStreamer callback
    pub fn new_frame_flag_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.new_frame_available)
    }

    /// Get clone of gstreamer_frame_time for profiling
    #[cfg(feature = "profiling")]
    pub fn frame_time_handle(&self) -> Arc<Mutex<Option<Instant>>> {
        Arc::clone(&self.gstreamer_frame_time)
    }

    /// Check if a new frame is available
    pub fn has_new_frame(&self) -> bool {
        self.new_frame_available.load(Ordering::Acquire)
    }

    /// Mark frame as consumed
    pub fn consume_frame(&self) {
        self.new_frame_available.store(false, Ordering::Release);
    }

    /// Get current frame data (BGRA format)
    pub fn current_frame_bgra(&self) -> Option<Vec<u8>> {
        self.current_frame.lock().ok()?.clone()
    }

    /// Get or set cached rendered frame
    pub fn get_cached_frame(&self) -> Option<Vec<u8>> {
        self.cached_frame.lock().ok()?.clone()
    }

    /// Update cached rendered frame
    pub fn update_cached_frame(&self, frame: Vec<u8>) {
        if let Ok(mut cache) = self.cached_frame.lock() {
            *cache = Some(frame);
        }
    }

    /// Clear cached frame
    pub fn clear_cached_frame(&self) {
        if let Ok(mut cache) = self.cached_frame.lock() {
            *cache = None;
        }
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
