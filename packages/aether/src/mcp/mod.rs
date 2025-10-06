mod client;
pub mod error;
pub mod manager;
pub mod run_mcp_task;

pub use error::{McpError, Result};
pub use manager::{ElicitationRequest, McpManager};
