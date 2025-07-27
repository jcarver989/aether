pub mod client;
pub mod protocol;
pub mod registry;

pub use client::McpClient;
pub use protocol::{McpError, McpResult};
pub use registry::ToolRegistry;