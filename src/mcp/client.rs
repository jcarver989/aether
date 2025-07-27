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
use tracing::{debug, error, info};

use crate::mcp_config::McpServerConfig;

pub struct McpClient {
    servers: HashMap<String, McpServer>,
}

struct McpServer {
    client: RunningService<RoleClient, InitializeRequestParam>,
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
                info!("Connecting to HTTP MCP server: {}", name);

                // Create HTTP transport
                let transport = StreamableHttpClientTransport::from_uri(url.clone());
                self.connect_http_server(name, transport).await
            }
            McpServerConfig::Process { command, args, .. } => {
                info!(
                    "Connecting to process MCP server: {} (command: {})",
                    name, command
                );
                // For now, return an error as process servers aren't fully implemented
                Err(color_eyre::Report::msg(format!(
                    "Process-based MCP servers not yet implemented: {} {}",
                    command,
                    args.join(" ")
                )))
            }
        }
    }

    async fn connect_http_server(
        &mut self,
        name: String,
        transport: StreamableHttpClientTransport<reqwest::Client>,
    ) -> Result<()> {
        // Configure client info
        let client_info = ClientInfo {
            protocol_version: Default::default(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "aether".to_string(),
                version: "0.1.0".to_string(),
            },
        };

        // Initialize the MCP client
        let client = rmcp::serve_client(client_info, transport)
            .await
            .map_err(|e| {
                color_eyre::Report::msg(format!(
                    "Failed to connect to HTTP MCP server {}: {}",
                    name, e
                ))
            })?;

        let server = McpServer { client };

        self.servers.insert(name.clone(), server);
        debug!("Successfully connected to HTTP MCP server: {}", name);

        Ok(())
    }

    pub async fn discover_tools(&self) -> Result<Vec<(String, RmcpTool)>> {
        info!("Discovering tools from all connected servers");
        let mut discovered_tools = Vec::new();

        for (server_name, server) in &self.servers {
            debug!("Discovering tools from server: {}", server_name);

            match server.client.list_tools(None).await {
                Ok(tools_response) => {
                    for tool in tools_response.tools {
                        debug!("Found tool: {} from server: {}", tool.name, server_name);
                        discovered_tools.push((server_name.clone(), tool));
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to discover tools from server {}: {}",
                        server_name, e
                    );
                    // Don't fail the entire discovery process for one server
                    continue;
                }
            }
        }

        info!(
            "Tool discovery completed. Found {} tools total",
            discovered_tools.len()
        );
        Ok(discovered_tools)
    }

    pub async fn execute_tool(&self, server_name: &str, tool_name: &str, args: Value) -> Result<Value> {
        debug!("Executing tool: {} on server: {} with args: {}", tool_name, server_name, args);

        // Log to file helper
        fn log_debug(msg: &str) {
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("/tmp/aether_debug.log")
            {
                use std::io::Write;
                let _ = writeln!(
                    file,
                    "[{}] MCP: {}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    msg
                );
            }
        }

        log_debug(&format!(
            "Executing tool: {} on server: {} with args: {}",
            tool_name, server_name, args
        ));

        let server = self
            .servers
            .get(server_name)
            .ok_or_else(|| color_eyre::Report::msg(format!("Server not found: {}", server_name)))?;

        let arguments = args.as_object().cloned();
        let request = CallToolRequestParam {
            name: tool_name.to_string().into(),
            arguments,
        };

        log_debug("Sending tool request to server...");

        let result = match server.client.call_tool(request).await {
            Ok(result) => result,
            Err(e) => {
                log_debug(&format!("Tool call failed with error: {:?}", e));
                return Err(color_eyre::Report::msg(format!(
                    "Failed to execute tool {} on server {}: {}",
                    tool_name, server_name, e
                )));
            }
        };

        log_debug(&format!("Got response: {:?}", result));

        if result.is_error.unwrap_or(false) {
            let error_msg = result
                .content
                .first()
                .map(|content| format!("{:?}", content))
                .unwrap_or_else(|| "Unknown error".to_string());
            return Err(color_eyre::Report::msg(format!(
                "Tool execution failed: {}",
                error_msg
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

        log_debug(&format!("Returning result value: {:?}", result_value));

        Ok(result_value)
    }

}
