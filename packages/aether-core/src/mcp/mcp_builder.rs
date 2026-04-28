use llm::ToolDefinition;
use mcp_utils::client::oauth::OAuthHandler;

use mcp_utils::client::{
    McpClientEvent, McpError, McpManager, McpServerConfig, McpServerStatusEntry, ParseError, RawMcpConfig,
    ServerFactory, ServerInstructions, root_from_path,
};

use crate::agent_spec::McpJsonFileRef;

use super::run_mcp_task::{McpCommand, run_mcp_task};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum McpConfigLayer {
    File(McpJsonFileRef),
    Json(String),
}

impl From<McpJsonFileRef> for McpConfigLayer {
    fn from(value: McpJsonFileRef) -> Self {
        Self::File(value)
    }
}

pub fn mcp() -> McpBuilder {
    McpBuilder::new()
}

/// Result of spawning MCP servers
pub struct McpSpawnResult {
    pub tool_definitions: Vec<ToolDefinition>,
    pub instructions: Vec<ServerInstructions>,
    pub server_statuses: Vec<McpServerStatusEntry>,
    pub command_tx: Sender<McpCommand>,
    pub event_rx: Receiver<McpClientEvent>,
    pub handle: JoinHandle<()>,
}

pub struct McpBuilder {
    mcp_configs: Vec<McpServerConfig>,
    factories: HashMap<String, ServerFactory>,
    mcp_channel_capacity: usize,
    roots: Vec<PathBuf>,
    oauth_handler: Option<Arc<dyn OAuthHandler>>,
}

impl Default for McpBuilder {
    fn default() -> Self {
        Self {
            mcp_configs: Vec::new(),
            factories: HashMap::new(),
            mcp_channel_capacity: 1000,
            roots: Vec::new(),
            oauth_handler: None,
        }
    }
}

impl McpBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_servers(mut self, configs: Vec<McpServerConfig>) -> Self {
        self.mcp_configs.extend(configs);
        self
    }

    pub fn register_in_memory_server(mut self, name: impl Into<String>, factory: ServerFactory) -> Self {
        self.factories.insert(name.into(), factory);
        self
    }

    /// Set workspace roots that will be advertised to MCP servers.
    ///
    /// These roots are passed to the MCP client via the roots protocol,
    /// allowing servers to dynamically receive workspace information without
    /// relying solely on CLI arguments.
    pub fn with_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.roots = roots;
        self
    }

    pub fn with_oauth_handler<T: OAuthHandler + 'static>(mut self, handler: T) -> Self {
        self.oauth_handler = Some(Arc::new(handler));
        self
    }

    /// Load and merge MCP server definitions from one or more JSON files.
    ///
    /// On server name collisions across files, the rightmost file in `paths` wins.
    /// Empty `paths` is a no-op.
    pub async fn from_json_files<T: AsRef<Path>>(mut self, paths: &[T]) -> Result<Self, ParseError> {
        if paths.is_empty() {
            return Ok(self);
        }
        let raw_config = RawMcpConfig::from_json_files(paths)?;
        let mcp_configs = raw_config.into_configs(&self.factories).await?;
        self.mcp_configs.extend(mcp_configs);
        Ok(self)
    }

    /// Load MCP server definitions from a pre-parsed `RawMcpConfig`.
    ///
    /// Useful when the caller has already parsed JSON from a CLI string or
    /// other non-file source.
    pub async fn from_raw_config(mut self, raw: RawMcpConfig) -> Result<Self, ParseError> {
        let configs = raw.into_configs(&self.factories).await?;
        self.mcp_configs.extend(configs);
        Ok(self)
    }

    /// Load MCP config layers in authored order.
    ///
    /// Direct file refs and inline JSON are merged before runtime conversion, so
    /// duplicate server names produce exactly one server with the rightmost layer
    /// winning. Proxy refs remain grouped in the synthetic `"proxy"` server.
    pub async fn from_mcp_config_layers(mut self, layers: &[McpConfigLayer]) -> Result<Self, ParseError> {
        if layers.is_empty() {
            return Ok(self);
        }

        let mut direct_servers = BTreeMap::new();
        let mut proxied_paths = Vec::new();

        for layer in layers {
            match layer {
                McpConfigLayer::File(config_ref) if config_ref.proxy => {
                    proxied_paths.push(config_ref.path.as_path());
                }
                McpConfigLayer::File(config_ref) => {
                    direct_servers.extend(RawMcpConfig::from_json_files(&[config_ref.path.as_path()])?.servers);
                }
                McpConfigLayer::Json(json) => {
                    direct_servers.extend(RawMcpConfig::from_json(json)?.servers);
                }
            }
        }

        if !direct_servers.is_empty() {
            let configs = RawMcpConfig { servers: direct_servers }.into_configs(&self.factories).await?;
            self.mcp_configs.extend(configs);
        }

        if !proxied_paths.is_empty() {
            let raw_config = RawMcpConfig::from_json_files(&proxied_paths)?;
            let servers = raw_config.into_proxy_server_configs(&self.factories).await?;
            self.mcp_configs.push(McpServerConfig::ToolProxy { name: "proxy".to_string(), servers });
        }

        Ok(self)
    }

    /// Load MCP server definitions from config refs, routing proxy-flagged files
    /// through a single merged `ToolProxy`.
    ///
    /// Direct refs are loaded normally. All proxy-flagged refs are merged into
    /// one `McpServerConfig::ToolProxy` named `"proxy"`.
    pub async fn from_mcp_config_refs(self, refs: &[McpJsonFileRef]) -> Result<Self, ParseError> {
        let layers: Vec<_> = refs.iter().cloned().map(McpConfigLayer::File).collect();
        self.from_mcp_config_layers(&layers).await
    }

    pub async fn spawn(self) -> Result<McpSpawnResult, McpError> {
        let (mcp_command_tx, mcp_command_rx) = mpsc::channel::<McpCommand>(self.mcp_channel_capacity);
        let (event_tx, event_rx) = mpsc::channel::<McpClientEvent>(self.mcp_channel_capacity);

        let mut mcp_manager = McpManager::new(event_tx, self.oauth_handler);
        mcp_manager.add_mcps(self.mcp_configs).await?;

        // Set workspace roots if provided
        if !self.roots.is_empty() {
            let roots = self.roots.into_iter().map(|path| root_from_path(&path, None)).collect();
            mcp_manager.set_roots(roots).await?;
        }

        let tool_definitions = mcp_manager.tool_definitions();
        let instructions = mcp_manager.server_instructions();
        let server_statuses = mcp_manager.server_statuses().to_vec();
        let mcp_handle = tokio::spawn(run_mcp_task(mcp_manager, mcp_command_rx));

        Ok(McpSpawnResult {
            tool_definitions,
            instructions,
            server_statuses,
            command_tx: mcp_command_tx,
            event_rx,
            handle: mcp_handle,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn from_mcp_config_layers_parses_servers_and_mcpservers_aliases() {
        let servers_json = r#"{
            "servers": {
                "one": {
                    "type": "stdio",
                    "command": "echo",
                    "args": ["one"]
                }
            }
        }"#
        .to_string();
        let mcpservers_json = r#"{
            "mcpServers": {
                "two": {
                    "type": "stdio",
                    "command": "echo",
                    "args": ["two"]
                }
            }
        }"#
        .to_string();

        let builder = mcp()
            .from_mcp_config_layers(&[McpConfigLayer::Json(servers_json), McpConfigLayer::Json(mcpservers_json)])
            .await
            .unwrap();
        assert_eq!(builder.mcp_configs.len(), 2);
    }

    #[tokio::test]
    async fn from_mcp_config_layers_json_layers_in_order_last_wins() {
        let first = r#"{
            "servers": {
                "same": {
                    "type": "stdio",
                    "command": "echo",
                    "args": ["first"]
                }
            }
        }"#
        .to_string();
        let second = r#"{
            "servers": {
                "same": {
                    "type": "stdio",
                    "command": "echo",
                    "args": ["second"]
                }
            }
        }"#
        .to_string();

        let builder =
            mcp().from_mcp_config_layers(&[McpConfigLayer::Json(first), McpConfigLayer::Json(second)]).await.unwrap();
        assert_eq!(builder.mcp_configs.len(), 1);
        match &builder.mcp_configs[0] {
            McpServerConfig::Server(mcp_utils::client::ServerConfig::Stdio { args, .. }) => {
                assert_eq!(args, &vec!["second".to_string()]);
            }
            other => panic!("expected stdio config, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn from_mcp_config_layers_merges_files_and_inline_json_before_conversion() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("mcp.json");
        std::fs::write(
            &file_path,
            r#"{
                "servers": {
                    "same": {
                        "type": "stdio",
                        "command": "echo",
                        "args": ["from-file"]
                    }
                }
            }"#,
        )
        .unwrap();
        let inline = r#"{
            "servers": {
                "same": {
                    "type": "stdio",
                    "command": "echo",
                    "args": ["from-json"]
                }
            }
        }"#
        .to_string();

        let builder = mcp()
            .from_mcp_config_layers(&[
                McpConfigLayer::File(McpJsonFileRef::direct(file_path)),
                McpConfigLayer::Json(inline),
            ])
            .await
            .unwrap();

        assert_eq!(builder.mcp_configs.len(), 1);
        match &builder.mcp_configs[0] {
            McpServerConfig::Server(mcp_utils::client::ServerConfig::Stdio { args, .. }) => {
                assert_eq!(args, &vec!["from-json".to_string()]);
            }
            other => panic!("expected stdio config, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn from_mcp_config_layers_preserves_file_and_json_order() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("mcp.json");
        std::fs::write(
            &file_path,
            r#"{
                "servers": {
                    "same": {
                        "type": "stdio",
                        "command": "echo",
                        "args": ["from-file"]
                    }
                }
            }"#,
        )
        .unwrap();
        let inline = r#"{
            "servers": {
                "same": {
                    "type": "stdio",
                    "command": "echo",
                    "args": ["from-json"]
                }
            }
        }"#
        .to_string();

        let builder = mcp()
            .from_mcp_config_layers(&[
                McpConfigLayer::Json(inline),
                McpConfigLayer::File(McpJsonFileRef::direct(file_path)),
            ])
            .await
            .unwrap();

        match &builder.mcp_configs[0] {
            McpServerConfig::Server(mcp_utils::client::ServerConfig::Stdio { args, .. }) => {
                assert_eq!(args, &vec!["from-file".to_string()]);
            }
            other => panic!("expected stdio config, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn from_raw_config_adds_configs() {
        let raw = RawMcpConfig::from_json(
            r#"{
                "servers": {
                    "stdio-one": {
                        "type": "stdio",
                        "command": "echo",
                        "args": ["one"]
                    }
                }
            }"#,
        )
        .unwrap();

        let builder = mcp().from_raw_config(raw).await.unwrap();
        assert_eq!(builder.mcp_configs.len(), 1);
    }
}
