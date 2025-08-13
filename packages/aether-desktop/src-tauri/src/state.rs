use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use aether_core::{
    agent::Agent,
    llm::LlmProvider,
    tools::ToolRegistry,
    types::McpServerStatus,
};

pub struct AgentState {
    pub agent: Arc<Mutex<Option<Agent<Box<dyn LlmProvider>>>>>,
    pub tool_registry: Arc<Mutex<ToolRegistry>>,
    pub mcp_server_status: Arc<Mutex<HashMap<String, McpServerStatus>>>,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            agent: Arc::new(Mutex::new(None)),
            tool_registry: Arc::new(Mutex::new(ToolRegistry::new())),
            mcp_server_status: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl AgentState {
    /// Update the status of an MCP server
    pub async fn update_mcp_server_status(&self, server_id: String, status: McpServerStatus) {
        let mut status_map = self.mcp_server_status.lock().await;
        status_map.insert(server_id, status);
    }

    /// Get the status of all MCP servers
    pub async fn get_mcp_server_statuses(&self) -> HashMap<String, McpServerStatus> {
        let status_map = self.mcp_server_status.lock().await;
        status_map.clone()
    }

    /// Remove MCP server status
    pub async fn remove_mcp_server_status(&self, server_id: &str) {
        let mut status_map = self.mcp_server_status.lock().await;
        status_map.remove(server_id);
    }
}