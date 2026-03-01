use futures::future::BoxFuture;
use rmcp::{
    RoleServer, service::DynService,
    transport::streamable_http_client::StreamableHttpClientTransportConfig,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

use super::variables::{VarError, expand_env_vars};

/// Top-level MCP configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RawMcpConfig {
    pub servers: HashMap<String, RawMcpServerConfig>,
}

/// Server connection definition
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RawMcpServerConfig {
    Stdio {
        command: String,

        #[serde(default)]
        args: Vec<String>,

        #[serde(default)]
        env: HashMap<String, String>,
    },

    Http {
        url: String,

        #[serde(default)]
        headers: HashMap<String, String>,
    },

    Sse {
        url: String,

        #[serde(default)]
        headers: HashMap<String, String>,
    },

    /// In-memory transport (Aether extension) - requires a registered factory
    /// The factory is looked up using the server name from the mcp.json key.
    ///
    /// When `input` contains a `"servers"` key, this is treated as a tool-proxy
    /// configuration: nested server configs are parsed and validated, producing
    /// a `McpServerConfig::ToolProxy` at runtime.
    #[serde(rename = "in-memory")]
    InMemory {
        #[serde(default)]
        args: Vec<String>,

        #[serde(default)]
        input: Option<Value>,
    },
}

/// A single connectable MCP server endpoint.
pub enum ServerConfig {
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

impl ServerConfig {
    pub fn name(&self) -> &str {
        match self {
            ServerConfig::Http { name, .. }
            | ServerConfig::Stdio { name, .. }
            | ServerConfig::InMemory { name, .. } => name,
        }
    }
}

impl std::fmt::Debug for ServerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerConfig::Http { name, config } => f
                .debug_struct("Http")
                .field("name", name)
                .field("config", config)
                .finish(),
            ServerConfig::Stdio {
                name,
                command,
                args,
                env,
            } => f
                .debug_struct("Stdio")
                .field("name", name)
                .field("command", command)
                .field("args", args)
                .field("env", env)
                .finish(),
            ServerConfig::InMemory { name, .. } => f
                .debug_struct("InMemory")
                .field("name", name)
                .field("server", &"<DynService>")
                .finish(),
        }
    }
}

/// Top-level MCP config: a single server OR a tool-proxy of single servers.
pub enum McpServerConfig {
    Server(ServerConfig),
    ToolProxy {
        name: String,
        servers: Vec<ServerConfig>,
    },
}

impl McpServerConfig {
    pub fn name(&self) -> &str {
        match self {
            McpServerConfig::Server(cfg) => cfg.name(),
            McpServerConfig::ToolProxy { name, .. } => name,
        }
    }
}

impl From<ServerConfig> for McpServerConfig {
    fn from(cfg: ServerConfig) -> Self {
        McpServerConfig::Server(cfg)
    }
}

impl std::fmt::Debug for McpServerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpServerConfig::Server(cfg) => cfg.fmt(f),
            McpServerConfig::ToolProxy { name, servers } => f
                .debug_struct("ToolProxy")
                .field("name", name)
                .field("servers", &format!("{} nested servers", servers.len()))
                .finish(),
        }
    }
}

/// Factory function that creates an MCP server instance asynchronously.
/// The factory receives parsed CLI arguments and an optional structured input from
/// the `"input"` field in the config JSON.
pub type ServerFactory = Box<
    dyn Fn(Vec<String>, Option<Value>) -> BoxFuture<'static, Box<dyn DynService<RoleServer>>>
        + Send
        + Sync,
>;

#[derive(Debug)]
pub enum ParseError {
    IoError(std::io::Error),
    JsonError(serde_json::Error),
    VarError(VarError),
    FactoryNotFound(String),
    InvalidNestedConfig(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::IoError(e) => write!(f, "Failed to read config file: {e}"),
            ParseError::JsonError(e) => write!(f, "Invalid JSON: {e}"),
            ParseError::VarError(e) => write!(f, "Variable expansion failed: {e}"),
            ParseError::FactoryNotFound(name) => {
                write!(f, "InMemory server factory '{name}' not registered")
            }
            ParseError::InvalidNestedConfig(msg) => {
                write!(f, "Invalid nested config in tool-proxy: {msg}")
            }
        }
    }
}

impl std::error::Error for ParseError {}

impl From<std::io::Error> for ParseError {
    fn from(error: std::io::Error) -> Self {
        ParseError::IoError(error)
    }
}

impl From<serde_json::Error> for ParseError {
    fn from(error: serde_json::Error) -> Self {
        ParseError::JsonError(error)
    }
}

impl From<VarError> for ParseError {
    fn from(error: VarError) -> Self {
        ParseError::VarError(error)
    }
}

impl RawMcpConfig {
    /// Parse MCP configuration from a JSON file
    pub fn from_json_file(path: impl AsRef<Path>) -> Result<Self, ParseError> {
        let content = std::fs::read_to_string(path)?;
        Self::from_json(&content)
    }

    /// Parse MCP configuration from a JSON string
    pub fn from_json(json: &str) -> Result<Self, ParseError> {
        Ok(serde_json::from_str(json)?)
    }

    /// Convert to runtime configuration with the provided factory registry
    pub async fn into_configs(
        self,
        factories: &HashMap<String, ServerFactory>,
    ) -> Result<Vec<McpServerConfig>, ParseError> {
        let mut configs = Vec::with_capacity(self.servers.len());
        for (name, raw_config) in self.servers {
            configs.push(raw_config.into_config(name, factories).await?);
        }
        Ok(configs)
    }
}

impl RawMcpServerConfig {
    /// Convert to runtime configuration with the provided factory registry
    pub async fn into_config(
        self,
        name: String,
        factories: &HashMap<String, ServerFactory>,
    ) -> Result<McpServerConfig, ParseError> {
        match self {
            RawMcpServerConfig::Stdio { command, args, env } => Ok(ServerConfig::Stdio {
                name,
                command: expand_env_vars(&command)?,
                args: args
                    .into_iter()
                    .map(|a| expand_env_vars(&a))
                    .collect::<Result<Vec<_>, _>>()?,
                env: env
                    .into_iter()
                    .map(|(k, v)| Ok((k, expand_env_vars(&v)?)))
                    .collect::<Result<HashMap<_, _>, VarError>>()?,
            }
            .into()),

            RawMcpServerConfig::Http { url, headers }
            | RawMcpServerConfig::Sse { url, headers } => {
                // Extract Authorization header if present (only header currently supported)
                let auth_header = headers
                    .get("Authorization")
                    .map(|v| expand_env_vars(v))
                    .transpose()?;

                Ok(ServerConfig::Http {
                    name,
                    config: StreamableHttpClientTransportConfig {
                        uri: expand_env_vars(&url)?.into(),
                        auth_header,
                        ..Default::default()
                    },
                }
                .into())
            }

            RawMcpServerConfig::InMemory { args, input } => {
                let servers_val = input.as_ref().and_then(|v| v.get("servers"));

                if let Some(servers_val) = servers_val {
                    return parse_tool_proxy(name, servers_val, factories).await;
                }

                let server_factory = factories
                    .get(&name)
                    .ok_or_else(|| ParseError::FactoryNotFound(name.clone()))?;

                let expanded_args = args
                    .into_iter()
                    .map(|a| expand_env_vars(&a))
                    .collect::<Result<Vec<_>, VarError>>()?;

                let server = server_factory(expanded_args, input).await;
                Ok(ServerConfig::InMemory { name, server }.into())
            }
        }
    }

    /// Convert to a `ServerConfig` (non-composite). Used by `parse_tool_proxy`
    /// where the result must be a single server, not a top-level `McpServerConfig`.
    async fn into_server_config(
        self,
        name: String,
        factories: &HashMap<String, ServerFactory>,
    ) -> Result<ServerConfig, ParseError> {
        match self.into_config(name, factories).await? {
            McpServerConfig::Server(cfg) => Ok(cfg),
            McpServerConfig::ToolProxy { name, .. } => Err(ParseError::InvalidNestedConfig(
                format!("tool-proxy '{name}' cannot be nested inside another tool-proxy"),
            )),
        }
    }
}

async fn parse_tool_proxy(
    name: String,
    servers_val: &Value,
    factories: &HashMap<String, ServerFactory>,
) -> Result<McpServerConfig, ParseError> {
    let nested_raw: HashMap<String, RawMcpServerConfig> =
        serde_json::from_value(servers_val.clone()).map_err(|e| {
            ParseError::InvalidNestedConfig(format!("failed to parse input.servers: {e}"))
        })?;

    let mut nested_configs = Vec::with_capacity(nested_raw.len());
    for (nested_name, nested_raw_cfg) in nested_raw {
        if matches!(nested_raw_cfg, RawMcpServerConfig::InMemory { .. }) {
            return Err(ParseError::InvalidNestedConfig(format!(
                "in-memory servers cannot be nested inside tool-proxy (server: '{nested_name}')"
            )));
        }

        nested_configs
            .push(Box::pin(nested_raw_cfg.into_server_config(nested_name, factories)).await?);
    }

    Ok(McpServerConfig::ToolProxy {
        name,
        servers: nested_configs,
    })
}
