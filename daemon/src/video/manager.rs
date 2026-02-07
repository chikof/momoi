//! Main VideoManager coordinating video playback
//!
//! This module integrates pipeline setup, frame handling, and statistics
//! to provide a high-level interface for video wallpaper playback.

use super::{frames::FrameHandler, pipeline, stats::VideoStats};
use anyhow::{Context, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

/// Manages video playback for wallpapers
pub struct VideoManager {
    /// GStreamer pipeline
    pipeline: gst::Pipeline,

    /// App sink to receive video frames
    app_sink: gst_app::AppSink,

    /// Frame data handler
    frames: FrameHandler,

    /// Statistics tracker
    stats: VideoStats,

    /// Last frame update time
    last_frame_time: Instant,

    /// Last render time for FPS limiting
    last_render_time: Instant,

    /// Target framerate (for frame timing)
    frame_duration: Duration,

    /// Video decode dimensions
    width: u32,

    /// Video decode dimensions
    height: u32,

    /// Target FPS limit (from config)
    #[allow(dead_code)]
    target_fps: u32,

    /// Whether video is playing
    is_playing: bool,

    /// Loop the video
    should_loop: bool,

    /// Optional GPU renderer for hardware-accelerated video display
    #[cfg(feature = "gpu")]
    gpu_renderer: Option<std::sync::Arc<crate::gpu::GpuRenderer>>,
}

impl VideoManager {
    /// Load a video file and prepare for playback
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the video file
    /// * `target_width` - Decode width (typically max output resolution)
    /// * `target_height` - Decode height
    /// * `scale_mode` - Scaling mode (currently unused, GPU handles scaling)
    /// * `muted` - Whether to mute audio (currently unused)
    /// * `target_fps` - Target FPS limit from configuration
    /// * `gpu_renderer` - Optional GPU renderer for hardware acceleration
    pub fn load(
        path: impl AsRef<Path>,
        target_width: u32,
        target_height: u32,
        _scale_mode: common::ScaleMode,
        _muted: bool,
        target_fps: u32,
        #[cfg(feature = "gpu")] gpu_renderer: Option<std::sync::Arc<crate::gpu::GpuRenderer>>,
    ) -> Result<Self> {
        // Initialize GStreamer
        pipeline::initialize_gstreamer();

        let path = path.as_ref();
        log::info!("Loading video: {}", path.display());

        // Build pipeline
        let (pipeline, app_sink) = pipeline::build_pipeline(path, target_width, target_height)?;

        // Configure appsink
        pipeline::configure_app_sink(&app_sink);

        // Create frame handler and stats
        let frames = FrameHandler::new();
        let detected_fps = pipeline::detect_fps(&pipeline);
        let stats = VideoStats::new(detected_fps);

        // Setup frame callback with proper handles
        pipeline::setup_frame_callback(
            &app_sink,
            frames.current_frame_handle(),
            frames.new_frame_flag_handle(),
            stats.frames_dropped_handle(),
            #[cfg(feature = "profiling")]
            frames.frame_time_handle(),
        );

        // Assume 30 FPS by default (will be updated when we detect actual FPS)
        let frame_duration = Duration::from_millis(33);

        #[cfg(feature = "gpu")]
        {
            if gpu_renderer.is_some() {
                log::info!("VideoManager: GPU rendering ENABLED for video playback");
            } else {
                log::info!("VideoManager: GPU rendering NOT available, using CPU path");
            }
        }

        #[cfg(not(feature = "gpu"))]
        log::info!("VideoManager: GPU feature not compiled, using CPU path");

        log::info!(
            "VideoManager: Target FPS set to {} (max_video_fps config)",
            target_fps
        );

        Ok(Self {
            pipeline,
            app_sink,
            frames,
            stats,
            last_frame_time: Instant::now(),
            last_render_time: Instant::now(),
            frame_duration,
            width: target_width,
            height: target_height,
            is_playing: false,
            should_loop: true,
            target_fps,
            #[cfg(feature = "gpu")]
            gpu_renderer,
        })
    }

    /// Start video playback
    pub fn play(&mut self) -> Result<()> {
        log::info!("Starting video playback");
        self.pipeline
            .set_state(gst::State::Playing)
            .context("Failed to set pipeline to Playing state")?;
        self.is_playing = true;
        Ok(())
    }

    /// Pause video playback
    #[allow(dead_code)] // Part of public API for video control
    pub fn pause(&mut self) -> Result<()> {
        log::info!("Pausing video playback");
        self.pipeline
            .set_state(gst::State::Paused)
            .context("Failed to set pipeline to Paused state")?;
        self.is_playing = false;
        Ok(())
    }

    /// Get raw BGRA frame data from GStreamer (for shared VideoManager usage)
    /// Returns decoded video frame at the VideoManager's resolution
    /// Outputs can then scale/convert this to their specific resolutions
    #[allow(dead_code)] // Alternative API for raw frame access
    pub fn current_frame_bgra(&self) -> Option<Vec<u8>> {
        self.frames.current_frame_bgra()
    }

    /// Get the decode resolution (width, height) of this VideoManager
    #[allow(dead_code)] // Part of public API for resolution queries
    pub fn resolution(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Get the current frame data as ARGB for Wayland
    /// If GPU rendering is enabled, uses async GPU path with frame caching
    #[allow(dead_code)] // Alternative API, current_frame_data_scaled is the primary method
    pub fn current_frame_data(&mut self) -> Option<Vec<u8>> {
        self.current_frame_data_scaled(self.width, self.height)
    }

    /// Get current frame data scaled to target resolution (for multi-resolution support)
    /// This allows a single VideoManager (decode at max res) to serve multiple outputs at different resolutions
    pub fn current_frame_data_scaled(
        &mut self,
        target_width: u32,
        target_height: u32,
    ) -> Option<Vec<u8>> {
        #[cfg(feature = "profiling")]
        let render_request_time = Instant::now();

        let bgra_data = self.frames.current_frame_bgra()?;

        // Get GStreamer frame delivery time for profiling
        #[cfg(feature = "profiling")]
        let gstreamer_delivery_latency = {
            self.frames
                .get_frame_timestamp()
                .map(|t| render_request_time.duration_since(t).as_secs_f64() * 1000.0)
        };

        // Try GPU rendering path first if available
        #[cfg(feature = "gpu")]
        {
            if let Some(ref gpu) = self.gpu_renderer {
                #[cfg(feature = "profiling")]
                let gpu_start = Instant::now();

                // Use futures::executor::block_on since we're in a blocking context
                // Pass source (decode) and target (output) resolutions for GPU scaling
                let result = futures::executor::block_on(async {
                    gpu.render_video_frame_bgra(
                        &bgra_data,
                        self.width,
                        self.height,
                        target_width,
                        target_height,
                    )
                    .await
                });

                match result {
                    Ok(Some(argb_data)) => {
                        // GPU readback completed successfully
                        #[cfg(feature = "profiling")]
                        {
                            let gpu_elapsed_ms = gpu_start.elapsed().as_secs_f64() * 1000.0;
                            self.stats.record_cache_miss();

                            // Detailed profiling every 60 frames
                            if self.stats.frames_rendered.is_multiple_of(60) {
                                let scale_info =
                                    if self.width == target_width && self.height == target_height {
                                        "no scaling".to_string()
                                    } else {
                                        format!(
                                            "scaled {}x{} -> {}x{}",
                                            self.width, self.height, target_width, target_height
                                        )
                                    };

                                log::info!(
                                    "[PROFILE] Frame {}: GStreamer→Render={:.2}ms, GPU={:.2}ms ({}), Total={:.2}ms",
                                    self.stats.frames_rendered,
                                    gstreamer_delivery_latency.unwrap_or(0.0),
                                    gpu_elapsed_ms,
                                    scale_info,
                                    render_request_time.elapsed().as_secs_f64() * 1000.0
                                );
                            }
                        }

                        // Update cache occasionally as fallback
                        if self.stats.frames_rendered.is_multiple_of(30) {
                            self.frames.update_cached_frame(argb_data.clone());
                            log::debug!(
                                "Updated cached_frame (frame {}), size={}MB",
                                self.stats.frames_rendered,
                                argb_data.len() / 1024 / 1024
                            );
                        }

                        return Some(argb_data);
                    }

                    Ok(None) => {
                        // GPU not ready yet - return cached frame
                        #[cfg(feature = "profiling")]
                        {
                            self.stats.record_cache_hit();
                            if self.stats.frames_rendered.is_multiple_of(60) {
                                log::info!(
                                    "[PROFILE] Frame {}: GPU not ready, using cache",
                                    self.stats.frames_rendered
                                );
                            }
                        }

                        if let Some(cached) = self.frames.get_cached_frame() {
                            return Some(cached);
                        }

                        // No cached frame available, fall through to CPU path
                        log::warn!("No cached frame available, falling back to CPU");
                    }

                    Err(e) => {
                        log::warn!("GPU video rendering failed: {}, falling back to CPU", e);
                        // Fall through to CPU path
                    }
                }
            } else {
                log::trace!("No GPU renderer available, using CPU path");
            }
        }

        // CPU path: BGRA from GStreamer is already in correct format for Wayland
        log::trace!("Using CPU video rendering path");
        Some(bgra_data)
    }

    /// Check if a new frame is available and should be displayed
    /// Returns true if there's a new frame to render
    pub fn update(&mut self) -> bool {
        if !self.is_playing {
            return false;
        }

        // Detect FPS from pipeline caps (do this once on first frame)
        if self.stats.detected_fps.is_none()
            && self.stats.frames_rendered == 0
            && let Some(pad) = self.app_sink.static_pad("sink")
            && let Some(caps) = pad.current_caps()
            && let Some(structure) = caps.structure(0)
            && let Ok(framerate) = structure.get::<gst::Fraction>("framerate")
        {
            let fps = framerate.numer() as f64 / framerate.denom() as f64;
            self.stats.detected_fps = Some(fps);
            self.frame_duration = Duration::from_secs_f64(1.0 / fps);

            log::info!(
                "Detected video FPS: {:.2} ({}/{}), frame duration: {:.1}ms",
                fps,
                framerate.numer(),
                framerate.denom(),
                self.frame_duration.as_secs_f64() * 1000.0
            );
        }

        // Check for EOS (end of stream) for looping
        // Drain ALL messages from the bus to prevent memory leak
        if let Some(bus) = self.pipeline.bus() {
            while let Some(msg) = bus.pop() {
                match msg.view() {
                    gst::MessageView::Eos(_) => {
                        if self.should_loop {
                            log::debug!("Video reached EOS, looping...");
                            let _ = self.pipeline.seek_simple(
                                gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                                gst::ClockTime::from_seconds(0),
                            );
                        } else {
                            log::info!("Video playback finished");
                            self.is_playing = false;
                        }
                    }

                    gst::MessageView::Error(err) => {
                        log::error!(
                            "GStreamer error: {} (debug: {:?})",
                            err.error(),
                            err.debug()
                        );
                        self.is_playing = false;
                    }

                    _ => {
                        // Drain other messages to prevent memory leak
                    }
                }
            }
        }

        // Check if we have a new frame available using atomic flag
        if self.frames.has_new_frame() {
            self.frames.consume_frame();

            // Respect video's native frame rate - don't display faster than source FPS
            let min_frame_time = self.frame_duration;
            let elapsed_since_last_render = self.last_render_time.elapsed();

            // Add 2ms tolerance to avoid rejecting frames that are "close enough"
            let tolerance = Duration::from_millis(2);
            let min_frame_time_with_tolerance = min_frame_time.saturating_sub(tolerance);

            if elapsed_since_last_render < min_frame_time_with_tolerance {
                // Too soon - skip this frame
                #[cfg(feature = "profiling")]
                if self.stats.frames_rendered.is_multiple_of(60) {
                    log::info!(
                        "[PROFILE] Frame pacing: rejected (elapsed={:.2}ms < min={:.2}ms, tolerance={:.2}ms)",
                        elapsed_since_last_render.as_secs_f64() * 1000.0,
                        min_frame_time_with_tolerance.as_secs_f64() * 1000.0,
                        tolerance.as_secs_f64() * 1000.0
                    );
                }

                return false;
            }

            // Calculate actual time since last frame for jitter detection
            let actual_frame_time = self.last_frame_time.elapsed();
            self.last_frame_time = Instant::now();
            self.last_render_time = Instant::now();
            self.stats.increment_rendered();

            // Log frame timing for jitter analysis
            if self.stats.frames_rendered.is_multiple_of(10) {
                let expected_ms = self.frame_duration.as_secs_f64() * 1000.0;
                let actual_ms = actual_frame_time.as_secs_f64() * 1000.0;
                let jitter_ms = (actual_ms - expected_ms).abs();

                log::debug!(
                    "Frame #{}: expected={:.1}ms, actual={:.1}ms, jitter={:.1}ms",
                    self.stats.frames_rendered,
                    expected_ms,
                    actual_ms,
                    jitter_ms
                );
            }

            // Log statistics periodically
            self.stats.maybe_log_stats(Duration::from_secs(3));

            true
        } else {
            false
        }
    }

    /// Get video dimensions
    #[allow(dead_code)] // Part of public API for dimension queries
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Get the frame duration for adaptive polling
    pub fn frame_duration(&self) -> Duration {
        self.frame_duration
    }

    /// Get detected FPS if available
    #[allow(dead_code)] // Part of public API for FPS introspection
    pub fn detected_fps(&self) -> Option<f64> {
        self.stats.detected_fps
    }

    /// Check if video is currently playing
    #[allow(dead_code)] // Part of public API for playback state queries
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// Set whether video should loop
    #[allow(dead_code)] // Part of public API for loop control
    pub fn set_loop(&mut self, should_loop: bool) {
        self.should_loop = should_loop;
    }
}

impl Drop for VideoManager {
    fn drop(&mut self) {
        log::info!("VideoManager::drop - Stopping video pipeline and cleaning up resources");

        // Clear callbacks first to prevent new frames
        self.app_sink
            .set_callbacks(gst_app::AppSinkCallbacks::builder().build());

        // Stop pipeline
        log::debug!("Setting pipeline state to Null...");
        match self.pipeline.set_state(gst::State::Null) {
            Ok(state_change) => {
                log::debug!("Pipeline state change result: {:?}", state_change);

                // Wait for state change to complete (with timeout)
                let (result, current, pending) =
                    self.pipeline.state(Some(gst::ClockTime::from_seconds(2)));

                match result {
                    Ok(_) => {
                        log::debug!(
                            "Pipeline final state: current={:?}, pending={:?}",
                            current,
                            pending
                        );
                    }
                    Err(e) => {
                        log::warn!("Failed to get pipeline final state: {:?}", e);
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to set pipeline state to Null: {}", e);
            }
        }

        // Drain pending messages from bus
        if let Some(bus) = self.pipeline.bus() {
            let mut drained = 0;
            while bus.pop().is_some() {
                drained += 1;
            }
            if drained > 0 {
                log::debug!("Drained {} pending messages from bus", drained);
            }
        }

        // Clear cached frames
        self.frames.clear_cached_frame();

        log::info!(
            "VideoManager::drop - Pipeline stopped, resources cleaned up (rendered: {}, dropped: {})",
            self.stats.frames_rendered,
            self.stats.frames_dropped.load(Ordering::Relaxed)
        );
    }
}
