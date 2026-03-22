//! Renderer factory — constructs the correct [`Renderer`] from a [`WallpaperConfig`].
//!
//! The factory is the single place that maps configuration to implementation.
//! All GPU/CPU fallback logic lives here; the orchestrator just calls
//! `build_renderer` and gets back a `DynRenderer` ready to use.

use anyhow::Result;
use config_system::WallpaperConfig;
use futures::future::BoxFuture;
use render_core::{DynRenderer, Renderer, SurfaceDescriptor};
use render_cpu::CpuRenderer;
use render_gpu::GpuRenderer;
use shader_engine::ShaderRegistry;
use std::{ops::Deref, sync::Arc};
use tracing::{info, warn};
use wallpaper_runtime::{ImageRenderer, TimeBasedRenderer, TimeConfig, image_wallpaper::ScaleMode};

/// Build a [`DynRenderer`] for `config` on `surface`.
///
/// Tries GPU first unless `prefer_gpu` is false; falls back to CPU
/// automatically on any GPU failure.
///
/// This is a `BoxFuture` (not `async fn`) so it can recurse for
/// `TimeBased` without hitting the "recursive `async fn`" restriction.
pub fn build_renderer<'a>(
    config: &'a WallpaperConfig,
    surface: &'a SurfaceDescriptor,
    prefer_gpu: bool,
    registry: &'a ShaderRegistry,
) -> BoxFuture<'a, Result<DynRenderer>> {
    Box::pin(async move {
        let renderer: Box<dyn Renderer> = match config {
            WallpaperConfig::Image { path } => {
                let mut r = ImageRenderer::new(shellexpand_path(path), ScaleMode::Fit);
                r.init(surface)?;
                info!(path = %path.display(), "image wallpaper loaded");
                Box::new(r)
            }

            WallpaperConfig::Shader { path, .. } => {
                let p = shellexpand_path(path);
                let name = shader_name(&p);
                if let Err(e) = registry.register_file(&name, &p) {
                    warn!(shader = %p.display(), error = %e, "shader registration failed");
                }
                gpu_or_cpu(surface, prefer_gpu, registry, Some(&name)).await?
            }

            WallpaperConfig::AudioReactive { path, .. } => {
                let p = shellexpand_path(path);
                let name = shader_name(&p);
                if let Err(e) = registry.register_file(&name, &p) {
                    warn!(shader = %p.display(), error = %e, "shader registration failed");
                }
                gpu_or_cpu(surface, prefer_gpu, registry, Some(&name)).await?
            }

            WallpaperConfig::TimeBased { day, night } => {
                let day_dyn = build_renderer(day, surface, prefer_gpu, registry).await?;
                let night_dyn = build_renderer(night, surface, prefer_gpu, registry).await?;

                // Wrap DynRenderer in a thin Box<dyn Renderer> shim so
                // TimeBasedRenderer can hold them.
                let day_box: Box<dyn Renderer> = Box::new(DynRendererShim(day_dyn));
                let night_box: Box<dyn Renderer> = Box::new(DynRendererShim(night_dyn));

                Box::new(TimeBasedRenderer::new(
                    day_box,
                    night_box,
                    TimeConfig::default(),
                ))
            }
        };

        Ok(Arc::new(parking_lot::Mutex::new(renderer)))
    })
}

/// Try to build a GPU renderer; silently fall back to CPU on any error.
async fn gpu_or_cpu(
    surface: &SurfaceDescriptor,
    prefer_gpu: bool,
    registry: &ShaderRegistry,
    shader_name: Option<&str>,
) -> Result<Box<dyn Renderer>> {
    if prefer_gpu {
        match GpuRenderer::new(registry.clone()).await {
            Ok(mut r) => {
                r.init(surface)?;
                if let Some(name) = shader_name
                    && let Err(e) = r.set_shader(name)
                {
                    warn!(shader = name, error = %e, "shader set failed — rendering black");
                }
                info!(backend = "gpu", "renderer ready");
                return Ok(Box::new(r));
            }
            Err(e) => {
                warn!(error = %e, "GPU unavailable, falling back to CPU renderer");
            }
        }
    }
    let mut r = CpuRenderer::default();
    r.init(surface)?;
    info!(backend = "cpu", "renderer ready");
    Ok(Box::new(r))
}

/// Derive a shader registry name from a file path (the stem, e.g. `audio_reactive`).
fn shader_name(path: &std::path::Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("shader")
        .to_owned()
}

/// Expand a leading `~/` to the user's home directory.
pub fn shellexpand_path(path: &std::path::Path) -> std::path::PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    path.to_path_buf()
}

/// Wraps a `DynRenderer` (`Arc<Mutex<Box<dyn Renderer>>>`) so it can be stored
/// as a plain `Box<dyn Renderer>` inside `TimeBasedRenderer`.
struct DynRendererShim(DynRenderer);

impl Deref for DynRendererShim {
    type Target = DynRenderer;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Renderer for DynRendererShim {
    fn init(&mut self, _s: &SurfaceDescriptor) -> Result<(), render_core::RenderError> {
        // Already initialised by the factory before wrapping.
        Ok(())
    }

    fn render_frame(
        &mut self,
        u: &render_core::ShaderUniforms,
        s: &mut render_core::FrameStats,
    ) -> Result<render_core::Frame, render_core::RenderError> {
        self.lock().render_frame(u, s)
    }

    fn resize(&mut self, w: u32, h: u32) -> Result<(), render_core::RenderError> {
        self.lock().resize(w, h)
    }

    fn backend_name(&self) -> &'static str {
        "dynamic-shim"
    }
}
