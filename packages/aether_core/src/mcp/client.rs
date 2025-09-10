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

pub struct McpClient {
    servers: HashMap<String, McpServer>,
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

    pub async fn discover_tools(&self) -> Result<Vec<(String, RmcpTool)>> {
        let mut discovered_tools = Vec::new();

        for (server_name, server) in &self.servers {
            match server.client.list_tools(None).await {
                Ok(tools_response) => {
                    for tool in tools_response.tools {
                        discovered_tools.push((server_name.clone(), tool));
                    }
                }
                Err(_) => {
                    continue;
                }
            }
        }

        Ok(discovered_tools)
    }

    pub async fn execute_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        args: Value,
    ) -> Result<Value> {
        let server = self
            .servers
            .get(server_name)
            .ok_or_else(|| color_eyre::Report::msg(format!("Server not found: {server_name}")))?;

        let arguments = args.as_object().cloned();
        let request = CallToolRequestParam {
            name: tool_name.to_string().into(),
            arguments,
        };

        let result = match server.client.call_tool(request).await {
            Ok(result) => result,
            Err(e) => {
                return Err(color_eyre::Report::msg(format!(
                    "Failed to execute tool {tool_name} on server {server_name}: {e}"
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
}
