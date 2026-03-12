use llm::ToolDefinition;

use super::{
    McpError, Result,
    config::{McpServerConfig, ServerConfig},
    connection::{ConnectParams, ConnectResult, McpServerConnection, ServerInstructions, Tool},
    mcp_client::McpClient,
    naming::{create_namespaced_tool_name, split_on_server_name},
    oauth::{OAuthHandler, perform_oauth_flow},
    tool_proxy::ToolProxy,
};
use rmcp::{
    RoleClient,
    model::{
        CallToolRequestParams, ClientCapabilities, ClientInfo, CreateElicitationRequestParams,
        CreateElicitationResult, ElicitationAction, Implementation, ProtocolVersion, Root,
    },
    service::RunningService,
    transport::streamable_http_client::StreamableHttpClientTransportConfig,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, oneshot};

pub use crate::status::{McpServerStatus, McpServerStatusEntry};

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

    fn connect_params(&self) -> ConnectParams {
        ConnectParams {
            mcp_client: self.create_mcp_client(),
            oauth_handler: self.oauth_handler.clone(),
        }
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
                // Only record Failed if not already recorded by connect logic
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
        let config = ServerConfig::Http {
            name: name.clone(),
            config: StreamableHttpClientTransportConfig {
                uri: base_url.into(),
                auth_header: Some(auth_header),
                ..Default::default()
            },
        };
        let params = self.connect_params();
        match McpServerConnection::connect(config, params).await {
            ConnectResult::Connected(conn) => {
                self.register_server(&name, conn, Registration::Direct)
                    .await
            }
            ConnectResult::NeedsOAuth { error, .. } => Err(error),
            ConnectResult::Failed(e) => Err(e),
        }
    }

    pub async fn add_mcp(&mut self, config: McpServerConfig) -> Result<()> {
        match config {
            McpServerConfig::ToolProxy { name, servers } => {
                self.connect_tool_proxy(name, servers).await
            }

            McpServerConfig::Server(config) => {
                let name = config.name().to_string();
                let params = self.connect_params();
                match McpServerConnection::connect(config, params).await {
                    ConnectResult::Connected(conn) => {
                        self.register_server(&name, conn, Registration::Direct)
                            .await
                    }
                    ConnectResult::NeedsOAuth {
                        name,
                        config,
                        error,
                    } => {
                        self.pending_configs.insert(name.clone(), config);
                        self.set_status(&name, McpServerStatus::NeedsOAuth);
                        Err(error)
                    }
                    ConnectResult::Failed(e) => Err(e),
                }
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
            let params = self.connect_params();

            let result = match McpServerConnection::connect(config, params).await {
                ConnectResult::Connected(conn) => {
                    self.register_server(&server_name, conn, Registration::Proxied)
                        .await
                }
                ConnectResult::NeedsOAuth {
                    name,
                    config,
                    error,
                } => {
                    self.pending_configs.insert(name.clone(), config);
                    self.set_status(&name, McpServerStatus::NeedsOAuth);
                    Err(error)
                }
                ConnectResult::Failed(e) => Err(e),
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

                        let description =
                            ToolProxy::extract_server_description(&client, &server_name);
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

        let mcp_client = self.create_mcp_client();
        let conn = McpServerConnection::reconnect_with_auth(&name, config, auth_client, mcp_client)
            .await?;

        // If this server is proxied, register without exposing tools to the agent
        if let Some(proxy) = self.proxy.as_ref().filter(|p| p.contains_server(&name)) {
            let tool_dir = proxy.tool_dir().to_path_buf();
            self.register_server(&name, conn, Registration::Proxied)
                .await?;
            // Write tool files now that connection succeeded
            if let Some(conn) = self.servers.get(&name) {
                let client = conn.client.clone();
                if let Err(e) = ToolProxy::write_tools_to_dir(&name, &client, &tool_dir).await {
                    tracing::warn!("Failed to write tool files for '{name}' after OAuth: {e}");
                }
            }
            Ok(())
        } else {
            self.register_server(&name, conn, Registration::Direct)
                .await
        }
    }

    /// Register a connected server and discover its tools.
    ///
    /// When `registration` is `Direct`, discovered tools are added to
    /// `self.tool_definitions` (visible to the agent). When `Proxied`, tools are
    /// only stored in `self.tools` for internal routing.
    async fn register_server(
        &mut self,
        name: &str,
        conn: McpServerConnection,
        registration: Registration,
    ) -> Result<()> {
        let tools = conn.list_tools().await.map_err(|e| {
            McpError::ToolDiscoveryFailed(format!("Failed to list tools for {name}: {e}"))
        })?;

        for rmcp_tool in &tools {
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

        let tool_count = tools.len();

        self.set_status(name, McpServerStatus::Connected { tool_count });

        // Remove from pending configs if it was there
        self.pending_configs.remove(name);

        self.servers.insert(name.to_string(), conn);
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
                        tracing::info!("Server '{server_name}' shut down gracefully");
                    }
                    Ok(Err(e)) => {
                        tracing::warn!("Server '{server_name}' task panicked: {e:?}");
                    }
                    Err(_) => {
                        tracing::warn!("Server '{server_name}' shutdown timed out");
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
                        tracing::info!("Server '{server_name}' shut down gracefully");
                    }
                    Ok(Err(e)) => {
                        tracing::warn!("Server '{server_name}' task panicked: {e:?}");
                    }
                    Err(_) => {
                        tracing::warn!("Server '{server_name}' shutdown timed out");
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
                tracing::debug!(
                    "Note: server '{server_name}' did not accept roots notification: {e}"
                );
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
                tracing::warn!("Server '{server_name}' task aborted during cleanup");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::McpManager;
    use crate::client::config::ServerConfig;
    use rmcp::{
        Json, RoleServer, ServerHandler,
        handler::server::{router::tool::ToolRouter, wrapper::Parameters},
        model::{Implementation, ServerCapabilities, ServerInfo},
        service::DynService,
        tool, tool_handler, tool_router,
    };
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};
    use std::{
        io,
        sync::{Arc, Mutex},
    };
    use tokio::sync::mpsc;

    #[derive(Clone)]
    struct TestServer {
        tool_router: ToolRouter<Self>,
    }

    #[tool_handler(router = self.tool_router)]
    impl ServerHandler for TestServer {
        fn get_info(&self) -> ServerInfo {
            ServerInfo {
                server_info: Implementation {
                    name: "test-server".to_string(),
                    version: "0.1.0".to_string(),
                    description: Some("Test MCP server".to_string()),
                    title: None,
                    icons: None,
                    website_url: None,
                },
                capabilities: ServerCapabilities::builder().enable_tools().build(),
                ..Default::default()
            }
        }
    }

    impl Default for TestServer {
        fn default() -> Self {
            Self {
                tool_router: Self::tool_router(),
            }
        }
    }

    #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    struct EchoRequest {
        value: String,
    }

    #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    struct EchoResult {
        value: String,
    }

    #[tool_router]
    impl TestServer {
        fn as_dyn(self) -> Box<dyn DynService<RoleServer>> {
            Box::new(self)
        }

        #[tool(description = "Returns the provided value")]
        async fn echo(&self, request: Parameters<EchoRequest>) -> Json<EchoResult> {
            let Parameters(EchoRequest { value }) = request;
            Json(EchoResult { value })
        }
    }

    #[derive(Clone)]
    struct SharedWriter(Arc<Mutex<Vec<u8>>>);

    impl io::Write for SharedWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn drop_logs_cleanup_abort_with_tracing() {
        let (elicitation_sender, _elicitation_receiver) = mpsc::channel(1);
        let mut manager = McpManager::new(elicitation_sender, None);
        manager
            .add_mcp(
                ServerConfig::InMemory {
                    name: "test".to_string(),
                    server: TestServer::default().as_dyn(),
                }
                .into(),
            )
            .await
            .unwrap();

        let output = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .without_time()
            .with_writer({
                let output = Arc::clone(&output);
                move || SharedWriter(Arc::clone(&output))
            })
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            drop(manager);
        });

        let logs = String::from_utf8(output.lock().unwrap().clone()).unwrap();
        assert!(logs.contains("Server 'test' task aborted during cleanup"));
    }
}
