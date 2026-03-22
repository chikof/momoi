//! Main per-output render loop.
//!
//! [`WallpaperRunner`] drives the per-output wallpaper lifecycle:
//! tick the timer → sample audio → build uniforms → render → composite overlays
//! → send frame to the Wayland submission loop.

use crate::{RuntimeError, WallpaperContext, time::FrameTimer};
use render_core::FrameStats;
use tokio::sync::watch;
use tracing::{debug, error, info, trace};

/// Rendered frame sent from a render thread to the Wayland submission loop.
pub struct RenderedFrame {
    /// Output name this frame belongs to (e.g. `"DP-1"`).
    pub output_name: String,
    /// Raw RGBA pixel data, row-major, 4 bytes per pixel.
    pub pixels: Vec<u8>,
}

/// Drives the render loop for a single [`WallpaperContext`].
pub struct WallpaperRunner {
    context: WallpaperContext,
    timer: FrameTimer,
    target_fps: u32,
}

impl WallpaperRunner {
    /// Create a runner for the given context.
    ///
    /// `target_fps` is clamped to `[1, 240]`.
    #[must_use]
    pub fn new(context: WallpaperContext, target_fps: u32) -> Self {
        Self {
            context,
            timer: FrameTimer::new(),
            target_fps: target_fps.clamp(1, 240),
        }
    }

    /// Run the render loop until `shutdown` fires `true`.
    ///
    /// Intended to be called on a dedicated OS thread (`std::thread::spawn`).
    ///
    /// # Errors
    /// Returns the first unrecoverable [`RuntimeError`].
    pub fn run(mut self, shutdown: &watch::Receiver<bool>) -> Result<(), RuntimeError> {
        info!(
            output = %self.context.output.name,
            fps = self.target_fps,
            "render loop starting"
        );

        loop {
            if *shutdown.borrow() {
                info!(output = %self.context.output.name, "render loop shutting down");
                break;
            }

            if let Err(e) = self.tick() {
                error!(output = %self.context.output.name, error = %e, "frame error");
            }

            self.timer.sleep_until_next_frame(self.target_fps);
        }

        Ok(())
    }

    /// Like [`Self::run`] but sends each completed frame over `tx` so the
    /// Wayland event loop can submit it to `wl_surface`.
    ///
    /// Uses `try_send` so a slow compositor never blocks the render thread;
    /// frames are silently dropped when the channel is full.
    ///
    /// # Errors
    /// Returns the first unrecoverable [`RuntimeError`].
    pub fn run_with_sender(
        mut self,
        shutdown: &watch::Receiver<bool>,
        tx: &tokio::sync::mpsc::Sender<RenderedFrame>,
        output_name: &str,
    ) -> Result<(), RuntimeError> {
        info!(output = %output_name, fps = self.target_fps, "render loop starting");

        loop {
            if *shutdown.borrow() {
                info!(output = %output_name, "render loop stopped");
                break;
            }
            if tx.is_closed() {
                info!(output = %output_name, "frame channel closed — stopping");
                break;
            }

            let (elapsed, delta) = self.timer.tick();
            self.context.uniforms.time = elapsed;
            self.context.uniforms.delta_time = delta;
            self.context.stats.frame_index += 1;

            // Sample the latest audio spectrum (non-blocking).
            let spectrum = self.context.audio.current_spectrum()?;
            self.context.uniforms.audio_bands = spectrum.to_f32_array();

            // Render frame.
            let mut frame = {
                let mut renderer = self.context.renderer.lock();
                let mut stats = FrameStats::default();
                renderer.render_frame(&self.context.uniforms, &mut stats)?
            };

            // Composite overlay widgets (alpha-blend on top).
            self.context.overlays.update_all();
            self.context
                .overlays
                .composite(&mut frame.data, frame.width, frame.height)?;

            debug!(
                frame = self.context.stats.frame_index,
                time = elapsed,
                "frame rendered"
            );

            // Non-blocking send — drop the frame if the channel is full.
            let msg = RenderedFrame {
                output_name: output_name.to_string(),
                pixels: frame.data,
            };
            if tx.try_send(msg).is_err() {
                trace!(output = %output_name, "frame dropped — channel full");
            }

            self.timer.sleep_until_next_frame(self.target_fps);
        }

        Ok(())
    }

    fn tick(&mut self) -> Result<(), RuntimeError> {
        let (elapsed, delta) = self.timer.tick();
        self.context.uniforms.time = elapsed;
        self.context.uniforms.delta_time = delta;
        self.context.stats.frame_index += 1;

        let spectrum = self.context.audio.current_spectrum()?;
        self.context.uniforms.audio_bands = spectrum.to_f32_array();

        let mut frame = {
            let mut renderer = self.context.renderer.lock();
            let mut stats = FrameStats::default();
            renderer.render_frame(&self.context.uniforms, &mut stats)?
        };

        self.context.overlays.update_all();
        self.context
            .overlays
            .composite(&mut frame.data, frame.width, frame.height)?;

        Ok(())
    }
}

impl WallpaperRunner {
    /// Like [`run_with_sender`](Self::run_with_sender) but also increments
    /// `frame_counter` atomically after each successfully sent frame.
    /// Used by the orchestrator to track per-output FPS for `momoi-ctl status`.
    ///
    /// # Errors
    /// Returns the first unrecoverable [`RuntimeError`].
    pub fn run_with_sender_counted(
        mut self,
        shutdown: &tokio::sync::watch::Receiver<bool>,
        tx: &tokio::sync::mpsc::Sender<RenderedFrame>,
        output_name: &str,
        frame_counter: &std::sync::Arc<std::sync::atomic::AtomicU64>,
    ) -> Result<(), RuntimeError> {
        use std::sync::atomic::Ordering;

        tracing::info!(output = %output_name, fps = self.target_fps, "render loop starting");

        loop {
            if *shutdown.borrow() {
                tracing::info!(output = %output_name, "render loop stopped");
                break;
            }
            if tx.is_closed() {
                break;
            }

            let (elapsed, delta) = self.timer.tick();
            self.context.uniforms.time = elapsed;
            self.context.uniforms.delta_time = delta;
            self.context.stats.frame_index += 1;

            let spectrum = self.context.audio.current_spectrum()?;
            self.context.uniforms.audio_bands = spectrum.to_f32_array();

            let mut frame = {
                let mut renderer = self.context.renderer.lock();
                let mut stats = render_core::FrameStats::default();
                renderer.render_frame(&self.context.uniforms, &mut stats)?
            };

            self.context.overlays.update_all();
            self.context
                .overlays
                .composite(&mut frame.data, frame.width, frame.height)?;

            let msg = RenderedFrame {
                output_name: output_name.to_string(),
                pixels: frame.data,
            };
            if tx.try_send(msg).is_ok() {
                frame_counter.fetch_add(1, Ordering::Relaxed);
            }

            self.timer.sleep_until_next_frame(self.target_fps);
        }

        Ok(())
    }
}
