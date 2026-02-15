#![doc = include_str!("../README.md")]

pub mod agent;
pub mod auth;
pub mod context;
pub mod fs;
pub mod mcp;
pub mod testing;
pub mod tools;
pub mod transport;
pub mod types;

// Re-export the llm crate so `aether::llm::*` paths continue to work
pub use llm;

// Convenience re-exports at crate root (used by internal modules)
pub use llm::{
    ChatMessage, Context, LlmError, LlmResponse, LlmResponseStream, ProviderFactory,
    StreamingModelProvider, ToolCallError, ToolCallRequest, ToolCallResult, ToolDefinition,
};

// Re-export rmcp types needed by consumers
pub use rmcp::model::{CreateElicitationRequestParams, CreateElicitationResult, ElicitationAction};
