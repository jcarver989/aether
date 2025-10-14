use rmcp::{
    RoleServer, service::DynService,
    transport::streamable_http_client::StreamableHttpClientTransportConfig,
};
use serde::{Deserialize, Serialize};
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
    #[serde(rename = "inmemory")]
    InMemory {
        /// Registry key for the server factory
        #[serde(rename = "factory")]
        server_name: String,
    },
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

impl McpServerConfig {
    pub fn name(&self) -> &str {
        match self {
            McpServerConfig::Http { name, .. } => name,
            McpServerConfig::Stdio { name, .. } => name,
            McpServerConfig::InMemory { name, .. } => name,
        }
    }
}

impl std::fmt::Debug for McpServerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpServerConfig::Http { name, config } => f
                .debug_struct("Http")
                .field("name", name)
                .field("config", config)
                .finish(),
            McpServerConfig::Stdio {
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
            McpServerConfig::InMemory { name, .. } => f
                .debug_struct("InMemory")
                .field("name", name)
                .field("server", &"<DynService>")
                .finish(),
        }
    }
}

/// Factory function that creates an MCP server instance
pub type ServerFactory = Box<dyn Fn() -> Box<dyn DynService<RoleServer>> + Send + Sync>;

#[derive(Debug)]
pub enum ParseError {
    IoError(std::io::Error),
    JsonError(serde_json::Error),
    VarError(VarError),
    FactoryNotFound(String),
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
    pub fn into_configs(
        self,
        factories: &HashMap<String, ServerFactory>,
    ) -> Result<Vec<McpServerConfig>, ParseError> {
        self.servers
            .into_iter()
            .map(|(name, raw_config)| raw_config.into_config(name, factories))
            .collect()
    }
}

impl RawMcpServerConfig {
    /// Convert to runtime configuration with the provided factory registry
    pub fn into_config(
        self,
        name: String,
        factories: &HashMap<String, ServerFactory>,
    ) -> Result<McpServerConfig, ParseError> {
        match self {
            RawMcpServerConfig::Stdio { command, args, env } => Ok(McpServerConfig::Stdio {
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
            }),

            RawMcpServerConfig::Http { url, headers }
            | RawMcpServerConfig::Sse { url, headers } => {
                // Extract Authorization header if present (only header currently supported)
                let auth_header = headers
                    .get("Authorization")
                    .map(|v| expand_env_vars(v))
                    .transpose()?;

                Ok(McpServerConfig::Http {
                    name,
                    config: StreamableHttpClientTransportConfig {
                        uri: expand_env_vars(&url)?.into(),
                        auth_header,
                        ..Default::default()
                    },
                })
            }

            RawMcpServerConfig::InMemory { server_name } => {
                let server_factory = factories
                    .get(&server_name)
                    .ok_or_else(|| ParseError::FactoryNotFound(server_name.clone()))?;
                let server = server_factory();
                Ok(McpServerConfig::InMemory { name, server })
            }
        }
    }
}
