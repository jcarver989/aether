pub mod mcp_builder;
pub mod run_mcp_task;
pub mod tool_bridge;

// Re-export submodules from mcp_utils::client for backward compatibility
pub use mcp_utils::client::config;
pub use mcp_utils::client::error;
pub use mcp_utils::client::manager;
pub use mcp_utils::client::oauth;
pub use mcp_utils::client::roots;
pub use mcp_utils::client::variables;

// Re-export key types at the mcp module level
pub use mcp_utils::client::Root;
pub use mcp_utils::client::config::*;
pub use mcp_utils::client::error::{McpError, Result};
pub use mcp_utils::client::manager::{ElicitationRequest, McpManager, ServerInstructions};
pub use mcp_utils::client::mcp_client::McpClient;
pub use mcp_utils::client::roots::root_from_path;
pub use mcp_utils::client::variables::{VarError, expand_env_vars};

pub use mcp_builder::*;
