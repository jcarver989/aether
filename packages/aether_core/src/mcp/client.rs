use color_eyre::Result;
use rmcp::{
    RoleClient,
    model::{
        CallToolRequestParam, ClientCapabilities, ClientInfo, Implementation,
        InitializeRequestParam, Tool as RmcpTool,
    },
    service::RunningService,
    transport::StreamableHttpClientTransport,
};
use serde_json::Value;
use std::collections::HashMap;

use crate::mcp::mcp_config::McpServerConfig;
use crate::types::ToolDefinition;

#[derive(Debug, Clone)]
pub struct Tool {
    pub description: String,
    pub parameters: Value,
}

impl From<RmcpTool> for Tool {
    fn from(tool: RmcpTool) -> Self {
        Self {
            description: tool.description.unwrap_or_default().to_string(),
            parameters: serde_json::Value::Object((*tool.input_schema).clone()),
        }
    }
}

pub struct McpClient {
    servers: HashMap<String, McpServer>,
    tools: HashMap<String, Tool>, // Now keyed by "server_name::tool_name"
    tool_to_server: HashMap<String, String>, // Maps namespaced tool name to server name
}

struct McpServer {
    client: RunningService<RoleClient, InitializeRequestParam>,
}

impl Default for McpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl McpClient {
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
            tools: HashMap::new(),
            tool_to_server: HashMap::new(),
        }
    }

    pub async fn connect_server(&mut self, name: String, config: McpServerConfig) -> Result<()> {
        match config {
            McpServerConfig::Http { url, .. } => {
                let transport = StreamableHttpClientTransport::from_uri(url.clone());
                self.connect_http_server(name, transport).await
            }
            McpServerConfig::Stdio { command, args, .. } => Err(color_eyre::Report::msg(format!(
                "Process-based MCP servers not yet implemented: {} {}",
                command,
                args.join(" ")
            ))),
        }
    }

    async fn connect_http_server(
        &mut self,
        name: String,
        transport: StreamableHttpClientTransport<reqwest::Client>,
    ) -> Result<()> {
        let client_info = ClientInfo {
            protocol_version: Default::default(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "aether".to_string(),
                version: "0.1.0".to_string(),
            },
        };

        let client = rmcp::serve_client(client_info, transport)
            .await
            .map_err(|e| {
                color_eyre::Report::msg(format!("Failed to connect to HTTP MCP server {name}: {e}"))
            })?;

        let server = McpServer { client };
        self.servers.insert(name.clone(), server);

        Ok(())
    }

    pub async fn discover_tools(&mut self) -> Result<()> {
        self.tools.clear();
        self.tool_to_server.clear();

        for (server_name, server) in &self.servers {
            match server.client.list_tools(None).await {
                Ok(tools_response) => {
                    for rmcp_tool in tools_response.tools {
                        let tool_name = rmcp_tool.name.to_string();
                        let namespaced_tool_name = format!("{server_name}::{tool_name}");
                        let tool = Tool::from(rmcp_tool);

                        self.tools.insert(namespaced_tool_name.clone(), tool);
                        self.tool_to_server
                            .insert(namespaced_tool_name, server_name.clone());
                    }
                }
                Err(_) => {
                    continue;
                }
            }
        }

        Ok(())
    }

    pub async fn execute_tool(&self, namespaced_tool_name: &str, args: Value) -> Result<Value> {
        let server_name = self
            .tool_to_server
            .get(namespaced_tool_name)
            .ok_or_else(|| {
                color_eyre::Report::msg(format!("Tool not found: {namespaced_tool_name}"))
            })?;

        let server = self
            .servers
            .get(server_name)
            .ok_or_else(|| color_eyre::Report::msg(format!("Server not found: {server_name}")))?;

        // Extract the original tool name from the namespaced name (server_name::tool_name)
        let original_tool_name = namespaced_tool_name.split("::").nth(1).ok_or_else(|| {
            color_eyre::Report::msg(format!("Invalid tool name format: {namespaced_tool_name}"))
        })?;

        let arguments = args.as_object().cloned();
        let request = CallToolRequestParam {
            name: original_tool_name.to_string().into(),
            arguments,
        };

        let result = match server.client.call_tool(request).await {
            Ok(result) => result,
            Err(e) => {
                return Err(color_eyre::Report::msg(format!(
                    "Failed to execute tool {original_tool_name} on server {server_name}: {e}"
                )));
            }
        };

        if result.is_error.unwrap_or(false) {
            let error_msg = result
                .content
                .first()
                .map(|content| format!("{content:?}"))
                .unwrap_or_else(|| "Unknown error".to_string());
            return Err(color_eyre::Report::msg(format!(
                "Tool execution failed: {error_msg}"
            )));
        }

        let result_value = result
            .content
            .first()
            .map(|content| {
                serde_json::to_value(content)
                    .unwrap_or(Value::String("Serialization error".to_string()))
            })
            .unwrap_or_else(|| Value::String("No result".to_string()));

        Ok(result_value)
    }

    pub fn get_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .map(|(namespaced_tool_name, tool)| ToolDefinition {
                name: namespaced_tool_name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.to_string(),
                server: self.tool_to_server.get(namespaced_tool_name).cloned(),
            })
            .collect()
    }
}
