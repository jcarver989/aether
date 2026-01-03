use aether_lspd::{ClientError, LanguageId, LspClient, lockfile_path, socket_path};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Error type for daemon harness operations
#[derive(Debug)]
pub enum HarnessError {
    SpawnFailed(String),
    ClientError(ClientError),
    DaemonNotReady,
    KillFailed(String),
}

impl std::fmt::Display for HarnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HarnessError::SpawnFailed(e) => write!(f, "Failed to spawn daemon: {}", e),
            HarnessError::ClientError(e) => write!(f, "Client error: {}", e),
            HarnessError::DaemonNotReady => write!(f, "Daemon not ready after retries"),
            HarnessError::KillFailed(e) => write!(f, "Failed to kill daemon: {}", e),
        }
    }
}

impl std::error::Error for HarnessError {}

impl From<ClientError> for HarnessError {
    fn from(e: ClientError) -> Self {
        HarnessError::ClientError(e)
    }
}

/// Test harness for the LSP daemon
pub struct DaemonHarness {
    socket_path: PathBuf,
    lockfile_path: PathBuf,
    workspace_root: PathBuf,
    language: LanguageId,
}

impl DaemonHarness {
    /// Spawn a daemon for testing using connect_or_spawn
    pub async fn spawn(workspace_root: &Path, language: LanguageId) -> Result<Self, HarnessError> {
        let sock_path = socket_path(workspace_root, language);
        let lock_path = lockfile_path(&sock_path);

        let _ = fs::remove_file(&sock_path);
        let _ = fs::remove_file(&lock_path);

        let _client = LspClient::connect_or_spawn(workspace_root, language)
            .await
            .map_err(|e| HarnessError::SpawnFailed(e.to_string()))?;

        Ok(Self {
            socket_path: sock_path,
            lockfile_path: lock_path,
            workspace_root: workspace_root.to_path_buf(),
            language,
        })
    }

    /// Connect a client to the running daemon
    pub async fn connect(&self) -> Result<LspClient, HarnessError> {
        LspClient::connect(&self.socket_path, &self.workspace_root, self.language)
            .await
            .map_err(HarnessError::ClientError)
    }

    /// Wait for rust-analyzer to be ready (send probe requests until it responds)
    pub async fn wait_for_lsp_ready(
        client: &LspClient,
        test_uri: lsp_types::Uri,
        timeout_duration: Duration,
    ) -> Result<(), HarnessError> {
        let deadline = tokio::time::Instant::now() + timeout_duration;

        loop {
            if tokio::time::Instant::now() > deadline {
                return Err(HarnessError::DaemonNotReady);
            }

            match client.hover(test_uri.clone(), 0, 0).await {
                Ok(_) => return Ok(()),
                Err(ClientError::LspError { .. }) => {
                    return Ok(());
                }
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }

    /// Kill the daemon by reading PID from lockfile and sending SIGTERM
    pub async fn kill(&self) -> Result<(), HarnessError> {
        let pid_str = fs::read_to_string(&self.lockfile_path)
            .map_err(|e| HarnessError::KillFailed(format!("Failed to read lockfile: {}", e)))?;

        let pid: i32 = pid_str
            .trim()
            .parse()
            .map_err(|e| HarnessError::KillFailed(format!("Failed to parse PID: {}", e)))?;

        #[cfg(unix)]
        {
            let result = unsafe { libc::kill(pid, libc::SIGTERM) };
            if result != 0 {
                let err = std::io::Error::last_os_error();

                if err.raw_os_error() != Some(libc::ESRCH) {
                    return Err(HarnessError::KillFailed(format!(
                        "kill({}, SIGTERM) failed: {}",
                        pid, err
                    )));
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;

        let _ = fs::remove_file(&self.socket_path);

        Ok(())
    }
}

impl Drop for DaemonHarness {
    fn drop(&mut self) {
        if let Ok(pid_str) = fs::read_to_string(&self.lockfile_path)
            && let Ok(pid) = pid_str.trim().parse::<i32>()
        {
            #[cfg(unix)]
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
        }
        let _ = fs::remove_file(&self.socket_path);
        let _ = fs::remove_file(&self.lockfile_path);
    }
}
