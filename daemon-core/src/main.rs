//! Momoi daemon entry point.

use anyhow::{Context, Result};
use config_system::ConfigLoader;
use ipc_protocol::socket_path;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, watch};
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};

mod factory;
mod ipc;
mod orchestrator;

use ipc::{IpcState, OrchestratorCmd, start_ipc_server};
use orchestrator::Orchestrator;

#[tokio::main]
async fn main() -> Result<()> {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("momoi=info,warn")),
        )
        .init();

    info!(version = env!("CARGO_PKG_VERSION"), "momoi starting");
    info!(socket = %socket_path().display(), "IPC socket path");

    let config = ConfigLoader::load_or_default().context("failed to load configuration")?;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let ipc_state = Arc::new(RwLock::new(IpcState::default()));

    // Channel from IPC server → orchestrator run-loop.
    let (cmd_tx, cmd_rx) = mpsc::channel::<OrchestratorCmd>(32);

    let mut orchestrator =
        Orchestrator::new(config, shutdown_rx.clone(), cmd_rx, Arc::clone(&ipc_state))
            .context("orchestrator init failed")?;

    orchestrator
        .start()
        .await
        .context("failed to start render threads")?;

    // IPC server runs as a background task.
    let ipc_state_clone = Arc::clone(&ipc_state);
    tokio::spawn(async move {
        start_ipc_server(ipc_state_clone, cmd_tx).await;
    });

    // Wait for the Wayland loop to end or a signal to arrive.
    let _shutdown_tx_sig = shutdown_tx.clone();
    tokio::select! {
        res = orchestrator.run_loop() => {
            if let Err(e) = res {
                tracing::error!(error = %e, "event loop error");
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("SIGINT received");
        }
        () = async {
            let mut s = tokio::signal::unix::signal(
                tokio::signal::unix::SignalKind::terminate()
            ).expect("SIGTERM handler");
            s.recv().await;
        } => {
            info!("SIGTERM received");
        }
    }

    let _ = shutdown_tx.send(true);
    orchestrator.shutdown();
    let _ = tokio::fs::remove_file(socket_path()).await;
    info!("momoi stopped");
    Ok(())
}
