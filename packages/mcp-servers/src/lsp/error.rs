//! LSP-specific error types

use aether_lspd::ClientError;
use thiserror::Error;

/// Errors that can occur during LSP operations
#[derive(Debug, Error)]
pub enum LspError {
    /// I/O error (e.g., reading a file)
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// LSP client / daemon communication error
    #[error(transparent)]
    Client(#[from] ClientError),

    /// Transport or protocol error
    #[error("Transport error: {0}")]
    Transport(String),
}

/// Result type alias for LSP operations
pub type Result<T> = std::result::Result<T, LspError>;
