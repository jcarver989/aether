use std::collections::HashMap;
use serde_json::Value;
use rmcp::model::Tool as RmcpTool;

#[derive(Debug, Clone)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    pub server_name: String,
}

impl Tool {
    pub fn from_rmcp_tool(server_name: String, tool: RmcpTool) -> Self {
        Self {
            name: tool.name.to_string(),
            description: tool.description.unwrap_or_default().to_string(),
            parameters: serde_json::Value::Object((*tool.input_schema).clone()),
            server_name,
        }
    }
}

pub struct ToolRegistry {
    tools: HashMap<String, Tool>,
    tool_to_server: HashMap<String, String>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            tool_to_server: HashMap::new(),
        }
    }

    pub fn register_tool(&mut self, server_name: String, rmcp_tool: RmcpTool) {
        let tool_name = rmcp_tool.name.to_string();
        let tool = Tool::from_rmcp_tool(server_name.clone(), rmcp_tool);
        
        self.tools.insert(tool_name.clone(), tool);
        self.tool_to_server.insert(tool_name, server_name);
    }

    pub fn register_tools(&mut self, server_name: String, rmcp_tools: Vec<RmcpTool>) {
        for rmcp_tool in rmcp_tools {
            self.register_tool(server_name.clone(), rmcp_tool);
        }
    }

    pub fn get_tool(&self, name: &str) -> Option<&Tool> {
        self.tools.get(name)
    }

    pub fn get_server_for_tool(&self, tool_name: &str) -> Option<&String> {
        self.tool_to_server.get(tool_name)
    }

    pub fn list_tools(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    pub fn get_tool_description(&self, tool_name: &str) -> Option<String> {
        self.tools.get(tool_name).map(|tool| tool.description.clone())
    }

    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    pub fn as_json_schemas(&self) -> Vec<Value> {
        self.tools.values().map(|tool| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters
                }
            })
        }).collect()
    }

    pub fn get_tool_parameters(&self, tool_name: &str) -> Option<&Value> {
        self.tools.get(tool_name).map(|tool| &tool.parameters)
    }
}