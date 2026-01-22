use anyhow::{Context, Result};
use image::{codecs::gif::GifDecoder, AnimationDecoder, ImageBuffer, Rgba};
use std::path::Path;
use std::time::{Duration, Instant};

/// Manages animated GIF playback
pub struct GifManager {
    frames: Vec<GifFrame>,
    current_frame: usize,
    last_frame_time: Instant,
    loop_count: Option<u32>, // None = infinite loop
}

/// A single frame from a GIF animation (pre-scaled and converted)
struct GifFrame {
    argb_data: Vec<u8>, // Pre-converted ARGB8888 data
    delay: Duration,
}

impl GifManager {
    /// Load a GIF file, extract all frames, and pre-scale them to target size
    pub fn load(
        path: impl AsRef<Path>,
        target_width: u32,
        target_height: u32,
        scale_mode: common::ScaleMode,
        wallpaper_manager: &crate::wallpaper_manager::WallpaperManager,
        #[cfg(feature = "gpu")] gpu_renderer: Option<&crate::gpu::GpuRenderer>,
    ) -> Result<Self> {
        let path = path.as_ref();
        let file = std::fs::File::open(path)
            .with_context(|| format!("Failed to open GIF: {}", path.display()))?;
        let reader = std::io::BufReader::new(file);

        let decoder = GifDecoder::new(reader).context("Failed to create GIF decoder")?;

        let frames_iter = decoder.into_frames();
        let mut frames = Vec::new();

        let load_start = std::time::Instant::now();
        log::info!(
            "Pre-scaling GIF frames to {}x{}",
            target_width,
            target_height
        );
        #[cfg(feature = "gpu")]
        if gpu_renderer.is_some() {
            log::info!("Using GPU acceleration for GIF frame scaling");
        }

        for (idx, frame_result) in frames_iter.enumerate() {
            let frame = frame_result.context("Failed to decode GIF frame")?;

            // Get frame delay (convert from centiseconds to Duration)
            let delay_cs = frame.delay().numer_denom_ms();
            let delay = Duration::from_millis((delay_cs.0 as u64 * 1000) / delay_cs.1 as u64);

            // Log unusually long delays
            if idx < 5 && delay.as_secs() > 1 {
                log::info!("Frame {} has delay: {:.1}s", idx, delay.as_secs_f32());
            }

            // Ensure minimum delay of 10ms (100fps max)
            let delay = if delay < Duration::from_millis(10) {
                Duration::from_millis(10)
            } else {
                delay
            };

            // Get frame buffer
            let buffer = frame.into_buffer();
            let (src_width, src_height) = buffer.dimensions();

            // Convert frame to DynamicImage
            let image = image::DynamicImage::ImageRgba8(buffer);

            // Scale to target size using GPU if available, otherwise CPU
            let argb_data = {
                #[cfg(feature = "gpu")]
                {
                    if let Some(gpu) = gpu_renderer {
                        // Use GPU scaling
                        let rgba_image = image.to_rgba8();
                        match gpu.render_image(
                            rgba_image.as_raw(),
                            src_width,
                            src_height,
                            target_width,
                            target_height,
                        ) {
                            Ok(data) => data,
                            Err(e) => {
                                log::warn!(
                                    "GPU scaling failed for GIF frame {}: {}, falling back to CPU",
                                    idx,
                                    e
                                );
                                // Fallback to CPU
                                let scaled = wallpaper_manager.scale_image(
                                    &image,
                                    target_width,
                                    target_height,
                                    scale_mode,
                                )?;
                                wallpaper_manager.rgba_to_argb8888(&scaled)
                            }
                        }
                    } else {
                        // No GPU, use CPU
                        let scaled = wallpaper_manager.scale_image(
                            &image,
                            target_width,
                            target_height,
                            scale_mode,
                        )?;
                        wallpaper_manager.rgba_to_argb8888(&scaled)
                    }
                }

                #[cfg(not(feature = "gpu"))]
                {
                    // GPU feature disabled, use CPU
                    let scaled = wallpaper_manager.scale_image(
                        &image,
                        target_width,
                        target_height,
                        scale_mode,
                    )?;
                    wallpaper_manager.rgba_to_argb8888(&scaled)
                }
            };

            frames.push(GifFrame { argb_data, delay });

            if (idx + 1) % 50 == 0 {
                log::info!("Pre-scaled {} frames...", idx + 1);
            }
        }

        let load_duration = load_start.elapsed();
        log::info!(
            "Loaded and pre-scaled GIF with {} frames in {:.2}s",
            frames.len(),
            load_duration.as_secs_f64()
        );

        Ok(Self {
            frames,
            current_frame: 0,
            last_frame_time: Instant::now() - Duration::from_secs(1), // Start in the past to trigger first update
            loop_count: None,                                         // Infinite loop by default
        })
    }

    /// Get the pre-scaled ARGB data for the current frame
    pub fn current_frame_data(&self) -> &[u8] {
        &self.frames[self.current_frame].argb_data
    }

    /// Check if it's time to advance to the next frame, and do so if needed
    /// Returns true if the frame changed
    pub fn update(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_frame_time);

        let current_delay = self.frames[self.current_frame].delay;

        if elapsed >= current_delay {
            let old_frame = self.current_frame;
            self.current_frame = (self.current_frame + 1) % self.frames.len();
            self.last_frame_time = now;
            log::trace!(
                "GIF frame update: {} -> {} (delay: {:?}ms, elapsed: {:?}ms)",
                old_frame,
                self.current_frame,
                current_delay.as_millis(),
                elapsed.as_millis()
            );
            true
        } else {
            false
        }
    }

    /// Get the time until the next frame should be displayed
    pub fn time_until_next_frame(&self) -> Duration {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_frame_time);
        let current_delay = self.frames[self.current_frame].delay;

        current_delay.saturating_sub(elapsed)
    }

    /// Get the total number of frames
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Get the current frame index
    pub fn current_frame_index(&self) -> usize {
        self.current_frame
    }
}
