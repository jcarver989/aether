mod client;
pub mod error;
pub mod manager;

pub use error::{McpError, Result};
pub use manager::{ElicitationRequest, McpManager};
