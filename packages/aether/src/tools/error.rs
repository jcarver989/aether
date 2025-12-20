use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolError {
    /// Tool execution failed
    #[error("Tool execution failed: {0}")]
    ExecutionFailed(String),
    /// Tool configuration error
    #[error("Tool configuration error: {0}")]
    ConfigurationError(String),
    /// IO error during tool operation
    #[error("IO error: {0}")]
    IoError(String),
    /// Generic error for other cases
    #[error("{0}")]
    Other(String),
}

impl From<std::io::Error> for ToolError {
    fn from(error: std::io::Error) -> Self {
        ToolError::IoError(error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, ToolError>;
