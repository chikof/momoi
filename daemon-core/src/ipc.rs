//! Unix-socket IPC server.
//!
//! Commands and responses are single-line JSON (`\n`-terminated).
//!
//! Stateless queries (`Status`, `ListOutputs`) are answered directly from
//! `IpcState`.  Mutating commands (`Reload`, `SetWallpaper`, `Quit`) are forwarded
//! to the orchestrator over `cmd_tx: mpsc::Sender<OrchestratorCmd>`.

use ipc_protocol::{Command, DaemonStatus, OutputStatus, Response, socket_path};
use std::sync::Arc;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::{RwLock, mpsc},
};
use tracing::{debug, error, info, warn};

/// Commands the IPC server forwards to the orchestrator run-loop.
#[derive(Debug)]
pub enum OrchestratorCmd {
    /// Reload configuration and shaders from disk.
    Reload,
    /// Switch the named output to a new wallpaper path/name.
    /// `"*"` targets all outputs.
    SetWallpaper { output: String, wallpaper: String },
    /// Graceful shutdown.
    Quit,
}

/// Shared daemon state written by render threads, read by IPC handlers.
#[derive(Debug, Default)]
pub struct IpcState {
    /// Total frames rendered across all outputs.
    pub total_frames: u64,
    /// Per-output status snapshot.
    pub outputs: Vec<OutputStatus>,
    /// Whether audio capture is currently active.
    pub audio_active: bool,
    /// Set `true` by Reload command; orchestrator clears after acting.
    #[allow(unused)]
    pub pending_reload: bool,
}

/// Bind the socket and serve connections until the process exits.
///
/// `cmd_tx` forwards mutating commands to the orchestrator run-loop.
pub async fn start_ipc_server(state: Arc<RwLock<IpcState>>, cmd_tx: mpsc::Sender<OrchestratorCmd>) {
    let path = socket_path();
    let _ = tokio::fs::remove_file(&path).await;

    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            error!(path = %path.display(), error = %e, "IPC socket bind failed");
            return;
        }
    };

    info!(path = %path.display(), "IPC server listening");

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let state = Arc::clone(&state);
                let cmd_tx = cmd_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_client(stream, state, cmd_tx).await {
                        debug!(error = %e, "IPC client error");
                    }
                });
            }
            Err(e) => warn!(error = %e, "IPC accept error"),
        }
    }
}

async fn handle_client(
    stream: UnixStream,
    state: Arc<RwLock<IpcState>>,
    cmd_tx: mpsc::Sender<OrchestratorCmd>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (reader, mut writer) = stream.into_split();
    let mut line = String::new();
    BufReader::new(reader).read_line(&mut line).await?;

    let response = match serde_json::from_str::<Command>(line.trim()) {
        Ok(cmd) => dispatch(cmd, &state, &cmd_tx).await,
        Err(e) => Response::Error {
            message: format!("invalid command JSON: {e}"),
        },
    };

    let mut out = serde_json::to_string(&response)?;
    out.push('\n');
    writer.write_all(out.as_bytes()).await?;
    Ok(())
}

async fn dispatch(
    cmd: Command,
    state: &Arc<RwLock<IpcState>>,
    cmd_tx: &mpsc::Sender<OrchestratorCmd>,
) -> Response {
    match cmd {
        Command::Status => {
            let s = state.read().await;
            Response::Status(DaemonStatus {
                version: env!("CARGO_PKG_VERSION").into(),
                active_outputs: s.outputs.len(),
                total_frames: s.total_frames,
                audio_active: s.audio_active,
            })
        }

        Command::ListOutputs => {
            let s = state.read().await;
            Response::Outputs(s.outputs.clone())
        }

        Command::Reload => {
            info!("reload requested via IPC");
            match cmd_tx.try_send(OrchestratorCmd::Reload) {
                Ok(()) => Response::Ok {
                    message: Some("reload queued".into()),
                },
                Err(_) => Response::Error {
                    message: "command channel full".into(),
                },
            }
        }

        Command::SetWallpaper { output, wallpaper } => {
            info!(output = %output, wallpaper = %wallpaper, "set-wallpaper requested");
            match cmd_tx.try_send(OrchestratorCmd::SetWallpaper {
                output: output.clone(),
                wallpaper: wallpaper.clone(),
            }) {
                Ok(()) => Response::Ok {
                    message: Some(format!("switching {output} to {wallpaper}")),
                },
                Err(_) => Response::Error {
                    message: "command channel full".into(),
                },
            }
        }

        Command::Quit => {
            info!("quit requested via IPC");
            let _ = cmd_tx.try_send(OrchestratorCmd::Quit);
            Response::Ok {
                message: Some("shutting down".into()),
            }
        }
    }
}
