use crate::client_handler::handle_client;
use crate::error::{DaemonError, DaemonResult};
use crate::lsp_manager::spawn_lsp_manager;
use crate::pid_lockfile::PidLockfile;
use std::fs::{create_dir_all, remove_file};
use std::future::pending;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::net::UnixListener;
use tokio::select;
use tokio::spawn;
use tokio::sync::{RwLock, oneshot};
use tokio::time::sleep;
use uuid::Uuid;

/// Run the daemon until shutdown
pub async fn run_daemon(socket_path: PathBuf, idle_timeout: Option<Duration>) -> DaemonResult<()> {
    if let Some(parent) = socket_path.parent() {
        create_dir_all(parent).map_err(DaemonError::Io)?;
    }

    {
        let _ = PidLockfile::acquire(&socket_path.with_extension("lock"))
            .map_err(|e| DaemonError::LockfileError(e.to_string()))?;

        let _ = remove_file(&socket_path);
    }

    let shutdown_rx = spawn_shutdown_signal_handler();
    let lsp_manager = spawn_lsp_manager();

    tracing::info!("Daemon listening on {:?}", socket_path);
    run_listener_loop(socket_path.clone(), shutdown_rx, &lsp_manager, idle_timeout).await?;

    tracing::info!("Shutting down LSP servers");
    lsp_manager.shutdown().await;

    let _ = remove_file(&socket_path);
    tracing::info!("Daemon shutdown complete");

    Ok(())
}

/// Main listener loop that handles connections and shutdown signals
async fn run_listener_loop(
    socket_path: PathBuf,
    mut shutdown_rx: oneshot::Receiver<()>,
    lsp_manager: &crate::lsp_manager::LspManagerHandle,
    idle_timeout: Option<Duration>,
) -> DaemonResult<()> {
    let listener = UnixListener::bind(&socket_path).map_err(DaemonError::BindFailed)?;
    let client_count = Arc::new(AtomicUsize::new(0));
    let last_activity = Arc::new(RwLock::new(Instant::now()));
    loop {
        select! {
            biased;

            _ = &mut shutdown_rx => {
                tracing::info!("Shutting down");
                return Ok(());
            }

            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => {
                        let client_id = Uuid::new_v4();
                        let manager = lsp_manager.clone();
                        let client_count = Arc::clone(&client_count);
                        let last_activity = Arc::clone(&last_activity);

                        client_count.fetch_add(1, Ordering::Relaxed);
                        *last_activity.write().await = Instant::now();

                        spawn(async move {
                            handle_client(stream, manager, client_id).await;
                            client_count.fetch_sub(1, Ordering::Relaxed);
                            *last_activity.write().await = Instant::now();
                            tracing::debug!("Client {} handler complete", client_id);
                        });
                    }
                    Err(e) => {
                        tracing::warn!("Failed to accept connection: {}", e);
                    }
                }
            }

            _ = check_idle_timeout(client_count.clone(), last_activity.clone(), idle_timeout) => {
                tracing::info!("Idle timeout reached, shutting down");
                return Ok(());
            }
        }
    }
}

/// Wait until idle timeout is reached
async fn check_idle_timeout(
    client_count: Arc<AtomicUsize>,
    last_activity: Arc<RwLock<Instant>>,
    timeout: Option<Duration>,
) {
    let Some(timeout) = timeout else {
        pending::<()>().await;
        return;
    };

    loop {
        sleep(Duration::from_secs(10)).await;

        let count = client_count.load(Ordering::Relaxed);
        if count > 0 {
            continue;
        }

        let last = *last_activity.read().await;
        if last.elapsed() >= timeout {
            return;
        }
    }
}

/// Spawn a task to handle shutdown signals (SIGTERM, SIGINT)
fn spawn_shutdown_signal_handler() -> oneshot::Receiver<()> {
    let (tx, rx) = oneshot::channel::<()>();

    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        spawn(async move {
            let mut sigterm =
                signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");

            let mut sigint =
                signal(SignalKind::interrupt()).expect("Failed to register SIGINT handler");

            select! {
                _ = sigterm.recv() => {
                    tracing::info!("Received SIGTERM");
                }
                _ = sigint.recv() => {
                    tracing::info!("Received SIGINT");
                }
            }
            let _ = tx.send(());
        });
    }

    rx
}
