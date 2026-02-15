use thiserror::Error;

use crate::mcp;

#[derive(Debug, Error)]
pub enum AgentError {
    /// MCP manager operation failed
    #[error("MCP error: {0}")]
    McpError(#[from] mcp::McpError),
    /// LLM provider error
    #[error("LLM error: {0}")]
    LlmError(#[from] crate::LlmError),
    /// IO error (file operations, etc.)
    #[error("IO error: {0}")]
    IoError(String),
    /// Generic error for other cases
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, AgentError>;
