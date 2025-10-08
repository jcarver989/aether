mod client;
pub mod config;
pub mod error;
pub mod manager;
pub mod parser;
pub mod run_mcp_task;
pub mod server_registry;
pub mod variables;

pub use config::{McpConfig, ServerDefinition};
pub use error::{McpError, Result};
pub use manager::{ElicitationRequest, McpManager};
pub use parser::{McpConfigParser, ParseError};
pub use server_registry::{McpServerRegistry, RegistryError, ServerFactory};
pub use variables::{VarError, expand_env_vars};
