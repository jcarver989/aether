use crate::client_handler::handle_client;
use crate::error::{DaemonError, DaemonResult};
use crate::lockfile::Lockfile;
use crate::lsp_manager::spawn_lsp_manager;
use std::fs::{create_dir_all, remove_file};
use std::future::pending;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::net::UnixListener;
use tokio::select;
use tokio::spawn;
use tokio::sync::{RwLock, mpsc};
use tokio::time::sleep;
use uuid::Uuid;

/// Run the daemon until shutdown
pub async fn run_daemon(socket_path: PathBuf, idle_timeout: Option<Duration>) -> DaemonResult<()> {
    if let Some(parent) = socket_path.parent() {
        create_dir_all(parent).map_err(DaemonError::Io)?;
    }

    // Acquire lock BEFORE removing socket to avoid disrupting a running daemon
    let lockfile_path = socket_path.with_extension("lock");
    let _lockfile =
        Lockfile::acquire(&lockfile_path).map_err(|e| DaemonError::LockfileError(e.to_string()))?;

    // Only remove stale socket after we have the lock
    let _ = remove_file(&socket_path);

    let listener = UnixListener::bind(&socket_path).map_err(DaemonError::BindFailed)?;
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let tx = shutdown_tx.clone();
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
            let _ = tx.send(()).await;
        });
    }

    let lsp_manager = spawn_lsp_manager();
    let client_count = Arc::new(AtomicUsize::new(0));
    let last_activity = Arc::new(RwLock::new(Instant::now()));
    tracing::info!("Daemon listening on {:?}", socket_path);

    loop {
        select! {
            biased;

            _ = shutdown_rx.recv() => {
                tracing::info!("Shutting down");
                break;
            }

            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => {
                        let client_id = Uuid::new_v4();
                        let manager = lsp_manager.clone();
                        let count = Arc::clone(&client_count);
                        let activity = Arc::clone(&last_activity);

                        count.fetch_add(1, Ordering::Relaxed);
                        *activity.write().await = Instant::now();

                        spawn(async move {
                            handle_client(stream, manager, client_id).await;
                            count.fetch_sub(1, Ordering::Relaxed);
                            *activity.write().await = Instant::now();
                            tracing::debug!("Client {} handler complete", client_id);
                        });
                    }
                    Err(e) => {
                        tracing::warn!("Failed to accept connection: {}", e);
                    }
                }
            }

            _ = check_idle_timeout(&client_count, &last_activity, idle_timeout) => {
                tracing::info!("Idle timeout reached, shutting down");
                break;
            }
        }
    }

    tracing::info!("Shutting down LSP servers");
    lsp_manager.shutdown().await;
    let _ = remove_file(&socket_path);

    tracing::info!("Daemon shutdown complete");
    Ok(())
}

/// Wait until idle timeout is reached
async fn check_idle_timeout(
    client_count: &AtomicUsize,
    last_activity: &RwLock<Instant>,
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
