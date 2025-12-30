/// Errors that can occur when working with Docker containers.
#[derive(Debug, thiserror::Error)]
pub enum ContainerError {
    /// Docker API error
    #[error("Docker error: {0}")]
    Docker(#[from] bollard::errors::Error),

    /// Image build failed
    #[error("Image build failed: {0}")]
    ImageBuild(String),

    /// Failed to attach to container
    #[error("Failed to attach to container: {0}")]
    AttachFailed(String),

    /// Failed to mount volume
    #[error("Failed to mount volume: {0}")]
    MountFailed(String),

    /// Container not found
    #[error("Container not found: {0}")]
    ContainerNotFound(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Container startup timeout
    #[error("Container startup timeout")]
    StartupTimeout,

    /// Container exited unexpectedly
    #[error("Container exited with code: {0}")]
    ContainerExited(i64),
}

pub type Result<T> = std::result::Result<T, ContainerError>;
