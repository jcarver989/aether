use aether_core::{agent::Agent, llm::LlmProvider, tools::ToolRegistry, types::McpServerStatus};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as TokioMutex;

pub struct AgentState {
    pub agent: Arc<TokioMutex<Option<Agent<Box<dyn LlmProvider>>>>>,
    pub tool_registry: Arc<Mutex<ToolRegistry>>,
    pub mcp_server_status: Arc<Mutex<HashMap<String, McpServerStatus>>>,
}

impl AgentState {
    pub fn set_tool_registry(&self, tool_registry: ToolRegistry) {
        let mut tool_registry_guard = self.tool_registry.lock().unwrap();
        *tool_registry_guard = tool_registry;
    }
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            agent: Arc::new(TokioMutex::new(None)),
            tool_registry: Arc::new(Mutex::new(ToolRegistry::new())),
            mcp_server_status: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl AgentState {
    /// Update the status of an MCP server
    pub fn update_mcp_server_status(&self, server_id: String, status: McpServerStatus) {
        let mut status_map = self.mcp_server_status.lock().unwrap();
        status_map.insert(server_id, status);
    }

    /// Get the status of all MCP servers
    pub fn get_mcp_server_statuses(&self) -> HashMap<String, McpServerStatus> {
        let status_map = self.mcp_server_status.lock().unwrap();
        status_map.clone()
    }

    /// Remove MCP server status
    pub fn remove_mcp_server_status(&self, server_id: &str) {
        let mut status_map = self.mcp_server_status.lock().unwrap();
        status_map.remove(server_id);
    }
}
