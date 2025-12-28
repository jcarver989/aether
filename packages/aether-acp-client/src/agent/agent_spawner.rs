use crate::{DockerAgentProcess, LocalAgentProcess};
use async_trait::async_trait;
use bollard::models::Mount;
use futures::io::{AsyncRead, AsyncWrite};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Input stream for sending data to an agent.
pub type AgentInput = Pin<Box<dyn AsyncWrite + Send + Unpin>>;

/// Output stream for receiving data from an agent.
pub type AgentOutput = Pin<Box<dyn AsyncRead + Send + Unpin>>;

/// Progress during Docker container startup.
#[derive(Clone, Debug, PartialEq)]
pub enum DockerProgress {
    /// Checking if image exists locally
    CheckingImage,
    /// Building image from Dockerfile
    BuildingImage,
    /// Pulling image from registry
    PullingImage,
    /// Creating overlay filesystem volumes
    CreatingVolumes,
    /// Starting the container
    StartingContainer,
    /// Initializing the ACP session
    Initializing,
}

impl DockerProgress {
    /// Get human-readable text for this phase.
    pub fn text(&self) -> &'static str {
        match self {
            DockerProgress::CheckingImage => "Checking image...",
            DockerProgress::BuildingImage => "Building image...",
            DockerProgress::PullingImage => "Pulling image...",
            DockerProgress::CreatingVolumes => "Creating volumes...",
            DockerProgress::StartingContainer => "Starting container...",
            DockerProgress::Initializing => "Initializing...",
        }
    }
}

/// Sender for Docker progress updates.
pub type ProgressTx = mpsc::UnboundedSender<DockerProgress>;

/// Receiver for Docker progress updates.
pub type ProgressRx = mpsc::UnboundedReceiver<DockerProgress>;

/// Specifies how to obtain the Docker image for the container.
#[derive(Debug, Clone)]
pub enum ImageSource {
    /// Use a pre-built image, pulling from registry if needed.
    Image(String),
    /// Build an image from a Dockerfile.
    Dockerfile(PathBuf),
}

/// Configuration for containerized agent.
#[derive(Debug, Clone)]
pub struct DockerConfig {
    /// Docker image source (pre-built image or Dockerfile).
    pub image: ImageSource,
    /// Additional volume mounts.
    pub mounts: Vec<Mount>,
    /// Environment variables.
    pub env: HashMap<String, String>,
    /// Mount SSH keys for git push capability.
    pub mount_ssh_keys: bool,
    /// Working directory inside container.
    pub working_dir: String,
}

/// Configuration for spawning an agent.
#[derive(Debug, Clone)]
pub enum SpawnConfig {
    /// Spawn as a local process.
    Local,
    /// Spawn in a Docker container.
    Docker(DockerConfig),
}

/// Spawn an agent with the given configuration.
///
/// Returns the agent handle (for lifecycle management) and IO streams
/// (for communication via ClientSideConnection).
///
/// For Docker agents, progress updates are sent through the optional `progress_tx` channel.
pub async fn spawn_agent_process(
    config: SpawnConfig,
    project_path: &Path,
    cmd: Vec<String>,
    progress_tx: Option<ProgressTx>,
) -> Result<(Arc<dyn AgentProcess>, AgentInput, AgentOutput), AgentError> {
    use SpawnConfig::*;
    match config {
        Local => {
            let (agent, input, output) = LocalAgentProcess::spawn(project_path, cmd).await?;
            Ok((Arc::new(agent), input, output))
        }

        Docker(docker_config) => {
            let (agent, input, output) =
                DockerAgentProcess::spawn(&docker_config, project_path, cmd, progress_tx).await?;
            Ok((Arc::new(agent), input, output))
        }
    }
}

/// Errors that can occur when spawning or managing agents.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// Docker/container error
    #[error("Container error: {0}")]
    Container(#[from] crate::error::ContainerError),

    /// Docker API error (direct from bollard)
    #[error("Docker error: {0}")]
    Docker(#[from] bollard::errors::Error),

    /// Process spawn error
    #[error("Failed to spawn process: {0}")]
    Spawn(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Agent not running
    #[error("Agent is not running")]
    NotRunning,
}

/// A spawned agent with lifecycle management.
///
/// This trait provides lifecycle operations (terminate, exec, etc.) that can
/// be used with `Arc<dyn Agent>`. IO streams are accessed directly on concrete
/// types via `into_io()` since they have different types per implementation.
#[async_trait]
pub trait AgentProcess: Send + Sync {
    /// Terminate the agent and clean up resources.
    ///
    /// Attempts graceful shutdown first, then force kills after timeout.
    /// For local processes: sends SIGTERM, waits, then SIGKILL if needed.
    /// For containers: calls `docker stop`, then removes the container.
    async fn terminate(&self, timeout_secs: i64) -> Result<(), AgentError>;

    /// Get a unique identifier for this agent.
    fn id(&self) -> &str;

    /// Execute a command in the agent's environment and return stdout.
    ///
    /// For local processes, this runs the command in the project directory.
    /// For containers, this uses `docker exec`.
    async fn exec(&self, cmd: Vec<String>) -> Result<String, AgentError>;

    /// Get the project path where the agent is running.
    fn project_path(&self) -> &Path;
}
