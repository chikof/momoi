//! Frame submission types.

use std::time::{Duration, Instant};

/// Data describing a single rendered frame ready for presentation.
#[derive(Debug)]
pub struct Frame {
    /// Raw RGBA/BGRA pixel data.
    pub data: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Monotonic timestamp when this frame was produced.
    pub timestamp: Instant,
}

/// Per-frame performance counters.
#[derive(Debug, Clone, Default)]
pub struct FrameStats {
    /// Frame number since daemon start.
    pub frame_index: u64,
    /// CPU time spent building this frame.
    pub cpu_time: Duration,
    /// GPU time (if measurable by the backend).
    pub gpu_time: Option<Duration>,
}
