use std::fmt;

#[derive(Debug)]
pub enum ToolError {
    /// Tool execution failed
    ExecutionFailed(String),
    /// Tool configuration error
    ConfigurationError(String),
    /// IO error during tool operation
    IoError(String),
    /// Generic error for other cases
    Other(String),
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolError::ExecutionFailed(msg) => write!(f, "Tool execution failed: {}", msg),
            ToolError::ConfigurationError(msg) => write!(f, "Tool configuration error: {}", msg),
            ToolError::IoError(msg) => write!(f, "IO error: {}", msg),
            ToolError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ToolError {}

impl From<std::io::Error> for ToolError {
    fn from(error: std::io::Error) -> Self {
        ToolError::IoError(error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, ToolError>;