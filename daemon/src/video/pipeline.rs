//! GStreamer pipeline setup and configuration
//!
//! This module handles the creation and configuration of GStreamer pipelines
//! for video decoding. Uses `decodebin` for automatic codec/container detection,
//! supporting H.264, H.265, VP9, AV1 in MP4, WebM, MKV, and other containers.
//! Hardware acceleration (VA-API, NVDEC, etc.) is used automatically when available.

use anyhow::{Context, Result};
use arc_swap::ArcSwap;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "profiling")]
use std::time::Instant;

/// Initialize GStreamer (idempotent, safe to call multiple times)
pub fn initialize_gstreamer() {
    static GSTREAMER_INITIALIZED: std::sync::Once = std::sync::Once::new();

    GSTREAMER_INITIALIZED.call_once(|| {
        gst::init().expect("Failed to initialize GStreamer");
        log::info!("GStreamer initialized");
    });
}

/// Build a video decoding pipeline using `decodebin` for automatic codec detection.
///
/// Uses GStreamer's element API (not string parsing) to avoid path escaping issues.
/// `decodebin` automatically selects the best decoder (hardware-accelerated when
/// available, software fallback otherwise) for any supported container and codec:
/// - Containers: MP4, WebM, MKV, AVI, MOV, OGG, etc.
/// - Codecs: H.264, H.265/HEVC, VP8, VP9, AV1, etc.
///
/// Pipeline structure:
/// ```text
/// filesrc -> decodebin -> videoconvert -> videoscale -> capsfilter(BGRA,WxH) -> appsink
/// ```
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

    // Build pipeline using element API to avoid path escaping issues
    let pipeline = gst::Pipeline::new();

    // Source: reads the file
    let filesrc = gst::ElementFactory::make("filesrc")
        .property(
            "location",
            path.to_str().context("Video path contains invalid UTF-8")?,
        )
        .build()
        .context("Failed to create filesrc element")?;

    // Decoder: auto-detects container and codec, uses HW accel when available
    let decodebin = gst::ElementFactory::make("decodebin")
        .build()
        .context("Failed to create decodebin element")?;

    // Color space converter: handles NV12/YUV -> BGRA conversion
    let videoconvert = gst::ElementFactory::make("videoconvert")
        .build()
        .context("Failed to create videoconvert element")?;

    // Scaler: resizes to target resolution
    let videoscale = gst::ElementFactory::make("videoscale")
        .build()
        .context("Failed to create videoscale element")?;

    // Caps filter: enforce output format and resolution
    let caps = gst::Caps::builder("video/x-raw")
        .field("format", "BGRA")
        .field("width", target_width as i32)
        .field("height", target_height as i32)
        .build();

    let capsfilter = gst::ElementFactory::make("capsfilter")
        .property("caps", &caps)
        .build()
        .context("Failed to create capsfilter element")?;

    // Sink: delivers frames to our application
    let appsink = gst::ElementFactory::make("appsink")
        .name("sink")
        .build()
        .context("Failed to create appsink element")?;

    // Add all elements to the pipeline
    pipeline
        .add_many([
            &filesrc,
            &decodebin,
            &videoconvert,
            &videoscale,
            &capsfilter,
            &appsink,
        ])
        .context("Failed to add elements to pipeline")?;

    // Link static elements: filesrc -> decodebin (static link)
    gst::Element::link_many([&filesrc, &decodebin])
        .context("Failed to link filesrc to decodebin")?;

    // Link post-decode chain: videoconvert -> videoscale -> capsfilter -> appsink
    gst::Element::link_many([&videoconvert, &videoscale, &capsfilter, &appsink])
        .context("Failed to link video processing chain")?;

    // Connect decodebin's dynamic pad to videoconvert.
    // decodebin creates pads dynamically when it discovers the stream type.
    let convert_weak = videoconvert.downgrade();
    decodebin.connect_pad_added(move |_decodebin, src_pad| {
        let Some(videoconvert) = convert_weak.upgrade() else {
            log::warn!("decodebin pad-added: videoconvert element already dropped");
            return;
        };

        // Only link video pads (ignore audio, subtitles, etc.)
        let pad_caps = src_pad
            .current_caps()
            .or_else(|| Some(src_pad.query_caps(None)));
        if let Some(caps) = pad_caps {
            let structure_name = caps
                .structure(0)
                .map(|s| s.name().to_string())
                .unwrap_or_default();

            if !structure_name.starts_with("video/") {
                log::debug!("Ignoring non-video pad from decodebin: {}", structure_name);
                return;
            }
        }

        let sink_pad = match videoconvert.static_pad("sink") {
            Some(pad) => pad,
            None => {
                log::error!("videoconvert has no sink pad");
                return;
            }
        };

        if sink_pad.is_linked() {
            log::debug!("videoconvert sink pad already linked, ignoring additional video stream");
            return;
        }

        match src_pad.link(&sink_pad) {
            Ok(_) => {
                log::info!("Linked decodebin to videoconvert successfully");
            }
            Err(e) => {
                log::error!("Failed to link decodebin to videoconvert: {:?}", e);
            }
        }
    });

    log::debug!(
        "GStreamer pipeline: filesrc({}) -> decodebin -> videoconvert -> videoscale -> BGRA {}x{} -> appsink",
        path.display(),
        target_width,
        target_height
    );

    // Cast appsink to AppSink type
    let app_sink = appsink
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
/// Callback receives frames from GStreamer and stores them for rendering.
/// Uses lock-free ArcSwap + generation counter instead of Mutex + AtomicBool
/// to eliminate contention between the GStreamer thread and the render thread.
///
/// Drop detection is handled on the reader side: the render thread compares
/// the current generation against its last-seen generation to count skipped frames.
pub fn setup_frame_callback(
    app_sink: &gst_app::AppSink,
    current_frame: Arc<ArcSwap<Vec<u8>>>,
    frame_generation: Arc<AtomicU64>,
    _frames_dropped: Arc<AtomicU64>,
    #[cfg(feature = "profiling")] gstreamer_frame_time: Arc<std::sync::Mutex<Option<Instant>>>,
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

                // Copy frame data (GStreamer buffer cannot be retained past this scope)
                let argb_data = data.to_vec();

                // Store frame lock-free and bump generation counter
                current_frame.store(Arc::new(argb_data));
                frame_generation.fetch_add(1, Ordering::Release);

                // Record when GStreamer delivered this frame
                #[cfg(feature = "profiling")]
                {
                    if let Ok(mut timestamp) = gstreamer_frame_time.lock() {
                        *timestamp = Some(frame_arrival);
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
