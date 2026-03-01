use llm::ToolDefinition;

use super::{
    McpError, Result,
    config::{McpServerConfig, ServerConfig},
    mcp_client::McpClient,
    oauth::{OAuthHandler, create_auth_manager_from_store, perform_oauth_flow},
    tool_proxy::ToolProxy,
};
use crate::transport::create_in_memory_transport;
use rmcp::{
    RoleClient, RoleServer, ServiceExt,
    model::{
        CallToolRequestParams, ClientCapabilities, ClientInfo, CreateElicitationRequestParams,
        CreateElicitationResult, ElicitationAction, Implementation, ProtocolVersion, Root,
        Tool as RmcpTool,
    },
    serve_client,
    service::{DynService, RunningService},
    transport::streamable_http_client::StreamableHttpClientTransportConfig,
    transport::{StreamableHttpClientTransport, TokioChildProcess, auth::AuthClient},
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, oneshot};
use tokio::{process::Command, task::JoinHandle};

pub use crate::status::{McpServerStatus, McpServerStatusEntry};

const SERVERNAME_DELIMITER: &str = "__";

#[derive(Debug)]
pub struct ElicitationRequest {
    pub request: CreateElicitationRequestParams,
    pub response_sender: oneshot::Sender<CreateElicitationResult>,
}

#[derive(Debug, Clone)]
pub struct ElicitationResponse {
    pub action: ElicitationAction,
    pub content: Option<Value>,
}

/// Whether a server's tools should be directly exposed to the agent or only
/// registered internally for proxy routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Registration {
    /// Tools are added to `tool_definitions` (visible to the agent).
    Direct,
    /// Tools are stored in `self.tools` for routing but not exposed to the agent.
    Proxied,
}

/// Manages connections to multiple MCP servers and their tools
pub struct McpManager {
    servers: HashMap<String, McpServerConnection>,
    tools: HashMap<String, Tool>,
    tool_definitions: Vec<ToolDefinition>,
    client_info: ClientInfo,
    elicitation_sender: mpsc::Sender<ElicitationRequest>,
    /// Roots shared with all MCP clients
    roots: Arc<RwLock<Vec<Root>>>,
    oauth_handler: Option<Arc<dyn OAuthHandler>>,
    server_statuses: Vec<McpServerStatusEntry>,
    /// Configs for failed HTTP servers so we can retry OAuth later
    pending_configs: HashMap<String, StreamableHttpClientTransportConfig>,
    /// Optional tool-proxy that wraps multiple servers behind a single `call_tool`.
    proxy: Option<ToolProxy>,
}

impl McpManager {
    pub fn new(
        elicitation_sender: mpsc::Sender<ElicitationRequest>,
        oauth_handler: Option<Arc<dyn OAuthHandler>>,
    ) -> Self {
        Self {
            servers: HashMap::new(),
            tools: HashMap::new(),
            tool_definitions: Vec::new(),
            client_info: ClientInfo {
                meta: None,
                protocol_version: ProtocolVersion::default(),
                capabilities: ClientCapabilities::builder()
                    .enable_elicitation()
                    .enable_roots()
                    .build(),
                client_info: Implementation {
                    name: "aether".to_string(),
                    version: "0.1.0".to_string(),
                    description: None,
                    title: None,
                    icons: None,
                    website_url: None,
                },
            },
            elicitation_sender,
            roots: Arc::new(RwLock::new(Vec::new())),
            oauth_handler,
            server_statuses: Vec::new(),
            pending_configs: HashMap::new(),
            proxy: None,
        }
    }

    fn create_mcp_client(&self) -> McpClient {
        McpClient::new(
            self.client_info.clone(),
            self.elicitation_sender.clone(),
            Arc::clone(&self.roots),
        )
    }

    /// Update or insert the status entry for a server.
    fn set_status(&mut self, name: &str, status: McpServerStatus) {
        if let Some(entry) = self.server_statuses.iter_mut().find(|s| s.name == name) {
            entry.status = status;
        } else {
            self.server_statuses.push(McpServerStatusEntry {
                name: name.to_string(),
                status,
            });
        }
    }

    pub async fn add_mcps(&mut self, configs: Vec<McpServerConfig>) -> Result<()> {
        for config in configs {
            let name = config.name().to_string();
            if let Err(e) = self.add_mcp(config).await {
                // Log warning but continue with other servers
                tracing::warn!("Failed to connect to MCP server '{}': {}", name, e);
                // Only record Failed if not already recorded by connect_http_server
                if !self.server_statuses.iter().any(|s| s.name == name) {
                    self.set_status(
                        &name,
                        McpServerStatus::Failed {
                            error: e.to_string(),
                        },
                    );
                }
            }
        }
        Ok(())
    }

    pub async fn add_mcp_with_auth(
        &mut self,
        name: String,
        base_url: &str,
        auth_header: String,
    ) -> Result<()> {
        let config = StreamableHttpClientTransportConfig {
            uri: base_url.into(),
            auth_header: Some(auth_header),
            ..Default::default()
        };
        self.connect_http_server_inner(name, config, Registration::Direct)
            .await
    }

    pub async fn add_mcp(&mut self, config: McpServerConfig) -> Result<()> {
        // HTTP connections need special OAuth handling, so they go through
        // their own path. Everything else uses the shared `connect_to_server`.
        match config {
            McpServerConfig::Server(ServerConfig::Http { name, config }) => {
                self.connect_http_server_inner(name, config, Registration::Direct)
                    .await
            }

            McpServerConfig::ToolProxy { name, servers } => {
                self.connect_tool_proxy(name, servers).await
            }

            McpServerConfig::Server(config) => {
                let name = config.name().to_string();
                let create_client = || self.create_mcp_client();
                let (client, task) = connect_to_server(config, create_client).await?;
                self.register_server_inner(&name, client, task, Registration::Direct)
                    .await
            }
        }
    }

    /// Connect a tool-proxy: register each nested server individually through
    /// the manager (getting OAuth for free), then inject a single `call_tool`
    /// virtual tool for the agent.
    async fn connect_tool_proxy(
        &mut self,
        proxy_name: String,
        servers: Vec<ServerConfig>,
    ) -> Result<()> {
        let tool_dir = ToolProxy::dir(&proxy_name)?;
        ToolProxy::clean_dir(&tool_dir).await?;

        let mut nested_names = HashSet::new();
        let mut server_descriptions = Vec::new();

        for config in servers {
            let server_name = config.name().to_string();

            // HTTP servers go through connect_http_server (gets OAuth for free).
            // Everything else uses the normal connect_to_server path.
            let result = match config {
                ServerConfig::Http { name, config: cfg } => {
                    self.connect_http_server_inner(name, cfg, Registration::Proxied)
                        .await
                }
                other => {
                    let create_client = || self.create_mcp_client();
                    match connect_to_server(other, &create_client).await {
                        Ok((client, task)) => {
                            self.register_server_inner(
                                &server_name,
                                client,
                                task,
                                Registration::Proxied,
                            )
                            .await
                        }
                        Err(e) => Err(e),
                    }
                }
            };

            match result {
                Ok(()) => {
                    // Write tool files to disk for agent browsing
                    if let Some(conn) = self.servers.get(&server_name) {
                        let client = conn.client.clone();
                        if let Err(e) =
                            ToolProxy::write_tools_to_dir(&server_name, &client, &tool_dir).await
                        {
                            tracing::warn!(
                                "Failed to write tool files for nested server '{server_name}': {e}"
                            );
                        }

                        let description = ToolProxy::extract_server_description(&client, &server_name);
                        server_descriptions.push((server_name.clone(), description));
                    }
                    nested_names.insert(server_name);
                }
                Err(e) => {
                    tracing::warn!("Failed to connect nested server '{server_name}': {e}");
                    // If it was stashed as NeedsOAuth, record the membership so
                    // authenticate_server can write tool files later.
                    if self.pending_configs.contains_key(&server_name) {
                        nested_names.insert(server_name);
                    }
                }
            }
        }

        let call_tool_def = ToolProxy::call_tool_definition(&proxy_name);
        self.tools.insert(
            call_tool_def.name.clone(),
            Tool {
                description: call_tool_def.description.clone(),
                parameters: serde_json::from_str(&call_tool_def.parameters)
                    .unwrap_or(Value::Object(serde_json::Map::default())),
            },
        );
        self.tool_definitions.push(call_tool_def);

        self.proxy = Some(ToolProxy::new(
            proxy_name.clone(),
            nested_names,
            tool_dir,
            &server_descriptions,
        ));

        // Add proxy status entry
        self.set_status(&proxy_name, McpServerStatus::Connected { tool_count: 1 });

        Ok(())
    }

    /// Connect to an HTTP MCP server. Tries stored OAuth credentials first,
    /// falls back to plain connection, and retries with a fresh OAuth flow on failure.
    async fn connect_http_server_inner(
        &mut self,
        name: String,
        config: StreamableHttpClientTransportConfig,
        registration: Registration,
    ) -> Result<()> {
        let mcp_client = self.create_mcp_client();
        let conn_err = |e| McpError::ConnectionFailed(format!("HTTP MCP server {name}: {e}"));

        let result = match self.create_auth_client(&name, &config.uri).await {
            Some(auth_client) if config.auth_header.is_none() => {
                tracing::debug!("Using OAuth for server '{name}'");
                let transport =
                    StreamableHttpClientTransport::with_client(auth_client, config.clone());
                serve_client(mcp_client, transport).await.map_err(conn_err)
            }
            _ => {
                let transport = StreamableHttpClientTransport::from_config(config.clone());
                serve_client(mcp_client, transport).await.map_err(conn_err)
            }
        };

        match result {
            Ok(client) => {
                self.register_server_inner(&name, client, None, registration)
                    .await
            }
            Err(err) => {
                tracing::warn!("Failed to connect to MCP server '{name}': {err}");

                // Only assume OAuth is needed if we have a handler that can
                // perform the flow. Otherwise the server is simply unreachable.
                let status = if self.oauth_handler.is_some() {
                    self.pending_configs.insert(name.clone(), config);
                    McpServerStatus::NeedsOAuth
                } else {
                    McpServerStatus::Failed {
                        error: err.to_string(),
                    }
                };
                self.set_status(&name, status);
                Err(err)
            }
        }
    }

    async fn oauth_and_reconnect(
        &mut self,
        name: String,
        config: StreamableHttpClientTransportConfig,
    ) -> Result<()> {
        let handler = self.oauth_handler.as_ref().ok_or_else(|| {
            McpError::ConnectionFailed(format!("No OAuth handler available for '{name}'"))
        })?;
        let auth_client = perform_oauth_flow(&name, &config.uri, handler.as_ref())
            .await
            .map_err(|e| McpError::ConnectionFailed(format!("OAuth failed for '{name}': {e}")))?;
        let transport = StreamableHttpClientTransport::with_client(auth_client, config);

        let mcp_client = self.create_mcp_client();
        let client = serve_client(mcp_client, transport).await.map_err(|e| {
            McpError::ConnectionFailed(format!("reconnect failed for '{name}': {e}"))
        })?;

        // If this server is proxied, register without exposing tools to the agent
        if let Some(proxy) = self.proxy.as_ref().filter(|p| p.contains_server(&name)) {
            let tool_dir = proxy.tool_dir().to_path_buf();
            self.register_server_inner(&name, client, None, Registration::Proxied)
                .await?;
            // Write tool files now that connection succeeded
            if let Some(conn) = self.servers.get(&name) {
                let client = conn.client.clone();
                if let Err(e) =
                    ToolProxy::write_tools_to_dir(&name, &client, &tool_dir).await
                {
                    tracing::warn!("Failed to write tool files for '{name}' after OAuth: {e}");
                }
            }
            Ok(())
        } else {
            self.register_server_inner(&name, client, None, Registration::Direct)
                .await
        }
    }

    async fn create_auth_client(
        &self,
        server_id: &str,
        base_url: &str,
    ) -> Option<AuthClient<reqwest::Client>> {
        let auth_manager = create_auth_manager_from_store(server_id, base_url)
            .await
            .ok()??;
        Some(AuthClient::new(reqwest::Client::default(), auth_manager))
    }

    /// Register a server connection and discover its tools.
    ///
    /// When `registration` is `Direct`, discovered tools are added to
    /// `self.tool_definitions` (visible to the agent). When `Proxied`, tools are
    /// only stored in `self.tools` for internal routing.
    async fn register_server_inner(
        &mut self,
        name: &str,
        client: RunningService<RoleClient, McpClient>,
        server_task: Option<JoinHandle<()>>,
        registration: Registration,
    ) -> Result<()> {
        let tools_response = client.list_tools(None).await.map_err(|e| {
            McpError::ToolDiscoveryFailed(format!("Failed to list tools for {name}: {e}"))
        })?;

        for rmcp_tool in &tools_response.tools {
            let tool_name = rmcp_tool.name.to_string();
            let namespaced_tool_name = create_namespaced_tool_name(name, &tool_name);
            let tool = Tool::from(rmcp_tool);

            if registration == Registration::Direct {
                self.tool_definitions.push(ToolDefinition {
                    name: namespaced_tool_name.clone(),
                    description: tool.description.clone(),
                    parameters: tool.parameters.to_string(),
                    server: Some(name.to_string()),
                });
            }

            self.tools.insert(namespaced_tool_name, tool);
        }

        let tool_count = tools_response.tools.len();

        self.set_status(name, McpServerStatus::Connected { tool_count });

        // Remove from pending configs if it was there
        self.pending_configs.remove(name);

        self.servers.insert(
            name.to_string(),
            McpServerConnection {
                instructions: extract_instructions(&client),
                client: Arc::new(client),
                server_task,
            },
        );
        Ok(())
    }

    /// Resolve and route a tool call.
    ///
    /// Returns the target MCP client and normalized call params. For proxy
    /// `call_tool`, this parses the wrapper arguments and forwards to the
    /// selected nested server/tool.
    pub fn get_client_for_tool(
        &self,
        namespaced_tool_name: &str,
        arguments_json: &str,
    ) -> Result<(
        Arc<RunningService<RoleClient, McpClient>>,
        CallToolRequestParams,
    )> {
        if !self.tools.contains_key(namespaced_tool_name) {
            return Err(McpError::ToolNotFound(namespaced_tool_name.to_string()));
        }

        let (server_name, tool_name) = split_on_server_name(namespaced_tool_name)
            .ok_or_else(|| McpError::InvalidToolNameFormat(namespaced_tool_name.to_string()))?;

        if let Some(proxy) = self.proxy.as_ref().filter(|p| p.name() == server_name) {
            let call = proxy.resolve_call(arguments_json)?;
            let conn = self.servers.get(&call.server).ok_or_else(|| {
                McpError::ServerNotFound(format!(
                    "Nested server '{}' is not connected",
                    call.server
                ))
            })?;
            let params = CallToolRequestParams {
                meta: None,
                name: call.tool.into(),
                arguments: call.arguments,
                task: None,
            };
            return Ok((conn.client.clone(), params));
        }

        let client = self
            .servers
            .get(server_name)
            .map(|server| server.client.clone())
            .ok_or_else(|| McpError::ServerNotFound(server_name.to_string()))?;

        let arguments = serde_json::from_str::<serde_json::Value>(arguments_json)?
            .as_object()
            .cloned();
        let params = CallToolRequestParams {
            meta: None,
            name: tool_name.to_string().into(),
            arguments,
            task: None,
        };

        Ok((client, params))
    }

    pub fn tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tool_definitions.clone()
    }

    /// Returns instructions from all connected MCP servers that provide them,
    /// plus synthesized instructions for tool-proxy groups.
    pub fn server_instructions(&self) -> Vec<ServerInstructions> {
        let mut instructions: Vec<ServerInstructions> = self
            .servers
            .iter()
            .filter(|(name, _)| self.proxy.as_ref().is_none_or(|p| !p.contains_server(name)))
            .filter_map(|(name, conn)| {
                conn.instructions.as_ref().map(|instr| ServerInstructions {
                    server_name: name.clone(),
                    instructions: instr.clone(),
                })
            })
            .collect();

        if let Some(proxy) = &self.proxy {
            instructions.push(ServerInstructions {
                server_name: proxy.name().to_string(),
                instructions: proxy.instructions().to_string(),
            });
        }

        instructions
    }

    pub fn server_statuses(&self) -> &[McpServerStatusEntry] {
        &self.server_statuses
    }

    /// Authenticate a server that previously failed with `NeedsOAuth`.
    ///
    /// Looks up the pending config, runs the OAuth flow, and updates the status
    /// entry on success.
    pub async fn authenticate_server(&mut self, name: &str) -> Result<()> {
        let config = self
            .pending_configs
            .get(name)
            .ok_or_else(|| {
                McpError::ConnectionFailed(format!("no pending config for server '{name}'"))
            })?
            .clone();

        self.oauth_and_reconnect(name.to_string(), config).await
    }

    /// List all prompts from all connected MCP servers with namespacing
    pub async fn list_prompts(&self) -> Result<Vec<rmcp::model::Prompt>> {
        use futures::future::join_all;

        let futures: Vec<_> = self
            .servers
            .iter()
            .filter(|(_, server_conn)| {
                server_conn
                    .client
                    .peer_info()
                    .and_then(|info| info.capabilities.prompts.as_ref())
                    .is_some()
            })
            .map(|(server_name, server_conn)| {
                let server_name = server_name.clone();
                let client = server_conn.client.clone();
                async move {
                    let prompts_response = client.list_prompts(None).await.map_err(|e| {
                        McpError::PromptListFailed(format!(
                            "Failed to list prompts for {server_name}: {e}"
                        ))
                    })?;

                    let namespaced_prompts: Vec<rmcp::model::Prompt> = prompts_response
                        .prompts
                        .into_iter()
                        .map(|prompt| {
                            let namespaced_name =
                                create_namespaced_tool_name(&server_name, &prompt.name);
                            rmcp::model::Prompt {
                                name: namespaced_name,
                                description: prompt.description,
                                arguments: prompt.arguments,
                                title: prompt.title,
                                icons: prompt.icons,
                                meta: prompt.meta,
                            }
                        })
                        .collect();

                    Ok::<_, McpError>(namespaced_prompts)
                }
            })
            .collect();

        let results = join_all(futures).await;
        let mut all_prompts = Vec::new();
        for result in results {
            all_prompts.extend(result?);
        }

        Ok(all_prompts)
    }

    /// Get a specific prompt by namespaced name
    pub async fn get_prompt(
        &self,
        namespaced_prompt_name: &str,
        arguments: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<rmcp::model::GetPromptResult> {
        let (server_name, prompt_name) = split_on_server_name(namespaced_prompt_name)
            .ok_or_else(|| McpError::InvalidToolNameFormat(namespaced_prompt_name.to_string()))?;

        let server_conn = self
            .servers
            .get(server_name)
            .ok_or_else(|| McpError::ServerNotFound(server_name.to_string()))?;

        let request = rmcp::model::GetPromptRequestParams {
            meta: None,
            name: prompt_name.into(),
            arguments,
        };

        server_conn.client.get_prompt(request).await.map_err(|e| {
            McpError::PromptGetFailed(format!(
                "Failed to get prompt '{prompt_name}' from {server_name}: {e}"
            ))
        })
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
                        println!("Server '{server_name}' shut down gracefully");
                    }
                    Ok(Err(e)) => {
                        eprintln!("Server '{server_name}' task panicked: {e:?}");
                    }
                    Err(_) => {
                        eprintln!("Server '{server_name}' shutdown timed out");
                        // Task will be cancelled when the handle is dropped
                    }
                }
            }
        }

        self.tools.clear();
        self.tool_definitions.clear();
        self.proxy = None;
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
                        println!("Server '{server_name}' shut down gracefully");
                    }
                    Ok(Err(e)) => {
                        eprintln!("Server '{server_name}' task panicked: {e:?}");
                    }
                    Err(_) => {
                        eprintln!("Server '{server_name}' shutdown timed out");
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

    /// Set the roots advertised to MCP servers.
    ///
    /// This updates the roots and sends notifications to all connected servers
    /// that support the `roots/list_changed` notification.
    pub async fn set_roots(&mut self, new_roots: Vec<Root>) -> Result<()> {
        // Update stored roots
        {
            let mut roots = self.roots.write().await;
            *roots = new_roots;
        }

        // Notify all connected servers
        self.notify_roots_changed().await;

        Ok(())
    }

    /// Send `roots/list_changed` notification to all connected servers.
    ///
    /// This prompts servers to re-request the roots via the roots/list endpoint.
    /// Servers that don't support roots will simply ignore the notification.
    async fn notify_roots_changed(&self) {
        for (server_name, server_conn) in &self.servers {
            // Try to send notification - servers that don't support roots will ignore it
            if let Err(e) = server_conn.client.notify_roots_list_changed().await {
                // Only log errors for debugging; it's expected that some servers may not support roots
                eprintln!("Note: server '{server_name}' did not accept roots notification: {e}");
            }
        }
    }
}

impl Drop for McpManager {
    fn drop(&mut self) {
        let servers: Vec<(String, McpServerConnection)> = self.servers.drain().collect();
        for (server_name, server) in servers {
            if let Some(handle) = server.server_task {
                handle.abort();
                eprintln!("Server '{server_name}' task aborted during cleanup");
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServerInstructions {
    pub server_name: String,
    pub instructions: String,
}

#[derive(Debug, Clone)]
pub struct Tool {
    pub description: String,
    pub parameters: Value,
}

struct McpServerConnection {
    client: Arc<RunningService<RoleClient, McpClient>>,
    server_task: Option<JoinHandle<()>>,
    instructions: Option<String>,
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
    format!("{server_name}{SERVERNAME_DELIMITER}{tool_name}")
}

/// Connect to an MCP server described by `config`, returning the running client
/// and an optional background task handle (for Stdio child processes / in-memory servers).
///
/// This is the shared connection logic used by `McpManager::add_mcp` and
/// proxied server registration.
pub(crate) async fn connect_to_server(
    config: ServerConfig,
    create_mcp_client: impl Fn() -> McpClient,
) -> Result<(
    RunningService<RoleClient, McpClient>,
    Option<JoinHandle<()>>,
)> {
    match config {
        ServerConfig::Stdio { command, args, .. } => {
            let mut cmd = Command::new(&command);
            cmd.args(&args);
            let mcp_client = create_mcp_client();
            let client = mcp_client.serve(TokioChildProcess::new(cmd)?).await?;
            Ok((client, None))
        }

        ServerConfig::Http { name, config: cfg } => {
            let transport = StreamableHttpClientTransport::from_config(cfg);
            let mcp_client = create_mcp_client();
            let client = serve_client(mcp_client, transport).await.map_err(|e| {
                McpError::ConnectionFailed(format!(
                    "Failed to connect to HTTP server '{name}': {e}"
                ))
            })?;
            Ok((client, None))
        }

        ServerConfig::InMemory { name, server } => {
            let mcp_client = create_mcp_client();
            let (client, handle) = serve_in_memory(server, mcp_client, &name).await?;
            Ok((client, Some(handle)))
        }
    }
}

/// Spawn an in-memory MCP server on a background task and connect a client to it.
///
/// Returns the running client service and the server's join handle.
pub(crate) async fn serve_in_memory(
    server: Box<dyn DynService<RoleServer>>,
    mcp_client: McpClient,
    label: &str,
) -> Result<(RunningService<RoleClient, McpClient>, JoinHandle<()>)> {
    let (client_transport, server_transport) = create_in_memory_transport();

    let server_handle = tokio::spawn(async move {
        match server.serve(server_transport).await {
            Ok(_service) => {
                std::future::pending::<()>().await;
            }
            Err(e) => {
                eprintln!("MCP server error: {e}");
            }
        }
    });

    let client = serve_client(mcp_client, client_transport)
        .await
        .map_err(|e| {
            McpError::ConnectionFailed(format!(
                "Failed to connect to in-memory server '{label}': {e}"
            ))
        })?;

    Ok((client, server_handle))
}

/// Extract non-empty instructions from an MCP client's peer info.
fn extract_instructions(client: &RunningService<RoleClient, McpClient>) -> Option<String> {
    client
        .peer_info()
        .and_then(|info| info.instructions.clone())
        .filter(|s| !s.is_empty())
}

pub fn split_on_server_name(namespaced_name: &str) -> Option<(&str, &str)> {
    namespaced_name.split_once(SERVERNAME_DELIMITER)
}
