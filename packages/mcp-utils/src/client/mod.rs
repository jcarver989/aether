pub mod config;
pub mod error;
pub mod manager;
pub mod mcp_client;
pub mod oauth;
pub mod roots;
pub mod variables;

pub use config::*;
pub use error::{McpError, Result};
pub use manager::{ElicitationRequest, McpManager, ServerInstructions};
pub use roots::root_from_path;
pub use variables::{VarError, expand_env_vars};

// Re-export rmcp's Root type for convenience
pub use rmcp::model::Root;
