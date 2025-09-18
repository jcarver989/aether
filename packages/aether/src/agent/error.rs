use std::fmt;

use crate::{llm, mcp};

#[derive(Debug)]
pub enum AgentError {
    /// MCP manager operation failed
    McpError(mcp::McpError),
    /// LLM provider error
    LlmError(llm::LlmError),
    /// Generic error for other cases
    Other(String),
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::McpError(e) => write!(f, "MCP error: {}", e),
            AgentError::LlmError(e) => write!(f, "LLM error: {}", e),
            AgentError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for AgentError {}

impl From<crate::mcp::McpError> for AgentError {
    fn from(error: crate::mcp::McpError) -> Self {
        AgentError::McpError(error)
    }
}

impl From<crate::llm::LlmError> for AgentError {
    fn from(error: crate::llm::LlmError) -> Self {
        AgentError::LlmError(error)
    }
}

pub type Result<T> = std::result::Result<T, AgentError>;
