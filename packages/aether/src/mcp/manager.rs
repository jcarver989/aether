use crate::{
    llm::ToolDefinition,
    mcp::{McpError, Result, config::McpServerConfig},
};
use rmcp::{
    RoleClient, ServiceExt,
    model::{
        ClientCapabilities, ClientInfo, CreateElicitationRequestParam, CreateElicitationResult,
        ElicitationAction, ElicitationCapability, Implementation, Tool as RmcpTool,
    },
    serve_client,
    service::RunningService,
    transport::{StreamableHttpClientTransport, TokioChildProcess},
};
use serde_json::Value;
use std::collections::HashMap;

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

/// Manages connections to multiple MCP servers and their tools
pub struct McpManager {
    servers: HashMap<String, McpServerConnection>,
    tools: HashMap<String, Tool>,
    tool_definitions: Vec<ToolDefinition>,
    client_info: ClientInfo,
    elicitation_sender: mpsc::Sender<ElicitationRequest>,
}

impl McpManager {
    pub fn new(elicitation_sender: mpsc::Sender<ElicitationRequest>) -> Self {
        Self {
            servers: HashMap::new(),
            tools: HashMap::new(),
            tool_definitions: Vec::new(),
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

    pub async fn add_mcps(&mut self, configs: Vec<McpServerConfig>) -> Result<()> {
        for config in configs {
            self.add_mcp(config).await?;
        }
        Ok(())
    }

    pub async fn add_mcp(&mut self, config: McpServerConfig) -> Result<()> {
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

                self.discover_tools_for_server(&name, &client).await?;
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
                self.discover_tools_for_server(&name, &client).await?;
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

                self.discover_tools_for_server(&name, &client).await?;
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

    /// Discover tools for a specific server and add them to the manager's bookkeeping
    async fn discover_tools_for_server(
        &mut self,
        server_name: &str,
        client: &RunningService<RoleClient, McpClient>,
    ) -> Result<()> {
        let tools_response = client.list_tools(None).await.map_err(|e| {
            McpError::ToolDiscoveryFailed(format!(
                "Failed to list tools for {}: {}",
                server_name, e
            ))
        })?;

        for rmcp_tool in &tools_response.tools {
            let tool_name = rmcp_tool.name.to_string();
            let namespaced_tool_name = create_namespaced_tool_name(server_name, &tool_name);
            let tool = Tool::from(rmcp_tool);

            self.tool_definitions.push(ToolDefinition {
                name: namespaced_tool_name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.to_string(),
                server: Some(server_name.to_string()),
            });

            self.tools.insert(namespaced_tool_name, tool);
        }

        Ok(())
    }

    /// Get the MCP client for a given tool name
    pub fn get_client_for_tool(
        &self,
        namespaced_tool_name: &str,
    ) -> Result<rmcp::Peer<RoleClient>> {
        if !self.tools.contains_key(namespaced_tool_name) {
            return Err(McpError::ToolNotFound(namespaced_tool_name.to_string()));
        }

        let (server_name, _) = parse_namespaced_tool_name(namespaced_tool_name)
            .ok_or_else(|| McpError::InvalidToolNameFormat(namespaced_tool_name.to_string()))?;

        let client = self
            .servers
            .get(server_name)
            .map(|server| server.client.clone())
            .ok_or_else(|| McpError::ServerNotFound(server_name.to_string()))?;

        Ok(client)
    }

    pub fn tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tool_definitions.clone()
    }

    /// Shutdown all servers and wait for their tasks to complete
    pub async fn shutdown(&mut self) {
        let servers: Vec<(String, McpServerConnection)> = self.servers.drain().collect();

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

        self.tools.clear();
        self.tool_definitions.clear();
    }

    /// Shutdown a specific server by name
    pub async fn shutdown_server(&mut self, server_name: &str) -> Result<()> {
        let server = self.servers.remove(server_name);

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
                .retain(|tool_name, _| !tool_name.starts_with(server_name));

            self.tool_definitions
                .retain(|tool_def| !tool_def.name.starts_with(server_name));
        }

        Ok(())
    }
}

impl Drop for McpManager {
    fn drop(&mut self) {
        let servers: Vec<(String, McpServerConnection)> = self.servers.drain().collect();
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
