#[cfg(feature = "video")]
use anyhow::{Context, Result};
#[cfg(feature = "video")]
use gstreamer as gst;
#[cfg(feature = "video")]
use gstreamer::prelude::*;
#[cfg(feature = "video")]
use gstreamer_app as gst_app;
#[cfg(feature = "video")]
use gstreamer_video as gst_video;
#[cfg(feature = "video")]
use std::path::Path;
#[cfg(feature = "video")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "video")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "video")]
use std::time::{Duration, Instant};

#[cfg(feature = "video")]
/// Manages video playback for wallpapers
pub struct VideoManager {
    /// GStreamer pipeline
    pipeline: gst::Pipeline,
    /// App sink to receive video frames
    app_sink: gst_app::AppSink,
    /// Current frame data (ARGB8888)
    current_frame: Arc<Mutex<Option<Vec<u8>>>>,
    /// Flag indicating a new frame is available
    new_frame_available: Arc<AtomicBool>,
    /// Last frame update time
    last_frame_time: Instant,
    /// Target framerate (for frame timing)
    frame_duration: Duration,
    /// Video dimensions
    width: u32,
    height: u32,
    /// Whether video is playing
    is_playing: bool,
    /// Loop the video
    should_loop: bool,
    /// Frame statistics
    frames_rendered: u64,
    frames_dropped: Arc<std::sync::atomic::AtomicU64>,
    /// Detected video FPS
    detected_fps: Option<f64>,
}

#[cfg(feature = "video")]
impl VideoManager {
    /// Load a video file and prepare for playback
    pub fn load(
        path: impl AsRef<Path>,
        target_width: u32,
        target_height: u32,
        scale_mode: common::ScaleMode,
        muted: bool,
    ) -> Result<Self> {
        // Initialize GStreamer
        gst::init().context("Failed to initialize GStreamer")?;

        let path = path.as_ref();
        log::info!("Loading video: {}", path.display());

        // Create GStreamer pipeline
        // Pipeline: filesrc -> decodebin -> videoconvert -> videoscale -> appsink
        let pipeline_str = format!(
            "filesrc location={} ! decodebin ! videoconvert ! videoscale ! video/x-raw,format=BGRA,width={},height={} ! appsink name=sink",
            path.display(),
            target_width,
            target_height
        );

        log::debug!("GStreamer pipeline: {}", pipeline_str);

        let pipeline = gst::parse::launch(&pipeline_str)
            .context("Failed to create GStreamer pipeline")?
            .dynamic_cast::<gst::Pipeline>()
            .map_err(|_| anyhow::anyhow!("Pipeline is not a gst::Pipeline"))?;

        // Get the appsink element
        let app_sink = pipeline
            .by_name("sink")
            .context("Failed to get appsink from pipeline")?
            .dynamic_cast::<gst_app::AppSink>()
            .map_err(|_| anyhow::anyhow!("sink is not an AppSink"))?;

        // Configure appsink
        app_sink.set_property("emit-signals", true);
        app_sink.set_property("sync", true); // Sync to clock to avoid decoding too fast
        app_sink.set_property("max-buffers", 1u32); // Only 1 buffer to minimize latency
        app_sink.set_property("drop", false); // Don't drop frames in GStreamer (we handle it)

        let current_frame = Arc::new(Mutex::new(None));
        let current_frame_clone = Arc::clone(&current_frame);

        let new_frame_flag = Arc::new(AtomicBool::new(false));
        let new_frame_flag_clone = Arc::clone(&new_frame_flag);

        let frames_dropped = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let frames_dropped_clone = Arc::clone(&frames_dropped);

        // Set up callback to receive frames
        app_sink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;

                    // Map buffer to read pixel data
                    let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;
                    let data = map.as_slice();

                    // Convert BGRA to ARGB8888 (just copy, they're the same on little-endian)
                    let argb_data = data.to_vec();

                    // Store the frame
                    if let Ok(mut frame) = current_frame_clone.lock() {
                        // Check if we're dropping a frame (previous frame NOT consumed yet)
                        // If new_frame_flag is still TRUE, it means the old frame wasn't consumed
                        if new_frame_flag_clone.load(std::sync::atomic::Ordering::Relaxed) {
                            frames_dropped_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            log::trace!(
                                "Video frame dropped (previous frame not consumed in time)"
                            );
                        }

                        *frame = Some(argb_data);
                        // Set flag to indicate new frame is available
                        new_frame_flag_clone.store(true, std::sync::atomic::Ordering::Release);
                    }

                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );

        // Assume 30 FPS by default (will be updated when we detect actual FPS)
        let frame_duration = Duration::from_millis(33); // ~30fps

        Ok(Self {
            pipeline,
            app_sink,
            current_frame,
            new_frame_available: new_frame_flag,
            last_frame_time: Instant::now(),
            frame_duration,
            width: target_width,
            height: target_height,
            is_playing: false,
            should_loop: true,
            frames_rendered: 0,
            frames_dropped,
            detected_fps: None,
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
    pub fn pause(&mut self) -> Result<()> {
        log::info!("Pausing video playback");
        self.pipeline
            .set_state(gst::State::Paused)
            .context("Failed to set pipeline to Paused state")?;
        self.is_playing = false;
        Ok(())
    }

    /// Get the current frame data (ARGB8888)
    pub fn current_frame_data(&self) -> Option<Vec<u8>> {
        self.current_frame.lock().ok()?.clone()
    }

    /// Check if a new frame is available and should be displayed
    /// Returns true if there's a new frame to render
    pub fn update(&mut self) -> bool {
        if !self.is_playing {
            return false;
        }

        // Detect FPS from pipeline caps (do this once on first frame)
        if self.detected_fps.is_none() && self.frames_rendered == 0 {
            if let Some(pad) = self.app_sink.static_pad("sink") {
                if let Some(caps) = pad.current_caps() {
                    if let Some(structure) = caps.structure(0) {
                        if let Ok(framerate) = structure.get::<gst::Fraction>("framerate") {
                            let fps = framerate.numer() as f64 / framerate.denom() as f64;
                            self.detected_fps = Some(fps);
                            self.frame_duration = Duration::from_secs_f64(1.0 / fps);
                            log::info!(
                                "Detected video FPS: {:.2} ({}/{}), frame duration: {:.1}ms",
                                fps,
                                framerate.numer(),
                                framerate.denom(),
                                self.frame_duration.as_secs_f64() * 1000.0
                            );
                        }
                    }
                }
            }
        }

        // Check for EOS (end of stream) for looping
        if let Some(bus) = self.pipeline.bus() {
            if let Some(msg) = bus.pop() {
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
                    _ => {}
                }
            }
        }

        // Check if we have a new frame available using atomic flag
        // This avoids rendering the same frame multiple times
        if self.new_frame_available.swap(false, Ordering::Acquire) {
            self.last_frame_time = Instant::now();
            self.frames_rendered += 1;

            // Log statistics every 600 frames (~20 seconds at 30fps, ~10 seconds at 60fps)
            if self.frames_rendered % 600 == 0 {
                let dropped = self
                    .frames_dropped
                    .load(std::sync::atomic::Ordering::Relaxed);
                let total_frames = self.frames_rendered + dropped;
                let drop_rate = if total_frames > 0 {
                    (dropped as f64 / total_frames as f64) * 100.0
                } else {
                    0.0
                };
                let fps_str = self
                    .detected_fps
                    .map(|f| format!("{:.2} fps", f))
                    .unwrap_or_else(|| "unknown fps".to_string());
                log::info!(
                    "Video stats ({}): {} rendered, {} dropped of {} total ({:.1}% drop rate)",
                    fps_str,
                    self.frames_rendered,
                    dropped,
                    total_frames,
                    drop_rate
                );
            }

            true
        } else {
            false
        }
    }

    /// Get video dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Get the frame duration for adaptive polling
    /// Returns the expected time between frames based on detected FPS
    pub fn frame_duration(&self) -> Duration {
        self.frame_duration
    }

    /// Get detected FPS if available
    pub fn detected_fps(&self) -> Option<f64> {
        self.detected_fps
    }

    /// Check if video is currently playing
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// Set whether video should loop
    pub fn set_loop(&mut self, should_loop: bool) {
        self.should_loop = should_loop;
    }
}

#[cfg(feature = "video")]
impl Drop for VideoManager {
    fn drop(&mut self) {
        log::debug!("Stopping video pipeline");
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

// Stub implementation when video feature is disabled
#[cfg(not(feature = "video"))]
pub struct VideoManager;

#[cfg(not(feature = "video"))]
impl VideoManager {
    pub fn load(
        _path: impl AsRef<std::path::Path>,
        _target_width: u32,
        _target_height: u32,
        _scale_mode: common::ScaleMode,
        _muted: bool,
    ) -> anyhow::Result<Self> {
        anyhow::bail!("Video support not compiled in. Build with --features video")
    }

    pub fn play(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn pause(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn current_frame_data(&self) -> Option<Vec<u8>> {
        None
    }

    pub fn update(&mut self) -> bool {
        false
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (0, 0)
    }

    pub fn is_playing(&self) -> bool {
        false
    }

    pub fn set_loop(&mut self, _should_loop: bool) {}

    pub fn frame_duration(&self) -> std::time::Duration {
        std::time::Duration::from_millis(33) // Stub: ~30 FPS
    }

    pub fn detected_fps(&self) -> Option<f64> {
        None
    }
}
