use color_eyre::{Report, Result};
use rmcp::{
    RoleClient, RoleServer, ServiceExt,
    model::{
        CallToolRequestParam, ClientCapabilities, ClientInfo, Implementation,
        InitializeRequestParam, Tool as RmcpTool,
    },
    serve_client,
    service::{DynService, RunningService},
    transport::{
        StreamableHttpClientTransport, TokioChildProcess,
        streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::Value;
use std::collections::HashMap;

use crate::{transport::create_in_memory_transport, types::ToolDefinition};
use tokio::process::Command;

pub enum McpServerConfig {
    Http {
        name: String,
        config: StreamableHttpClientTransportConfig,
    },

    Stdio {
        name: String,
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    },

    InMemory {
        name: String,
        server: Box<dyn DynService<RoleServer>>,
    },
}

pub struct McpManager {
    servers: HashMap<String, McpServerConnection>,
    tools: HashMap<String, Tool>, // Now keyed by "server_name::tool_name"
    client_info: ClientInfo,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
            tools: HashMap::new(),
            client_info: ClientInfo {
                protocol_version: Default::default(),
                capabilities: ClientCapabilities::default(),
                client_info: Implementation {
                    name: "aether".to_string(),
                    version: "0.1.0".to_string(),
                    title: None,
                    icons: None,
                    website_url: None,
                },
            },
        }
    }

    pub async fn add_mcp(&mut self, config: McpServerConfig) -> Result<()> {
        use McpServerConfig::*;
        match config {
            Http { name, config } => {
                let transport = StreamableHttpClientTransport::from_config(config.clone());
                let client = serve_client(self.client_info.clone(), transport)
                    .await
                    .map_err(|e| {
                        Report::msg(format!("Failed to connect to HTTP MCP server {name}: {e}"))
                    })?;

                let server_connection = McpServerConnection {
                    _name: name.clone(),
                    client,
                    server_task: None,
                };

                self.servers.insert(name, server_connection);
                Ok(())
            }

            Stdio {
                name,
                command,
                args,
                env: _env,
            } => {
                let cmd = {
                    let mut cmd = Command::new(&command);
                    cmd.args(&args);
                    cmd
                };

                let client = self
                    .client_info
                    .clone()
                    .serve(TokioChildProcess::new(cmd)?)
                    .await?;

                self.servers.insert(
                    name.clone(),
                    McpServerConnection {
                        _name: name.clone(),
                        client,
                        server_task: None,
                    },
                );

                Ok(())
            }

            InMemory { name, server } => {
                let (client_transport, server_transport) = create_in_memory_transport();

                let server_handle = tokio::spawn(async move {
                    match server.serve(server_transport).await {
                        Ok(_service) => {
                            // Keep the service running (it will run until the transport is dropped)
                            std::future::pending::<()>().await;
                        }
                        Err(e) => {
                            eprintln!("MCP server error: {}", e);
                        }
                    }
                });

                let client = serve_client(self.client_info.clone(), client_transport)
                    .await
                    .map_err(|e| {
                        Report::msg(format!(
                            "Failed to connect to in-memory MCP server {name}: {e}"
                        ))
                    })?;

                let server_connection = McpServerConnection {
                    _name: name.clone(),
                    client,
                    server_task: Some(server_handle),
                };

                self.servers.insert(name, server_connection);

                Ok(())
            }
        }
    }

    pub async fn discover_tools(&mut self) -> Result<()> {
        self.tools.clear();

        for (server_name, server) in &self.servers {
            match server.client.list_tools(None).await {
                Ok(tools_response) => {
                    for rmcp_tool in tools_response.tools {
                        let tool_name = rmcp_tool.name.to_string();
                        let namespaced_tool_name = format!("{server_name}__{tool_name}");
                        let tool = Tool::from(rmcp_tool);

                        self.tools.insert(namespaced_tool_name, tool);
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
        // Check if the tool exists first
        if !self.tools.contains_key(namespaced_tool_name) {
            return Err(color_eyre::Report::msg(format!(
                "Tool not found: {namespaced_tool_name}"
            )));
        }

        let (server_name, original_tool_name) =
            namespaced_tool_name.split_once("__").ok_or_else(|| {
                color_eyre::Report::msg(format!("Invalid tool name format: {namespaced_tool_name}"))
            })?;

        let server = self
            .servers
            .get(server_name)
            .ok_or_else(|| color_eyre::Report::msg(format!("Server not found: {server_name}")))?;

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
            .map(|(namespaced_tool_name, tool)| {
                let server_name = namespaced_tool_name
                    .split("__")
                    .next()
                    .map(|s| s.to_string());

                ToolDefinition {
                    name: namespaced_tool_name.clone(),
                    description: tool.description.clone(),
                    parameters: tool.parameters.to_string(),
                    server: server_name,
                }
            })
            .collect()
    }

    /// Shutdown all servers and wait for their tasks to complete
    pub async fn shutdown(&mut self) {
        for (server_name, server) in self.servers.drain() {
            if let Some(handle) = server.server_task {
                // Drop the client first to signal shutdown
                drop(server.client);

                // Wait for the server task to complete (with a timeout)
                match tokio::time::timeout(std::time::Duration::from_secs(5), handle).await {
                    Ok(Ok(())) => {
                        println!("Server '{}' shut down gracefully", server_name);
                    }
                    Ok(Err(e)) => {
                        eprintln!("Server '{}' task panicked: {:?}", server_name, e);
                    }
                    Err(_) => {
                        eprintln!("Server '{}' shutdown timed out", server_name);
                        // Task will be cancelled when the handle is dropped
                    }
                }
            }
        }

        // Clear all cached data
        self.tools.clear();
    }

    /// Shutdown a specific server by name
    pub async fn shutdown_server(&mut self, server_name: &str) -> Result<()> {
        if let Some(server) = self.servers.remove(server_name) {
            if let Some(handle) = server.server_task {
                // Drop the client first to signal shutdown
                drop(server.client);

                // Wait for the server task to complete (with a timeout)
                match tokio::time::timeout(std::time::Duration::from_secs(5), handle).await {
                    Ok(Ok(())) => {
                        println!("Server '{}' shut down gracefully", server_name);
                    }
                    Ok(Err(e)) => {
                        eprintln!("Server '{}' task panicked: {:?}", server_name, e);
                    }
                    Err(_) => {
                        eprintln!("Server '{}' shutdown timed out", server_name);
                        // Task will be cancelled when the handle is dropped
                    }
                }
            }

            // Remove tools from this server
            self.tools
                .retain(|tool_name, _| !tool_name.starts_with(&format!("{}__", server_name)));
        }

        Ok(())
    }
}

impl Drop for McpManager {
    fn drop(&mut self) {
        // Cancel all server tasks when the client is dropped
        for (server_name, server) in self.servers.drain() {
            if let Some(handle) = server.server_task {
                handle.abort();
                eprintln!("Server '{}' task aborted during cleanup", server_name);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Tool {
    pub description: String,
    pub parameters: Value,
}

struct McpServerConnection {
    _name: String,
    client: RunningService<RoleClient, InitializeRequestParam>,
    server_task: Option<tokio::task::JoinHandle<()>>,
}

impl From<RmcpTool> for Tool {
    fn from(tool: RmcpTool) -> Self {
        Self {
            description: tool.description.unwrap_or_default().to_string(),
            parameters: serde_json::Value::Object((*tool.input_schema).clone()),
        }
    }
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{FileServerMcp, InMemoryFileSystem};

    #[tokio::test]
    async fn test_in_memory_transport_integration() {
        // Create a file system and server
        let filesystem = InMemoryFileSystem::new();
        let server = FileServerMcp::new(filesystem);
        let dyn_service = Box::new(server.into_dyn());

        // Create client and connect in-memory server directly
        let mut client = McpManager::new();
        let mcp_config = McpServerConfig::InMemory {
            name: "test_server".to_string(),
            server: dyn_service,
        };
        client.add_mcp(mcp_config).await.unwrap();

        // Discover tools
        client.discover_tools().await.unwrap();

        // Verify tools were discovered
        let tool_definitions = client.get_tool_definitions();
        assert!(!tool_definitions.is_empty());

        // Check for the write_file tool
        let write_file_tool = tool_definitions
            .iter()
            .find(|t| t.name.contains("write_file"));
        assert!(write_file_tool.is_some());

        // Test tool execution
        let args = serde_json::json!({
            "path": "/test.txt",
            "content": "Hello, World!"
        });

        let result = client
            .execute_tool("test_server__write_file", args)
            .await
            .unwrap();

        // Verify the result
        let result_text = result
            .get("text")
            .and_then(|t| t.as_str())
            .expect("Result should contain text field");
        assert!(result_text.contains("Successfully wrote"));
    }
}
