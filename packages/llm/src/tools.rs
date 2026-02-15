use serde::{Deserialize, Serialize};

// Re-export tool types from agent-events
pub use agent_events::{ToolCallError, ToolCallRequest, ToolCallResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: String,
    pub server: Option<String>,
}
