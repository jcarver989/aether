use crate::mcp::{McpError, Result};
use crate::types::ToolDefinition;
use futures::future::join_all;
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
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tracing::{Level, span};

use crate::{mcp::client::McpClient, transport::create_in_memory_transport};
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
#[derive(Clone)]
pub struct McpManager {
    servers: Arc<Mutex<HashMap<String, McpServerConnection>>>,
    tools: Arc<Mutex<HashMap<String, Tool>>>,
    tool_definitions: Arc<Mutex<Vec<ToolDefinition>>>,
    client_info: ClientInfo,
    elicitation_sender: mpsc::Sender<ElicitationRequest>,
}

impl McpManager {
    pub fn new(elicitation_sender: mpsc::Sender<ElicitationRequest>) -> Self {
        Self {
            servers: Arc::new(Mutex::new(HashMap::new())),
            tools: Arc::new(Mutex::new(HashMap::new())),
            tool_definitions: Arc::new(Mutex::new(Vec::new())),
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

    pub async fn add_mcps(&self, configs: Vec<McpServerConfig>) -> Result<()> {
        let futures = configs.into_iter().map(|config| self.add_mcp(config));
        join_all(futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>>>()?;
        Ok(())
    }

    pub async fn add_mcp(&self, config: McpServerConfig) -> Result<()> {
        use McpServerConfig::*;
        match config {
            Http { name, config } => {
                let transport = StreamableHttpClientTransport::from_config(config.clone());
                let mcp_client = self.create_mcp_client();
                let client = serve_client(mcp_client, transport).await.map_err(|e| {
                    McpError::ConnectionFailed(format!(
                        "Failed to connect to HTTP MCP server {name}: {e}"
                    ))
                })?;

                // Discover tools before adding to servers map
                self.discover_tools_for_server(&name, &client).await?;

                let server_connection = McpServerConnection {
                    _name: name.clone(),
                    client,
                    server_task: None,
                };

                self.servers.lock().unwrap().insert(name, server_connection);

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

                // Discover tools before adding to servers map
                self.discover_tools_for_server(&name, &client).await?;

                self.servers.lock().unwrap().insert(
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
                        McpError::ConnectionFailed(format!(
                            "Failed to connect to in-memory MCP server {name}: {e}"
                        ))
                    })?;

                // Discover tools before adding to servers map
                self.discover_tools_for_server(&name, &client).await?;

                let server_connection = McpServerConnection {
                    _name: name.clone(),
                    client,
                    server_task: Some(server_handle),
                };

                self.servers.lock().unwrap().insert(name, server_connection);

                Ok(())
            }
        }
    }

    /// Discover tools for all connected servers (useful for refreshing tool list)
    pub async fn discover_tools(&self) -> Result<()> {
        // Clear existing tools before rediscovery
        {
            self.tools.lock().unwrap().clear();
            self.tool_definitions.lock().unwrap().clear();
        }

        let servers = self.servers.lock().unwrap();
        for (name, server) in servers.iter() {
            self.discover_tools_for_server(name, &server.client).await?;
        }

        Ok(())
    }

    /// Discover tools for a specific server and add them to the manager's bookkeeping
    async fn discover_tools_for_server(
        &self,
        server_name: &str,
        client: &RunningService<RoleClient, McpClient>,
    ) -> Result<()> {
        let tools_response = client.list_tools(None).await.map_err(|e| {
            McpError::ToolDiscoveryFailed(format!(
                "Failed to list tools for {}: {}",
                server_name, e
            ))
        })?;

        let mut tools = self.tools.lock().unwrap();
        let mut tool_definitions = self.tool_definitions.lock().unwrap();

        for rmcp_tool in &tools_response.tools {
            let tool_name = rmcp_tool.name.to_string();
            let namespaced_tool_name = create_namespaced_tool_name(server_name, &tool_name);
            let tool = Tool::from(rmcp_tool);

            tool_definitions.push(ToolDefinition {
                name: namespaced_tool_name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.to_string(),
                server: Some(server_name.to_string()),
            });

            tools.insert(namespaced_tool_name, tool);
        }

        Ok(())
    }

    pub async fn execute_tool(&self, namespaced_tool_name: &str, args: Value) -> Result<Value> {
        let span = span!(Level::DEBUG, "mcp_manager_execute_tool");
        let _guard = span.enter();

        if !self
            .tools
            .lock()
            .unwrap()
            .contains_key(namespaced_tool_name)
        {
            return Err(McpError::ToolNotFound(namespaced_tool_name.to_string()));
        }

        let (server_name, original_tool_name) = parse_namespaced_tool_name(namespaced_tool_name)
            .ok_or_else(|| McpError::InvalidToolNameFormat(namespaced_tool_name.to_string()))?;

        let client = {
            self.servers
                .lock()
                .unwrap()
                .get(server_name)
                .map(|server| server.client.clone())
                .ok_or_else(|| McpError::ServerNotFound(server_name.to_string()))?
        };

        let arguments = args.as_object().cloned();
        let request = CallToolRequestParam {
            name: original_tool_name.to_string().into(),
            arguments,
        };

        let result = match client.call_tool(request).await {
            Ok(result) => result,
            Err(e) => {
                return Err(McpError::ToolExecutionFailed {
                    tool_name: original_tool_name.to_string(),
                    server_name: server_name.to_string(),
                    error: e.to_string(),
                });
            }
        };

        if result.is_error.unwrap_or(false) {
            let error_msg = result
                .content
                .first()
                .map(|content| format!("{content:?}"))
                .unwrap_or_else(|| "Unknown error".to_string());
            return Err(McpError::ToolExecutionError(error_msg));
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

    pub fn tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tool_definitions.lock().unwrap().clone()
    }

    /// Shutdown all servers and wait for their tasks to complete
    pub async fn shutdown(&self) {
        let servers: Vec<(String, McpServerConnection)> =
            { self.servers.lock().unwrap().drain().collect() };

        for (server_name, server) in servers {
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

        self.tools.lock().unwrap().clear();
        self.tool_definitions.lock().unwrap().clear();
    }

    /// Shutdown a specific server by name
    pub async fn shutdown_server(&self, server_name: &str) -> Result<()> {
        let server = self.servers.lock().unwrap().remove(server_name);

        if let Some(server) = server {
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
                .lock()
                .unwrap()
                .retain(|tool_name, _| !tool_name.starts_with(server_name));

            self.tool_definitions
                .lock()
                .unwrap()
                .retain(|tool_def| !tool_def.name.starts_with(server_name));
        }

        Ok(())
    }
}

impl Drop for McpManager {
    fn drop(&mut self) {
        // Cancel all server tasks when the client is dropped
        let servers: Vec<(String, McpServerConnection)> =
            { self.servers.lock().unwrap().drain().collect() };

        for (server_name, server) in servers {
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

impl From<&RmcpTool> for Tool {
    fn from(tool: &RmcpTool) -> Self {
        Self {
            description: tool.description.clone().unwrap_or_default().to_string(),
            parameters: serde_json::Value::Object((*tool.input_schema).clone()),
        }
    }
}

fn create_namespaced_tool_name(server_name: &str, tool_name: &str) -> String {
    format!("{server_name}{TOOL_NAMESPACE_DELIMITER}{tool_name}")
}

pub fn parse_namespaced_tool_name(namespaced_name: &str) -> Option<(&str, &str)> {
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
        let (elicitation_tx, _elicitation_rx) = mpsc::channel::<ElicitationRequest>(50);

        // Create client and connect in-memory server directly
        let client = McpManager::new(elicitation_tx);
        let mcp_config = McpServerConfig::InMemory {
            name: "test_server".to_string(),
            server: dyn_service,
        };
        client.add_mcp(mcp_config).await.unwrap();

        // Verify tools were discovered immediately after adding server (no need to call discover_tools)
        let tool_definitions = client.tool_definitions();
        assert!(!tool_definitions.is_empty());

        // Check for the write_file tool
        let write_file_tool = tool_definitions
            .iter()
            .find(|tool| tool.name.contains("write_file"));
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
        let (elicitation_tx, mut elicitation_rx) = mpsc::channel::<ElicitationRequest>(50);

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
        elicitation_tx.send(test_request).await.unwrap();

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
