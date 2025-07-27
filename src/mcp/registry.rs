use rmcp::model::Tool as RmcpTool;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use color_eyre::Result;
use super::client::McpClient;

#[derive(Debug, Clone)]
pub struct Tool {
    pub description: String,
    pub parameters: Value,
}

impl Tool {
    pub fn from_rmcp_tool(_server_name: String, tool: RmcpTool) -> Self {
        Self {
            description: tool.description.unwrap_or_default().to_string(),
            parameters: serde_json::Value::Object((*tool.input_schema).clone()),
        }
    }
}

#[derive(Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, Tool>,
    tool_to_server: HashMap<String, String>,
    mcp_client: Option<Arc<McpClient>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            tool_to_server: HashMap::new(),
            mcp_client: None,
        }
    }

    pub fn register_tool(&mut self, server_name: String, rmcp_tool: RmcpTool) {
        let tool_name = rmcp_tool.name.to_string();
        let tool = Tool::from_rmcp_tool(server_name.clone(), rmcp_tool);

        self.tools.insert(tool_name.clone(), tool);
        self.tool_to_server.insert(tool_name, server_name);
    }

    pub fn get_server_for_tool(&self, tool_name: &str) -> Option<&String> {
        self.tool_to_server.get(tool_name)
    }

    pub fn list_tools(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    pub fn get_tool_description(&self, tool_name: &str) -> Option<String> {
        self.tools
            .get(tool_name)
            .map(|tool| tool.description.clone())
    }

    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    pub fn get_tool_parameters(&self, tool_name: &str) -> Option<&Value> {
        self.tools.get(tool_name).map(|tool| &tool.parameters)
    }

    /// Set the main MCP client
    pub fn set_mcp_client(&mut self, client: Arc<McpClient>) {
        self.mcp_client = Some(client);
    }

    /// Remove the MCP client
    pub fn remove_mcp_client(&mut self) {
        self.mcp_client = None;
    }

    /// Invoke a tool using the MCP client
    pub async fn invoke_tool(&self, tool_name: &str, args: Value) -> Result<Value> {
        // Check if the tool exists in our registry
        if !self.tools.contains_key(tool_name) {
            return Err(color_eyre::Report::msg(format!("Tool not found in registry: {}", tool_name)));
        }

        // Get the server name for this tool
        let server_name = self.get_server_for_tool(tool_name)
            .ok_or_else(|| color_eyre::Report::msg(format!("Server not found for tool: {}", tool_name)))?;

        // Get the MCP client
        let mcp_client = self.mcp_client.as_ref()
            .ok_or_else(|| color_eyre::Report::msg("No MCP client available"))?;

        // Delegate to the MCP client for execution with server name
        mcp_client.execute_tool(server_name, tool_name, args).await
    }
}
