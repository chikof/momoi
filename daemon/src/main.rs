mod buffer;
mod config;
mod gif_manager;
mod ipc_server;
mod macros;
mod overlay_shader;
mod playlist;
mod resource_monitor;
mod scheduler;
mod shader_manager;
mod transition;
mod video_manager;
mod wallpaper_manager;
mod wayland;

#[cfg(feature = "gpu")]
mod gpu;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!(
        "Starting Wayland Wallpaper Daemon v{}",
        env!("CARGO_PKG_VERSION")
    );

    // Load configuration
    let config_path = config::Config::default_config_path()?;
    log::info!("Looking for config at: {}", config_path.display());

    let config = match config::Config::load() {
        Ok(cfg) => {
            log::info!("✓ Configuration loaded successfully");
            log::info!("  General settings:");
            log::info!("    - Log level: {}", cfg.general.log_level);
            log::info!(
                "    - Default transition: {} ({}ms)",
                cfg.general.default_transition,
                cfg.general.default_duration
            );
            log::info!("    - Default scale: {}", cfg.general.default_scale);

            if let Some(ref playlist_cfg) = cfg.playlist {
                if playlist_cfg.enabled {
                    log::info!("  Playlist settings:");
                    log::info!("    - Enabled: yes");
                    log::info!("    - Interval: {}s", playlist_cfg.interval);
                    log::info!(
                        "    - Shuffle: {}",
                        if playlist_cfg.shuffle { "yes" } else { "no" }
                    );
                    log::info!("    - Sources: {} path(s)", playlist_cfg.sources.len());
                } else {
                    log::info!("  Playlist: disabled");
                }
            } else {
                log::info!("  Playlist: not configured");
            }

            if !cfg.schedule.is_empty() {
                log::info!("  Schedule settings:");
                log::info!("    - Entries: {}", cfg.schedule.len());
                for entry in &cfg.schedule {
                    log::info!(
                        "      - {}: {} to {}",
                        entry.name,
                        entry.start_time,
                        entry.end_time
                    );
                }
            } else {
                log::info!("  Schedule: not configured");
            }

            if !cfg.output.is_empty() {
                log::info!("  Per-output settings:");
                log::info!("    - Configured outputs: {}", cfg.output.len());
                for output in &cfg.output {
                    log::info!("      - {}", output.name);
                }
            } else {
                log::info!("  Per-output: not configured");
            }

            Some(cfg)
        }
        Err(e) => {
            log::warn!("Failed to load config: {}. Using defaults.", e);
            log::info!("To create a config file:");
            log::info!("  mkdir -p {}", config_path.parent().unwrap().display());
            log::info!("  cp config.toml.example {}", config_path.display());
            None
        }
    };

    // Initialize GPU rendering if available (feature-gated)
    #[cfg(feature = "gpu")]
    {
        log::info!("GPU rendering feature enabled, checking availability...");
        if gpu::is_available() {
            match gpu::GpuRenderer::new().await {
                Ok(renderer) => {
                    let caps = renderer.context().capabilities();
                    caps.log_info();
                    log::info!("✓ GPU rendering initialized successfully!");

                    // Test GPU rendering with a simple color
                    log::info!("Running GPU proof-of-concept test...");
                    match renderer.render_solid_color(800, 600, [100, 150, 200, 255]) {
                        Ok(buffer) => {
                            log::info!(
                                "✓ GPU test successful! Rendered {}x{} ({} bytes)",
                                800,
                                600,
                                buffer.len()
                            );
                            log::info!("GPU rendering is ready to use!");
                        }
                        Err(e) => {
                            log::warn!("GPU test failed: {}", e);
                            log::info!("Falling back to CPU rendering");
                        }
                    }
                    // Renderer is dropped here, we'll integrate it properly later
                }
                Err(e) => {
                    log::warn!("Failed to initialize GPU rendering: {}", e);
                    log::info!("Falling back to CPU rendering");
                }
            }
        } else {
            log::info!("No suitable GPU found, using CPU rendering");
        }
    }

    #[cfg(not(feature = "gpu"))]
    {
        log::info!("GPU rendering not compiled (build with --features gpu to enable)");
    }

    // Create channels for communication between IPC and Wayland
    let (wallpaper_tx, wallpaper_rx) = mpsc::unbounded_channel();

    // Create shared state
    let mut daemon_state = DaemonState::new();

    // Initialize playlist if configured
    if let Some(ref cfg) = config {
        if let Some(ref playlist_cfg) = cfg.playlist {
            if playlist_cfg.enabled {
                match playlist::PlaylistState::new(
                    &playlist_cfg.sources,
                    &playlist_cfg.extensions,
                    playlist_cfg.interval,
                    playlist_cfg.shuffle,
                    None, // Global playlist
                ) {
                    Ok(playlist) => {
                        log::info!("Playlist initialized with {} wallpapers", playlist.len());
                        daemon_state.playlist = Some(playlist);
                    }
                    Err(e) => {
                        log::error!("Failed to create playlist: {}", e);
                    }
                }
            }
        }

        // Initialize scheduler if configured
        if !cfg.schedule.is_empty() {
            let scheduler = scheduler::SchedulerState::new(cfg.schedule.clone());
            log::info!(
                "Scheduler initialized with {} entries",
                scheduler.entries().len()
            );
            daemon_state.scheduler = Some(scheduler);
        }

        daemon_state.config = config;
    }

    let state = Arc::new(Mutex::new(daemon_state));

    // Start IPC server
    let ipc_state = state.clone();
    let ipc_tx = wallpaper_tx.clone();
    let ipc_handle = tokio::spawn(async move {
        if let Err(e) = ipc_server::start(ipc_state, ipc_tx).await {
            log::error!("IPC server error: {}", e);
        }
    });

    // Start Wayland event loop
    let wayland_state = state.clone();
    let wayland_handle = tokio::spawn(async move {
        if let Err(e) = wayland::run(wayland_state, wallpaper_rx).await {
            log::error!("Wayland manager error: {}", e);
        }
    });

    // Set up signal handlers
    let signal_state = state.clone();
    tokio::spawn(async move {
        use tokio::signal::unix::{SignalKind, signal};

        let mut sigterm = signal(SignalKind::terminate()).expect("Failed to setup SIGTERM handler");
        let mut sigint = signal(SignalKind::interrupt()).expect("Failed to setup SIGINT handler");

        tokio::select! {
            _ = sigterm.recv() => {
                log::info!("Received SIGTERM, shutting down...");
            }
            _ = sigint.recv() => {
                log::info!("Received SIGINT, shutting down...");
            }
        }

        signal_state.lock().await.should_exit = true;
    });

    // Wait for either task to complete
    tokio::select! {
        _ = ipc_handle => {
            log::info!("IPC server stopped");
        }
        _ = wayland_handle => {
            log::info!("Wayland manager stopped");
        }
    }

    log::info!("Daemon shutting down");
    Ok(())
}

/// Commands sent from IPC to Wayland manager
#[derive(Debug, Clone)]
pub enum WallpaperCommand {
    SetImage {
        path: String,
        output: Option<String>,
        scale: common::ScaleMode,
        transition: Option<common::TransitionType>,
    },
    SetColor {
        color: String,
        output: Option<String>,
    },
    SetShader {
        shader: String,
        output: Option<String>,
        transition: Option<common::TransitionType>,
        params: Option<common::ShaderParams>,
    },
    SetOverlay {
        overlay: String,
        params: crate::overlay_shader::OverlayParams,
        output: Option<String>,
    },
    ClearOverlay {
        output: Option<String>,
    },
}

/// Type of wallpaper content being displayed
#[derive(Debug, Clone)]
pub enum WallpaperContent {
    Static,
    AnimatedGif { frame_count: usize },
    Video { duration_secs: Option<u64> },
    Shader { shader_name: String },
}

/// Shared daemon state
pub struct DaemonState {
    pub should_exit: bool,
    pub start_time: std::time::Instant,
    pub outputs: Vec<common::OutputInfo>,
    pub wallpapers: std::collections::HashMap<String, common::WallpaperType>,
    pub config: Option<config::Config>,
    pub playlist: Option<playlist::PlaylistState>,
    pub scheduler: Option<scheduler::SchedulerState>,
    pub performance_mode: String,
    pub resource_stats: Option<resource_monitor::ResourceStats>,
}

impl DaemonState {
    fn new() -> Self {
        Self {
            should_exit: false,
            start_time: std::time::Instant::now(),
            outputs: Vec::new(),
            wallpapers: std::collections::HashMap::new(),
            config: None,
            playlist: None,
            scheduler: None,
            performance_mode: "balanced".to_string(),
            resource_stats: None,
        }
    }

    pub fn uptime_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
}
