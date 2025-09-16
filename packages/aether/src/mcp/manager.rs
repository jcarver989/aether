use color_eyre::{Report, Result};
use rmcp::{
    RoleClient, RoleServer, ServiceExt,
    model::{
        CallToolRequestParam, ClientCapabilities, ClientInfo, CreateElicitationRequestParam,
        CreateElicitationResult, ElicitationAction, ElicitationCapability, Implementation,
        Tool as RmcpTool,
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

use crate::{mcp::client::McpClient, transport::create_in_memory_transport, types::ToolDefinition};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};

const TOOL_NAMESPACE_DELIMITER: &str = "__";

#[derive(Debug)]
pub struct ElicitationRequest {
    pub request: CreateElicitationRequestParam,
    pub response_sender: oneshot::Sender<CreateElicitationResult>,
}

#[derive(Debug, Clone)]
pub struct ElicitationResponse {
    pub action: ElicitationAction,
    pub content: Option<Value>,
}

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

/// Manages connections to multiple MCP servers and their tools
pub struct McpManager {
    servers: HashMap<String, McpServerConnection>,
    tools: HashMap<String, Tool>, // Now keyed by "server_name::tool_name"
    client_info: ClientInfo,
    elicitation_sender: mpsc::UnboundedSender<ElicitationRequest>,
}

impl McpManager {
    pub fn new(elicitation_sender: mpsc::UnboundedSender<ElicitationRequest>) -> Self {
        Self {
            servers: HashMap::new(),
            tools: HashMap::new(),
            client_info: ClientInfo {
                protocol_version: Default::default(),
                capabilities: ClientCapabilities {
                    elicitation: Some(ElicitationCapability {
                        schema_validation: Some(true),
                    }),
                    ..ClientCapabilities::default()
                },
                client_info: Implementation {
                    name: "aether".to_string(),
                    version: "0.1.0".to_string(),
                    title: None,
                    icons: None,
                    website_url: None,
                },
            },
            elicitation_sender,
        }
    }

    fn create_mcp_client(&self) -> McpClient {
        McpClient::new(self.client_info.clone(), self.elicitation_sender.clone())
    }

    pub async fn add_mcp(&mut self, config: McpServerConfig) -> Result<()> {
        use McpServerConfig::*;
        match config {
            Http { name, config } => {
                let transport = StreamableHttpClientTransport::from_config(config.clone());
                let mcp_client = self.create_mcp_client();
                let client = serve_client(mcp_client, transport).await.map_err(|e| {
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

                let mcp_client = self.create_mcp_client();
                let client = mcp_client.serve(TokioChildProcess::new(cmd)?).await?;

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

                let mcp_client = self.create_mcp_client();
                let client = serve_client(mcp_client, client_transport)
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
                        let namespaced_tool_name =
                            create_namespaced_tool_name(server_name, &tool_name);
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
        if !self.tools.contains_key(namespaced_tool_name) {
            return Err(color_eyre::Report::msg(format!(
                "Tool not found: {namespaced_tool_name}"
            )));
        }

        let (server_name, original_tool_name) = parse_namespaced_tool_name(namespaced_tool_name)
            .ok_or_else(|| {
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
                let server_name = parse_namespaced_tool_name(namespaced_tool_name)
                    .map(|(server, _)| server.to_string());

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
                .retain(|tool_name, _| !tool_name.starts_with(server_name));
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
    client: RunningService<RoleClient, McpClient>,
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

fn create_namespaced_tool_name(server_name: &str, tool_name: &str) -> String {
    format!("{server_name}{TOOL_NAMESPACE_DELIMITER}{tool_name}")
}

fn parse_namespaced_tool_name(namespaced_name: &str) -> Option<(&str, &str)> {
    namespaced_name.split_once(TOOL_NAMESPACE_DELIMITER)
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

        // Create elicitation channel for test
        let (elicitation_tx, _elicitation_rx) = mpsc::unbounded_channel::<ElicitationRequest>();

        // Create client and connect in-memory server directly
        let mut client = McpManager::new(elicitation_tx);
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

    #[test]
    fn test_namespacing_functions() {
        // Test creating namespaced tool name
        let namespaced = create_namespaced_tool_name("server1", "tool1");
        assert_eq!(namespaced, "server1__tool1");

        // Test parsing namespaced tool name
        let (server, tool) = parse_namespaced_tool_name(&namespaced).unwrap();
        assert_eq!(server, "server1");
        assert_eq!(tool, "tool1");

        // Test invalid namespaced tool name
        assert!(parse_namespaced_tool_name("invalid_name").is_none());

        // Test that tool names start with server name for filtering
        assert!(namespaced.starts_with("server1"));
    }

    #[tokio::test]
    async fn test_elicitation_channel_creation() {
        // Test that elicitation channels are properly wired up
        let (elicitation_tx, mut elicitation_rx) = mpsc::unbounded_channel::<ElicitationRequest>();

        let _manager = McpManager::new(elicitation_tx.clone());

        // Simulate an elicitation request being sent
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let test_request = ElicitationRequest {
            request: rmcp::model::CreateElicitationRequestParam {
                message: "Test elicitation".to_string(),
                requested_schema: serde_json::Map::new(),
            },
            response_sender: response_tx,
        };

        // Send the request through the channel
        elicitation_tx.send(test_request).unwrap();

        // Verify we can receive the request
        let received_request = elicitation_rx.recv().await.unwrap();
        assert_eq!(received_request.request.message, "Test elicitation");

        // Simulate responding to the request
        let response = rmcp::model::CreateElicitationResult {
            action: rmcp::model::ElicitationAction::Accept,
            content: Some(serde_json::json!({"test": "data"})),
        };
        received_request.response_sender.send(response).unwrap();

        // Verify the response was received
        let received_response = response_rx.await.unwrap();
        assert_eq!(
            received_response.action,
            rmcp::model::ElicitationAction::Accept
        );
    }
}
