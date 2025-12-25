//! LSP-specific error types

use thiserror::Error;

/// Errors that can occur during LSP operations
#[derive(Debug, Error)]
pub enum LspError {
    /// Failed to spawn the language server process
    #[error("Failed to spawn language server: {0}")]
    SpawnFailed(#[from] std::io::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Server returned an error response
    #[error("Server error (code={code}): {message}")]
    ServerError { code: i32, message: String },

    /// Transport layer error (e.g., reading/writing to stdio)
    #[error("Transport error: {0}")]
    Transport(String),

    /// Server initialization failed
    #[error("Server initialization failed: {0}")]
    InitializationFailed(String),

    /// Timeout waiting for a response
    #[error("Timeout waiting for response")]
    Timeout,

    /// Invalid message format (e.g., missing Content-Length header)
    #[error("Invalid message format: {0}")]
    InvalidMessage(String),

    /// Request was cancelled
    #[error("Request cancelled")]
    Cancelled,
}

impl LspError {
    /// Create a transport error for when the handler task is closed
    pub fn handler_closed() -> Self {
        Self::Transport("Handler task closed".into())
    }

    /// Create a transport error for when the response channel is closed
    pub fn response_closed() -> Self {
        Self::Transport("Response channel closed".into())
    }
}

/// Result type alias for LSP operations
pub type Result<T> = std::result::Result<T, LspError>;
