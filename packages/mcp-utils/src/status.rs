use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum McpServerStatus {
    Connected { tool_count: usize },
    Failed { error: String },
    NeedsOAuth,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpServerStatusEntry {
    pub name: String,
    pub status: McpServerStatus,
}
