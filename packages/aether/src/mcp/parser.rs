use rmcp::{RoleServer, service::DynService};
use std::collections::HashMap;
use std::path::Path;

use super::config::*;
use super::variables::{VarError, expand_env_vars};

impl McpConfigParser {
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    pub fn parse_json_file(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<Vec<McpServerConfig>, ParseError> {
        let content = std::fs::read_to_string(path)?;
        self.parse_json(&content)
    }

    pub fn parse_json(&self, json: &str) -> Result<Vec<McpServerConfig>, ParseError> {
        let config: McpConfig = serde_json::from_str(json)?;

        let mut results = Vec::new();
        for (name, server_def) in config.servers {
            let mcp_config = self.to_mcp_server_config(name, server_def)?;
            results.push(mcp_config);
        }

        Ok(results)
    }

    pub fn register_in_memory_server(
        &mut self,
        name: impl Into<String>,
        factory: ServerFactory,
    ) -> &mut Self {
        self.factories.insert(name.into(), factory);
        self
    }

    fn to_mcp_server_config(
        &self,
        name: String,
        def: ServerDefinition,
    ) -> Result<McpServerConfig, ParseError> {
        match def {
            ServerDefinition::Stdio { command, args, env } => Ok(McpServerConfig::Stdio {
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

            ServerDefinition::Http { url, headers } | ServerDefinition::Sse { url, headers } => {
                use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;

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

            ServerDefinition::InMemory { server_name } => {
                let server_factory = self
                    .factories
                    .get(&server_name)
                    .ok_or_else(|| ParseError::FactoryNotFound(server_name.clone()))?;
                let server = server_factory();
                Ok(McpServerConfig::InMemory { name, server })
            }
        }
    }
}

/// Factory function that creates an MCP server instance
pub type ServerFactory = Box<dyn Fn() -> Box<dyn DynService<RoleServer>> + Send + Sync>;

/// Parser for MCP JSON configuration files
pub struct McpConfigParser {
    factories: HashMap<String, ServerFactory>,
}

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
            ParseError::IoError(e) => write!(f, "Failed to read config file: {}", e),
            ParseError::JsonError(e) => write!(f, "Invalid JSON: {}", e),
            ParseError::VarError(e) => write!(f, "Variable expansion failed: {}", e),
            ParseError::FactoryNotFound(name) => {
                write!(f, "InMemory server factory '{}' not registered", name)
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
