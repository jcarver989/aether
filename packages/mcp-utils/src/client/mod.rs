pub mod config;
pub mod connection;
pub mod error;
pub mod manager;
pub mod mcp_client;
pub mod naming;
pub use llm::oauth;
pub mod roots;
pub mod tool_proxy;
pub mod variables;

pub use config::*;
pub use connection::ServerInstructions;
pub use error::{McpError, Result};
pub use manager::{ElicitationRequest, McpManager, McpServerStatus, McpServerStatusEntry};
pub use naming::split_on_server_name;
pub use roots::root_from_path;
pub use variables::{VarError, expand_env_vars};

// Re-export rmcp's Root type for convenience
pub use rmcp::model::Root;

use std::path::PathBuf;

/// Resolve the Aether home directory.
///
/// Returns `$AETHER_HOME` if set, otherwise `$HOME/.aether` (or `$USERPROFILE/.aether`
/// on Windows). Returns `None` if no home directory environment variable is set.
pub(crate) fn aether_home() -> Option<PathBuf> {
    match std::env::var("AETHER_HOME") {
        Ok(value) if !value.trim().is_empty() => Some(PathBuf::from(value)),
        _ => {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .ok()?;
            Some(PathBuf::from(home).join(".aether"))
        }
    }
}
