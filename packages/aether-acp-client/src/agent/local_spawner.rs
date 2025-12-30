use super::agent_spawner::{AgentError, AgentInput, AgentOutput, AgentProcess};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tracing::debug;

/// A local process agent handle for lifecycle management.
///
/// This struct is `Sync` and implements `Agent` for use with `Arc<dyn Agent>`.
/// IO streams are returned separately from `spawn()`.
pub struct LocalAgentProcess {
    id: String,
    child: Arc<Mutex<Child>>,
    project_path: PathBuf,
}

impl LocalAgentProcess {
    /// Spawn a new agent as a local process.
    ///
    /// Returns the agent handle and IO streams separately. The handle implements
    /// `Agent` and can be used with `Arc<dyn Agent>`. The IO streams are for
    /// passing to `ClientSideConnection::new()`.
    pub async fn spawn(
        project_path: &Path,
        cmd: Vec<String>,
    ) -> Result<(Self, AgentInput, AgentOutput), AgentError> {
        if cmd.is_empty() {
            return Err(AgentError::Spawn("Empty command".to_string()));
        }

        let (command, args) = (&cmd[0], &cmd[1..]);
        debug!("Spawning local process: {} {:?}", command, args);
        let mut child = Command::new(command)
            .args(args)
            .current_dir(project_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| AgentError::Spawn(e.to_string()))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AgentError::Spawn("Failed to get stdin".to_string()))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentError::Spawn("Failed to get stdout".to_string()))?;

        let agent_id = uuid::Uuid::new_v4().to_string();

        let agent = Self {
            id: agent_id,
            child: Arc::new(Mutex::new(child)),
            project_path: project_path.to_path_buf(),
        };

        let input: AgentInput = Box::pin(stdin.compat_write());
        let output: AgentOutput = Box::pin(stdout.compat());

        Ok((agent, input, output))
    }
}

#[async_trait]
impl AgentProcess for LocalAgentProcess {
    async fn terminate(&self, timeout_secs: i64) -> Result<(), AgentError> {
        debug!("Terminating local agent {}", self.id);

        let mut child = self.child.lock().await;

        #[cfg(unix)]
        {
            use nix::sys::signal::{Signal, kill};
            use nix::unistd::Pid;

            if let Some(pid) = child.id() {
                let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
                let timeout = tokio::time::Duration::from_secs(timeout_secs as u64);
                match tokio::time::timeout(timeout, child.wait()).await {
                    Ok(Ok(_)) => {
                        debug!("Local agent {} stopped gracefully", self.id);
                        return Ok(());
                    }
                    Ok(Err(e)) => {
                        return Err(AgentError::Io(e));
                    }
                    Err(_) => {
                        debug!(
                            "Local agent {} did not stop within timeout, force killing",
                            self.id
                        );
                        child.kill().await.map_err(AgentError::Io)?;
                        let _ = child.wait().await;
                    }
                }
            }
        }

        #[cfg(not(unix))]
        {
            child.kill().await.map_err(AgentError::Io)?;
            let _ = child.wait().await;
        }

        Ok(())
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn exec(&self, cmd: Vec<String>) -> Result<String, AgentError> {
        if cmd.is_empty() {
            return Err(AgentError::Spawn("Empty command".to_string()));
        }

        let (command, args) = (&cmd[0], &cmd[1..]);
        debug!(
            "Executing command in {}: {} {:?}",
            self.project_path.display(),
            command,
            args
        );

        let output = Command::new(command)
            .args(args)
            .current_dir(&self.project_path)
            .output()
            .await
            .map_err(|e| AgentError::Spawn(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AgentError::Spawn(format!(
                "Command failed with status {}: {}",
                output.status, stderr
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn project_path(&self) -> &Path {
        &self.project_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_spawn_echo() {
        let temp_dir = tempfile::tempdir().unwrap();
        let result = LocalAgentProcess::spawn(
            temp_dir.path(),
            vec!["echo".to_string(), "hello".to_string()],
        )
        .await;

        assert!(result.is_ok());
        let (agent, _input, _output) = result.unwrap();
        assert!(!agent.id().is_empty());
    }

    #[tokio::test]
    async fn test_spawn_empty_command() {
        let temp_dir = tempfile::tempdir().unwrap();
        let result = LocalAgentProcess::spawn(temp_dir.path(), vec![]).await;

        assert!(result.is_err());
        if let Err(AgentError::Spawn(msg)) = result {
            assert_eq!(msg, "Empty command");
        }
    }
}
