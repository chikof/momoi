use anyhow::Result;
use common::{Command, DaemonStatus, Response, WallpaperError, WallpaperStatus};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Mutex, mpsc};

use crate::{DaemonState, WallpaperCommand};

pub async fn start(
    state: Arc<Mutex<DaemonState>>,
    wallpaper_tx: mpsc::UnboundedSender<WallpaperCommand>,
) -> Result<()> {
    let socket_path = common::get_socket_path();

    // Remove old socket if it exists
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    let listener = UnixListener::bind(&socket_path)?;
    log::info!("IPC server listening on: {}", socket_path.display());

    loop {
        // Check if we should exit
        if state.lock().await.should_exit {
            break;
        }

        // Accept connections with timeout
        let accept_result =
            tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept()).await;

        match accept_result {
            Ok(Ok((stream, _addr))) => {
                let state = state.clone();
                let tx = wallpaper_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_client(stream, state, tx).await {
                        log::error!("Error handling client: {}", e);
                    }
                });
            }
            Ok(Err(e)) => {
                log::error!("Error accepting connection: {}", e);
            }
            Err(_) => {
                // Timeout, continue loop to check exit condition
                continue;
            }
        }
    }

    // Clean up socket
    let _ = std::fs::remove_file(&socket_path);
    log::info!("IPC server stopped");
    Ok(())
}

async fn handle_client(
    stream: UnixStream,
    state: Arc<Mutex<DaemonState>>,
    wallpaper_tx: mpsc::UnboundedSender<WallpaperCommand>,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    while reader.read_line(&mut line).await? > 0 {
        let response = match serde_json::from_str::<Command>(&line) {
            Ok(command) => handle_command(command, &state, &wallpaper_tx).await,
            Err(e) => {
                log::warn!("Invalid command: {}", e);
                Response::Error(WallpaperError::Ipc(format!("Invalid command: {}", e)))
            }
        };

        // Send response
        let response_json = serde_json::to_string(&response)?;
        writer.write_all(response_json.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        line.clear();
    }

    Ok(())
}

async fn handle_command(
    command: Command,
    state: &Arc<Mutex<DaemonState>>,
    wallpaper_tx: &mpsc::UnboundedSender<WallpaperCommand>,
) -> Response {
    log::debug!("Handling command: {:?}", command);

    match command {
        Command::Ping => Response::Pong,

        Command::Query => {
            let state = state.lock().await;
            let status = DaemonStatus {
                version: env!("CARGO_PKG_VERSION").to_string(),
                uptime_secs: state.uptime_secs(),
                current_wallpapers: state
                    .wallpapers
                    .iter()
                    .map(|(output, wallpaper)| WallpaperStatus {
                        output: output.clone(),
                        wallpaper: wallpaper.clone(),
                    })
                    .collect(),
            };
            Response::Status(status)
        }

        Command::ListOutputs => {
            let state = state.lock().await;
            Response::Outputs(state.outputs.clone())
        }

        Command::SetWallpaper {
            path,
            output,
            transition,
            scale,
        } => {
            log::info!(
                "Setting wallpaper: {} on output: {:?} with scale: {:?}, transition: {:?}",
                path,
                output,
                scale,
                transition
            );

            // Validate file exists
            if !std::path::Path::new(&path).exists() {
                return Response::Error(WallpaperError::NotFound(format!(
                    "Wallpaper file not found: {}",
                    path
                )));
            }

            // Send command to Wayland manager
            let cmd = WallpaperCommand::SetImage {
                path,
                output,
                scale: scale.unwrap_or_default(),
                transition,
            };
            if let Err(e) = wallpaper_tx.send(cmd) {
                return Response::Error(WallpaperError::Ipc(format!(
                    "Failed to send command to Wayland manager: {}",
                    e
                )));
            }

            Response::Ok
        }

        Command::SetColor { color, output } => {
            log::info!("Setting color: {} on output: {:?}", color, output);

            // Validate color format
            if !is_valid_hex_color(&color) {
                return Response::Error(WallpaperError::Ipc(format!(
                    "Invalid color format: {}. Use hex format like #FF5733 or FF5733",
                    color
                )));
            }

            // Send command to Wayland manager
            let cmd = WallpaperCommand::SetColor { color, output };
            if let Err(e) = wallpaper_tx.send(cmd) {
                return Response::Error(WallpaperError::Ipc(format!(
                    "Failed to send command to Wayland manager: {}",
                    e
                )));
            }

            Response::Ok
        }

        Command::SetShader {
            shader,
            output,
            transition,
            params,
        } => {
            log::info!("Setting shader: {} on output: {:?}", shader, output);

            // Send command to Wayland manager
            let cmd = WallpaperCommand::SetShader {
                shader,
                output,
                transition,
                params,
            };
            if let Err(e) = wallpaper_tx.send(cmd) {
                return Response::Error(WallpaperError::Ipc(format!(
                    "Failed to send command to Wayland manager: {}",
                    e
                )));
            }

            Response::Ok
        }

        Command::SetOverlay {
            overlay,
            params,
            output,
        } => {
            log::info!("Setting overlay: {} on output: {:?}", overlay, output);

            // Convert common::OverlayParams to internal overlay_shader::OverlayParams
            let internal_params = if let Some(p) = params {
                crate::overlay_shader::OverlayParams {
                    intensity: p.intensity,
                    strength: p.strength,
                    ..Default::default()
                }
            } else {
                crate::overlay_shader::OverlayParams::default()
            };

            let cmd = WallpaperCommand::SetOverlay {
                overlay,
                params: internal_params,
                output,
            };

            if let Err(e) = wallpaper_tx.send(cmd) {
                return Response::Error(WallpaperError::Ipc(format!(
                    "Failed to send command: {}",
                    e
                )));
            }

            Response::Ok
        }

        Command::ClearOverlay { output } => {
            log::info!("Clearing overlay for output: {:?}", output);
            let cmd = WallpaperCommand::ClearOverlay { output };
            if let Err(e) = wallpaper_tx.send(cmd) {
                return Response::Error(WallpaperError::Ipc(format!(
                    "Failed to send command: {}",
                    e
                )));
            }
            Response::Ok
        }

        Command::Kill => {
            log::info!("Received kill command");
            // Set exit flag
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                std::process::exit(0);
            });
            Response::Ok
        }

        Command::PlaylistNext => {
            let mut state = state.lock().await;

            // Get transition config first
            let (transition, duration) = if let Some(ref config) = state.config {
                if let Some(ref playlist_cfg) = config.playlist {
                    (
                        playlist_cfg.transition.clone(),
                        playlist_cfg.transition_duration,
                    )
                } else {
                    ("fade".to_string(), 500)
                }
            } else {
                ("fade".to_string(), 500)
            };

            // Now work with playlist
            if let Some(ref mut playlist) = state.playlist {
                if let Some(next) = playlist.next() {
                    log::info!("Moving to next wallpaper: {:?}", next);
                    let next_path = next.to_path_buf();

                    // Drop the state lock before sending command
                    drop(state);

                    // Create transition type
                    let transition_type = common::TransitionType::Fade {
                        duration_ms: duration as u32,
                    };

                    // Send command to set the wallpaper
                    let cmd = WallpaperCommand::SetImage {
                        path: next_path.to_string_lossy().to_string(),
                        output: None,
                        scale: common::ScaleMode::Fill,
                        transition: Some(transition_type),
                    };

                    if let Err(e) = wallpaper_tx.send(cmd) {
                        return Response::Error(WallpaperError::Ipc(format!(
                            "Failed to send command: {}",
                            e
                        )));
                    }

                    Response::Ok
                } else {
                    Response::Error(WallpaperError::Ipc("Playlist is empty".to_string()))
                }
            } else {
                Response::Error(WallpaperError::Ipc("No playlist configured".to_string()))
            }
        }

        Command::PlaylistPrev => {
            let mut state = state.lock().await;

            // Get transition config first
            let (transition, duration) = if let Some(ref config) = state.config {
                if let Some(ref playlist_cfg) = config.playlist {
                    (
                        playlist_cfg.transition.clone(),
                        playlist_cfg.transition_duration,
                    )
                } else {
                    ("fade".to_string(), 500)
                }
            } else {
                ("fade".to_string(), 500)
            };

            // Now work with playlist
            if let Some(ref mut playlist) = state.playlist {
                if let Some(prev) = playlist.prev() {
                    log::info!("Moving to previous wallpaper: {:?}", prev);
                    let prev_path = prev.to_path_buf();

                    // Drop the state lock before sending command
                    drop(state);

                    // Create transition type
                    let transition_type = common::TransitionType::Fade {
                        duration_ms: duration as u32,
                    };

                    // Send command to set the wallpaper
                    let cmd = WallpaperCommand::SetImage {
                        path: prev_path.to_string_lossy().to_string(),
                        output: None,
                        scale: common::ScaleMode::Fill,
                        transition: Some(transition_type),
                    };

                    if let Err(e) = wallpaper_tx.send(cmd) {
                        return Response::Error(WallpaperError::Ipc(format!(
                            "Failed to send command: {}",
                            e
                        )));
                    }

                    Response::Ok
                } else {
                    Response::Error(WallpaperError::Ipc("Playlist is empty".to_string()))
                }
            } else {
                Response::Error(WallpaperError::Ipc("No playlist configured".to_string()))
            }
        }

        Command::PlaylistToggleShuffle => {
            let mut state = state.lock().await;
            if let Some(ref mut playlist) = state.playlist {
                playlist.toggle_shuffle();
                log::info!("Toggled shuffle mode");
                Response::Ok
            } else {
                Response::Error(WallpaperError::Ipc("No playlist configured".to_string()))
            }
        }

        Command::GetResources => {
            let state = state.lock().await;
            if let Some(ref stats) = state.resource_stats {
                Response::Resources(common::ResourceStatus {
                    performance_mode: state.performance_mode.clone(),
                    memory_mb: stats.memory_bytes / 1024 / 1024,
                    cpu_percent: stats.cpu_percent,
                    on_battery: stats.on_battery,
                    battery_percent: stats.battery_percent,
                })
            } else {
                Response::Error(WallpaperError::Ipc(
                    "Resource stats not yet available".to_string(),
                ))
            }
        }

        Command::SetPerformanceMode { mode } => Response::Error(WallpaperError::Ipc(
            "Setting performance mode not yet implemented (use config file)".to_string(),
        )),
    }
}

fn is_valid_hex_color(color: &str) -> bool {
    let color = color.trim_start_matches('#');
    (color.len() == 6 || color.len() == 8) && color.chars().all(|c| c.is_ascii_hexdigit())
}
