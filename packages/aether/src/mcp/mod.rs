mod client;
pub mod error;
pub mod manager;

pub use manager::{ElicitationRequest, McpManager};
pub use error::{McpError, Result};
