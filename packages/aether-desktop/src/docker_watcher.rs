//! File change detection for Docker containers via polling.
//!
//! Since inotify doesn't work across container boundaries, we poll for changes
//! by periodically running `git status` inside the container.

use aether_acp_client::AgentProcess;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Polling interval for detecting changes in Docker containers.
const POLL_INTERVAL: Duration = Duration::from_secs(3);

/// Events emitted by the Docker file poller.
#[derive(Debug, Clone)]
pub enum DockerFileEvent {
    /// Files have changed in the container.
    Changed,
    /// An error occurred while polling.
    Error(String),
}

/// A poller that detects file changes inside a Docker container.
pub struct DockerFilePoller {
    handle: Arc<dyn AgentProcess>,
    tx: mpsc::UnboundedSender<DockerFileEvent>,
    shutdown_rx: Option<tokio::sync::oneshot::Receiver<()>>,
}

impl DockerFilePoller {
    /// Create a new Docker file poller.
    ///
    /// Returns the poller and a sender that can be used to shut it down.
    pub fn new(
        handle: Arc<dyn AgentProcess>,
        tx: mpsc::UnboundedSender<DockerFileEvent>,
    ) -> (Self, tokio::sync::oneshot::Sender<()>) {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        (
            Self {
                handle,
                tx,
                shutdown_rx: Some(shutdown_rx),
            },
            shutdown_tx,
        )
    }

    /// Start polling for file changes.
    ///
    /// This consumes the poller and spawns a background task that polls
    /// for changes at regular intervals.
    pub fn start(mut self) {
        let handle = self.handle;
        let tx = self.tx;
        let shutdown_rx = self.shutdown_rx.take().expect("shutdown_rx already taken");

        tokio::spawn(async move {
            // Use Option to distinguish "no baseline yet" from "baseline is empty string"
            let mut last_status: Option<String> = None;
            let mut interval = tokio::time::interval(POLL_INTERVAL);

            tokio::pin!(shutdown_rx);

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        match get_git_status(&handle).await {
                            Ok(status) => {
                                // Only emit Changed after first poll establishes baseline
                                if let Some(prev) = &last_status
                                    && status != *prev
                                {
                                    let _ = tx.send(DockerFileEvent::Changed);
                                }
                                last_status = Some(status);
                            }
                            Err(e) => {
                                let _ = tx.send(DockerFileEvent::Error(e));
                            }
                        }
                    }
                    _ = &mut shutdown_rx => {
                        break;
                    }
                }
            }
        });
    }
}

/// Get the current git status from inside the container.
async fn get_git_status(handle: &Arc<dyn AgentProcess>) -> Result<String, String> {
    handle
        .exec(vec![
            "git".to_string(),
            "status".to_string(),
            "--porcelain".to_string(),
        ])
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poll_interval() {
        // Verify the poll interval is reasonable (1-5 seconds for docker exec)
        assert!(POLL_INTERVAL.as_secs() >= 1);
        assert!(POLL_INTERVAL.as_secs() <= 5);
    }
}
