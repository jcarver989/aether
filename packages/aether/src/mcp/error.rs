use std::fmt;

#[derive(Debug)]
pub enum McpError {
    /// Tool not found in the registry
    ToolNotFound(String),
    /// Invalid tool name format (should be server__tool)
    InvalidToolNameFormat(String),
    /// MCP server not found
    ServerNotFound(String),
    /// Failed to execute tool on MCP server
    ToolExecutionFailed {
        tool_name: String,
        server_name: String,
        error: String,
    },
    /// Tool execution returned an error
    ToolExecutionError(String),
    /// Tool discovery failed
    ToolDiscoveryFailed(String),
    /// Server connection failed
    ConnectionFailed(String),
    /// Server startup failed
    ServerStartupFailed(String),
    /// Transport error
    TransportError(String),
    /// JSON serialization/deserialization error
    JsonError(String),
    /// Generic error for other cases
    Other(String),
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpError::ToolNotFound(tool) => write!(f, "Tool not found: {tool}"),
            McpError::InvalidToolNameFormat(name) => {
                write!(f, "Invalid tool name format: {name}")
            }
            McpError::ServerNotFound(server) => write!(f, "Server not found: {server}"),
            McpError::ToolExecutionFailed {
                tool_name,
                server_name,
                error,
            } => {
                write!(f, "Failed to execute tool {tool_name} on server {server_name}: {error}")
            }
            McpError::ToolExecutionError(msg) => write!(f, "Tool execution failed: {msg}"),
            McpError::ToolDiscoveryFailed(msg) => write!(f, "Tool discovery failed: {msg}"),
            McpError::ConnectionFailed(msg) => write!(f, "Connection failed: {msg}"),
            McpError::ServerStartupFailed(msg) => write!(f, "Server startup failed: {msg}"),
            McpError::TransportError(msg) => write!(f, "Transport error: {msg}"),
            McpError::JsonError(msg) => write!(f, "JSON error: {msg}"),
            McpError::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for McpError {}

impl From<serde_json::Error> for McpError {
    fn from(error: serde_json::Error) -> Self {
        McpError::JsonError(error.to_string())
    }
}

impl From<std::io::Error> for McpError {
    fn from(error: std::io::Error) -> Self {
        McpError::TransportError(error.to_string())
    }
}

impl From<rmcp::service::ClientInitializeError> for McpError {
    fn from(error: rmcp::service::ClientInitializeError) -> Self {
        McpError::ConnectionFailed(error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, McpError>;
