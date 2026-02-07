//! GStreamer pipeline setup and configuration
//!
//! This module handles the creation and configuration of GStreamer pipelines
//! for hardware-accelerated video decoding.

use anyhow::{Context, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Initialize GStreamer (idempotent, safe to call multiple times)
pub fn initialize_gstreamer() {
    static GSTREAMER_INITIALIZED: std::sync::Once = std::sync::Once::new();

    GSTREAMER_INITIALIZED.call_once(|| {
        gst::init().expect("Failed to initialize GStreamer");
        log::info!("GStreamer initialized");
    });
}

/// Build a hardware-accelerated video pipeline
///
/// Uses VA-API for hardware decoding and post-processing:
/// - `vah264dec`: Hardware H.264 decoder (outputs NV12)
/// - `vapostproc`: Hardware scaling + color conversion to BGRA
///
/// # Arguments
///
/// * `path` - Path to the video file
/// * `target_width` - Target width for decoded frames
/// * `target_height` - Target height for decoded frames
///
/// # Returns
///
/// Tuple of (pipeline, app_sink) where app_sink can receive decoded frames
pub fn build_pipeline(
    path: impl AsRef<Path>,
    target_width: u32,
    target_height: u32,
) -> Result<(gst::Pipeline, gst_app::AppSink)> {
    let path = path.as_ref();
    log::info!("Creating GStreamer pipeline for: {}", path.display());

    // Hardware-accelerated pipeline
    let pipeline_str = format!(
        "filesrc location={} ! qtdemux ! h264parse ! vah264dec ! vapostproc ! video/x-raw,format=BGRA,width={},height={} ! appsink name=sink",
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

    Ok((pipeline, app_sink))
}

/// Configure AppSink for low-latency video delivery
///
/// Settings optimized for wallpaper video playback:
/// - `sync=true`: Proper frame pacing (respects video timestamps)
/// - `max-buffers=1`: Minimal latency
/// - `drop=true`: Let GStreamer drop old frames if queue fills
pub fn configure_app_sink(app_sink: &gst_app::AppSink) {
    app_sink.set_property("emit-signals", true);
    app_sink.set_property("sync", true); // Critical for proper frame pacing
    app_sink.set_property("max-buffers", 1u32);
    app_sink.set_property("drop", true);
}

/// Setup frame callback for AppSink
///
/// Callback receives frames from GStreamer and stores them for rendering
pub fn setup_frame_callback(
    app_sink: &gst_app::AppSink,
    current_frame: Arc<Mutex<Option<Vec<u8>>>>,
    new_frame_flag: Arc<std::sync::atomic::AtomicBool>,
    frames_dropped: Arc<AtomicU64>,
    #[cfg(feature = "profiling")] gstreamer_frame_time: Arc<Mutex<Option<Instant>>>,
) {
    app_sink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |sink| {
                #[cfg(feature = "profiling")]
                let frame_arrival = Instant::now();

                let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;

                // Map buffer to read pixel data
                let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;
                let data = map.as_slice();

                // Convert BGRA to ARGB8888 (just copy on little-endian)
                let argb_data = data.to_vec();

                // Store the frame
                if let Ok(mut frame) = current_frame.lock() {
                    // Check if we're dropping a frame (previous frame not consumed yet)
                    if new_frame_flag.load(std::sync::atomic::Ordering::Relaxed) {
                        frames_dropped.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        log::trace!("Video frame dropped (previous frame not consumed in time)");
                    }

                    *frame = Some(argb_data);

                    // Set flag to indicate new frame is available
                    new_frame_flag.store(true, std::sync::atomic::Ordering::Release);

                    // Record when GStreamer delivered this frame
                    #[cfg(feature = "profiling")]
                    {
                        if let Ok(mut timestamp) = gstreamer_frame_time.lock() {
                            *timestamp = Some(frame_arrival);
                        }
                    }
                }

                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );
}

/// Detect video FPS from pipeline
pub fn detect_fps(pipeline: &gst::Pipeline) -> Option<f64> {
    // Try to get FPS from the pipeline
    if let Some(pad) = pipeline.by_name("sink")?.static_pad("sink")?.peer() {
        if let Some(caps) = pad.current_caps() {
            if let Some(structure) = caps.structure(0) {
                if let Ok(framerate) = structure.get::<gst::Fraction>("framerate") {
                    let fps = framerate.numer() as f64 / framerate.denom() as f64;
                    log::info!("Detected video FPS: {:.2}", fps);
                    return Some(fps);
                }
            }
        }
    }

    log::warn!("Could not detect video FPS, assuming 30fps");
    Some(30.0)
}
