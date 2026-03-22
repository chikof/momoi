//! Top-level orchestrator: wires all subsystems together.

use crate::factory;
use crate::ipc::{IpcState, OrchestratorCmd};
use anyhow::{Context, Result};
use audio_engine::{AudioSource, SilentSource};
use config_system::DaemonConfig;
use ipc_protocol::OutputStatus;
use render_core::{DynRenderer, OutputInfo, PixelFormat::Rgba8Unorm, Renderer, SurfaceDescriptor};
use render_cpu::CpuRenderer;
use render_gpu::GpuRenderer;
use shader_engine::ShaderRegistry;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{RwLock, mpsc, watch};
use tracing::{error, info, warn};
use wallpaper_runtime::image_wallpaper::ScaleMode;
use wallpaper_runtime::runner::RenderedFrame;
use wallpaper_runtime::{WallpaperContext, WallpaperRunner};
use wayland_backend::WaylandSession;

pub struct Orchestrator {
    config: DaemonConfig,
    shutdown: watch::Receiver<bool>,
    registry: ShaderRegistry,
    wayland: WaylandSession,
    frame_rx: mpsc::Receiver<RenderedFrame>,
    frame_tx: mpsc::Sender<RenderedFrame>,
    render_threads: Vec<std::thread::JoinHandle<()>>,
    renderers: HashMap<String, DynRenderer>,
    wallpaper_names: HashMap<String, String>,
    frame_counters: HashMap<String, Arc<AtomicU64>>,
    cmd_rx: mpsc::Receiver<OrchestratorCmd>,
    ipc_state: Arc<RwLock<IpcState>>,
    last_ipc_update: Instant,
}

impl Orchestrator {
    pub fn new(
        config: DaemonConfig,
        shutdown: watch::Receiver<bool>,
        cmd_rx: mpsc::Receiver<OrchestratorCmd>,
        ipc_state: Arc<RwLock<IpcState>>,
    ) -> Result<Self> {
        let mut wayland =
            WaylandSession::connect().context("failed to connect to Wayland display")?;

        // First roundtrip: enumerate globals and outputs; creates layer surfaces
        // in `new_output()`.
        wayland
            .roundtrip()
            .context("initial Wayland roundtrip failed")?;

        if wayland.state.outputs.is_empty() {
            warn!("no outputs discovered after roundtrips");
        } else {
            for o in &wayland.state.outputs {
                info!(
                      output = %o.name,
                      width  = o.width,
                height = o.height,
                      hz     = o.refresh_mhz / 1000,
                      "output ready"
                  );
            }
        }

        let registry = ShaderRegistry::default();
        let example_wgsl = include_str!("../../assets/shaders/audio_reactive.wgsl");
        if let Err(e) = registry.register_wgsl("audio_reactive", example_wgsl) {
            warn!(error = %e, "could not register built-in shader");
        }

        let (frame_tx, frame_rx) = mpsc::channel(64);

        Ok(Self {
            config,
            shutdown,
            registry,
            wayland,
            frame_rx,
            frame_tx,
            render_threads: Vec::new(),
            renderers: HashMap::new(),
            wallpaper_names: HashMap::new(),
            frame_counters: HashMap::new(),
            cmd_rx,
            ipc_state,
            last_ipc_update: Instant::now(),
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        let outputs = self.wayland.state.outputs.clone();
        if outputs.is_empty() {
            warn!("no outputs — nothing to render");
            return Ok(());
        }

        let audio: Arc<dyn AudioSource> = build_audio_source(&self.config.audio);

        // Populate IPC state immediately so `momoi-ctl status` works from
        // the first second, not after the first update_ipc_state tick.
        {
            let mut state = self.ipc_state.write().await;
            state.audio_active = self.config.audio.enabled;
            state.outputs = outputs
                .iter()
                .map(|o| OutputStatus {
                    name: o.name.clone(),
                    resolution: format!("{}x{}", o.width, o.height),
                    wallpaper: "audio_reactive".to_owned(),
                    fps: 0.0,
                })
                .collect();
        }

        for output in outputs {
            self.spawn_render_thread(output, Arc::clone(&audio)).await?;
        }

        Ok(())
    }

    async fn spawn_render_thread(
        &mut self,
        output: OutputInfo,
        audio: Arc<dyn AudioSource>,
    ) -> Result<()> {
        let surface = SurfaceDescriptor {
            output: output.clone(),
            format: Rgba8Unorm,
        };

        let wallpaper_cfg = self
            .config
            .outputs
            .iter()
            .find(|o| o.name == output.name)
            .or_else(|| self.config.outputs.iter().find(|o| o.name == "*"))
            .map(|o| o.wallpaper.clone());

        let label = wallpaper_cfg
            .as_ref()
            .map_or("audio_reactive".to_owned(), wallpaper_label);

        let dyn_renderer: DynRenderer = build_renderer_non_blocking(
            wallpaper_cfg,
            &surface,
            self.config.prefer_gpu,
            &self.registry,
        )
        .await
        .unwrap_or_else(|e| {
            warn!(error = %e, output = %output.name, "factory failed — CPU fallback");
            make_cpu_renderer(&surface)
        });

        let counter = Arc::new(AtomicU64::new(0));
        self.frame_counters
            .insert(output.name.clone(), Arc::clone(&counter));
        self.renderers
            .insert(output.name.clone(), Arc::clone(&dyn_renderer));
        self.wallpaper_names.insert(output.name.clone(), label);

        let ctx = WallpaperContext::new(output.clone(), dyn_renderer, audio);
        let runner = WallpaperRunner::new(ctx, self.config.fps);
        let shutdown = self.shutdown.clone();
        let frame_tx = self.frame_tx.clone();
        let output_name = output.name.clone();
        let counter_thread = Arc::clone(&counter);

        let handle = std::thread::Builder::new()
            .name(format!("render-{output_name}"))
            .spawn(move || {
                if let Err(e) = runner.run_with_sender_counted(
                    &shutdown,
                    &frame_tx,
                    &output_name.clone(),
                    &counter_thread,
                ) {
                    error!(output = %output_name, error = %e, "render thread crashed");
                }
            })
            .context("failed to spawn render thread")?;

        self.render_threads.push(handle);
        info!(output = %output.name, "render thread spawned");
        Ok(())
    }

    pub async fn run_loop(&mut self) -> Result<()> {
        loop {
            if *self.shutdown.borrow() {
                break;
            }

            while let Ok(cmd) = self.cmd_rx.try_recv() {
                match cmd {
                    OrchestratorCmd::Reload => {
                        info!("config reload requested");
                        match config_system::ConfigLoader::load_or_default() {
                            Ok(new_cfg) => {
                                self.config = new_cfg;
                                if let Err(e) = self.reload_all_renderers().await {
                                    error!(error = %e, "renderer reload failed");
                                }
                            }
                            Err(e) => error!(error = %e, "config reload failed"),
                        }
                    }
                    OrchestratorCmd::SetWallpaper { output, wallpaper } => {
                        info!(output = %output, wallpaper = %wallpaper, "hot-swapping wallpaper");
                        // Spawn the swap as a background task so the Wayland
                        // event loop keeps running (dispatch_pending keeps
                        // firing) during the image load. The swap itself is
                        // safe: it only locks the renderer mutex at the end.
                        self.swap_wallpaper_background(output, &wallpaper);
                    }
                    OrchestratorCmd::Quit => {
                        info!("quit command received");
                        return Ok(());
                    }
                }
            }

            // Keep only the latest frame per output — older frames are stale.
            // This prevents the drain loop from taking seconds in debug mode
            // when 64 frames (×14MB each) have accumulated in the channel.
            {
                let mut latest: std::collections::HashMap<String, RenderedFrame> =
                    std::collections::HashMap::new();
                while let Ok(frame) = self.frame_rx.try_recv() {
                    latest.insert(frame.output_name.clone(), frame);
                }
                for frame in latest.into_values() {
                    self.wayland.present_cpu(&frame.output_name, &frame.pixels);
                }
            }

            self.wayland
                .read_and_dispatch()
                .context("Wayland dispatch error")?;

            if self.last_ipc_update.elapsed() >= Duration::from_secs(1) {
                self.update_ipc_state().await;
                self.last_ipc_update = Instant::now();
            }

            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        Ok(())
    }

    /// Load and swap a wallpaper without blocking the `run_loop`.
    ///
    /// Builds the new renderer(s) on the tokio blocking pool, then swaps
    /// the live `DynRenderer` Arc in-place. The Wayland event loop continues
    /// to run `dispatch_pending` during the load.
    fn swap_wallpaper_background(&mut self, output_name: String, wallpaper: &str) {
        let targets: Vec<String> = if output_name == "*" {
            self.renderers.keys().cloned().collect()
        } else if self.renderers.contains_key(&output_name) {
            vec![output_name]
        } else {
            error!(output = %output_name, "unknown output");
            return;
        };

        let cfg = infer_wallpaper_config(wallpaper);
        let prefer_gpu = self.config.prefer_gpu;
        let registry = self.registry.clone();

        // Collect surfaces before spawning so we don't borrow self across await.
        let work: Vec<(String, SurfaceDescriptor)> = targets
            .iter()
            .filter_map(|name| {
                self.wayland
                    .state
                    .outputs
                    .iter()
                    .find(|o| o.name == *name)
                    .map(|o| {
                        (
                            name.clone(),
                            SurfaceDescriptor {
                                output: o.clone(),
                                format: Rgba8Unorm,
                            },
                        )
                    })
            })
            .collect();

        // Clone the live renderer Arcs so we can swap from inside the task.
        let live_renderers: HashMap<String, DynRenderer> = work
            .iter()
            .filter_map(|(name, _)| {
                self.renderers
                    .get(name)
                    .map(|r| (name.clone(), Arc::clone(r)))
            })
            .collect();

        let cfg_clone = cfg;
        let wallpaper_clone = wallpaper.to_string();

        // Load all renderers in parallel on the blocking pool, then swap.
        // We spawn a tokio task so the run_loop `.await` returns immediately
        // and Wayland dispatch continues during the load.
        tokio::spawn(async move {
            let futs: Vec<_> = work
                .into_iter()
                .map(|(name, surface)| {
                    let cfg = cfg_clone.clone();
                    let registry = registry.clone();
                    async move {
                        let result =
                            build_renderer_non_blocking(Some(cfg), &surface, prefer_gpu, &registry)
                                .await;
                        (name, result)
                    }
                })
                .collect();

            let results = futures::future::join_all(futs).await;

            for (name, result) in results {
                match result {
                    Ok(new_arc) => {
                        // Arc::try_unwrap works because factory just created it.
                        if let Ok(mutex) = Arc::try_unwrap(new_arc) {
                            let new_box = mutex.into_inner();
                            if let Some(live) = live_renderers.get(&name) {
                                *live.lock() = new_box;
                                info!(
                                    output = %name,
                                    wallpaper = %wallpaper_clone,
                                    "renderer hot-swapped ✓"
                                );
                            }
                        } else {
                            error!(output = %name, "Arc still shared after factory");
                        }
                    }
                    Err(e) => error!(output = %name, error = %e, "renderer build failed"),
                }
            }
        });

        // Update wallpaper names optimistically (before load completes).
        for name in &targets {
            self.wallpaper_names
                .insert(name.clone(), wallpaper.to_string());
        }
    }

    async fn reload_all_renderers(&mut self) -> Result<()> {
        let outputs = self.wayland.state.outputs.clone();
        for output in outputs {
            let cfg = self
                .config
                .outputs
                .iter()
                .find(|o| o.name == output.name)
                .or_else(|| self.config.outputs.iter().find(|o| o.name == "*"))
                .map(|o| o.wallpaper.clone());
            let Some(cfg) = cfg else { continue };
            let surface = SurfaceDescriptor {
                output: output.clone(),
                format: Rgba8Unorm,
            };
            match build_renderer_non_blocking(
                Some(cfg.clone()),
                &surface,
                self.config.prefer_gpu,
                &self.registry,
            )
            .await
            {
                Ok(new_arc) => {
                    let new_box: Box<dyn Renderer> = Arc::try_unwrap(new_arc)
                        .map_err(|_| anyhow::anyhow!("Arc still shared"))?
                        .into_inner();
                    if let Some(live) = self.renderers.get(&output.name) {
                        *live.lock() = new_box;
                        self.wallpaper_names
                            .insert(output.name.clone(), wallpaper_label(&cfg));
                        info!(output = %output.name, "renderer reloaded");
                    }
                }
                Err(e) => error!(output = %output.name, error = %e, "reload failed"),
            }
        }
        Ok(())
    }

    async fn update_ipc_state(&self) {
        let total: u64 = self
            .frame_counters
            .values()
            .map(|c| c.load(Ordering::Relaxed))
            .sum();

        let outputs: Vec<OutputStatus> = self
            .wayland
            .state
            .outputs
            .iter()
            .map(|o| {
                let frames = self
                    .frame_counters
                    .get(&o.name)
                    .map_or(0, |c| c.load(Ordering::Relaxed));
                OutputStatus {
                    name: o.name.clone(),
                    resolution: format!("{}x{}", o.width, o.height),
                    wallpaper: self
                        .wallpaper_names
                        .get(&o.name)
                        .cloned()
                        .unwrap_or_else(|| "unknown".into()),
                    fps: frames as f32,
                }
            })
            .collect();

        for c in self.frame_counters.values() {
            c.store(0, Ordering::Relaxed);
        }

        let mut state = self.ipc_state.write().await;
        state.total_frames += total;
        state.outputs = outputs;
    }

    pub fn shutdown(self) {
        info!(
            count = self.render_threads.len(),
            "waiting for render threads"
        );
        for h in self.render_threads {
            let name = h.thread().name().unwrap_or("?").to_owned();
            if let Err(e) = h.join() {
                error!(thread = %name, "render thread panicked: {e:?}");
            }
        }
        info!("all render threads stopped");
    }
}

fn build_audio_source(cfg: &config_system::AudioConfig) -> Arc<dyn AudioSource> {
    if !cfg.enabled {
        info!("audio disabled in config");
        return Arc::new(SilentSource);
    }
    #[cfg(feature = "pipewire-audio")]
    {
        match audio_engine::pipewire_source::PipeWireSource::new(
            cfg.fft_size,
            cfg.device.as_deref(),
        ) {
            Ok(src) => {
                info!("PipeWire audio active");
                return Arc::new(src);
            }
            Err(e) => warn!(error = %e, "PipeWire unavailable, using silent source"),
        }
    }
    info!("using silent audio source");
    Arc::new(SilentSource)
}

async fn build_renderer_non_blocking(
    wallpaper_cfg: Option<config_system::WallpaperConfig>,
    surface: &SurfaceDescriptor,
    prefer_gpu: bool,
    registry: &ShaderRegistry,
) -> Result<DynRenderer> {
    match wallpaper_cfg {
        Some(config_system::WallpaperConfig::Image { path }) => {
            let surface = surface.clone();
            let path = crate::factory::shellexpand_path(&path);
            tokio::task::spawn_blocking(move || {
                let mut r = wallpaper_runtime::ImageRenderer::new(path, ScaleMode::Cover);
                r.init(&surface)?;
                let boxed: Box<dyn Renderer> = Box::new(r);
                Ok::<DynRenderer, anyhow::Error>(Arc::new(parking_lot::Mutex::new(boxed)))
            })
            .await
            .context("spawn_blocking panicked")?
        }
        Some(cfg) => factory::build_renderer(&cfg, surface, prefer_gpu, registry).await,
        None => Ok(make_default_renderer(surface, prefer_gpu, registry).await),
    }
}

async fn make_default_renderer(
    surface: &SurfaceDescriptor,
    prefer_gpu: bool,
    registry: &ShaderRegistry,
) -> DynRenderer {
    if prefer_gpu {
        match GpuRenderer::new(registry.clone()).await {
            Ok(mut r) => {
                if r.init(surface).is_ok() {
                    let _ = r.set_shader("audio_reactive");
                    info!(output = %surface.output.name, backend = "gpu", "default renderer");
                    return Arc::new(parking_lot::Mutex::new(Box::new(r) as Box<dyn Renderer>));
                }
            }
            Err(e) => warn!(error = %e, "GPU unavailable — CPU fallback"),
        }
    }
    info!(output = %surface.output.name, backend = "cpu", "default renderer");
    make_cpu_renderer(surface)
}

fn make_cpu_renderer(surface: &SurfaceDescriptor) -> DynRenderer {
    let mut r = CpuRenderer::default();
    let _ = r.init(surface);
    Arc::new(parking_lot::Mutex::new(Box::new(r) as Box<dyn Renderer>))
}

fn infer_wallpaper_config(s: &str) -> config_system::WallpaperConfig {
    let expanded = crate::factory::shellexpand_path(&std::path::PathBuf::from(s));
    let ext = expanded
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" => {
            config_system::WallpaperConfig::Image { path: expanded }
        }
        _ => config_system::WallpaperConfig::AudioReactive {
            path: expanded,
            bands: None,
        },
    }
}

fn wallpaper_label(cfg: &config_system::WallpaperConfig) -> String {
    match cfg {
        config_system::WallpaperConfig::Image { path } => path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("image")
            .to_owned(),
        config_system::WallpaperConfig::Shader { path, .. }
        | config_system::WallpaperConfig::AudioReactive { path, .. } => path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("shader")
            .to_owned(),
        config_system::WallpaperConfig::TimeBased { .. } => "time-based".to_owned(),
    }
}
