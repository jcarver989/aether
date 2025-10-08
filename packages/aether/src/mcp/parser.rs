use std::collections::HashMap;
use std::path::Path;

use super::config::*;
use super::manager::McpServerConfig;
use super::server_registry::{McpServerRegistry, RegistryError};
use super::variables::{VarError, expand_env_vars};

/// Parser for MCP JSON configuration files
pub struct McpConfigParser {
    registry: McpServerRegistry,
}

impl McpConfigParser {
    /// Create a new parser with a custom server registry
    pub fn new(registry: McpServerRegistry) -> Self {
        Self { registry }
    }

    /// Create a parser with an empty registry
    pub fn default() -> Self {
        Self::new(McpServerRegistry::new())
    }

    /// Parse an MCP configuration file
    pub async fn parse_file(&self, path: impl AsRef<Path>) -> Result<Vec<McpServerConfig>, ParseError> {
        let content = tokio::fs::read_to_string(path).await?;
        self.parse_string(&content)
    }

    /// Parse an MCP configuration from a JSON string
    pub fn parse_string(&self, json: &str) -> Result<Vec<McpServerConfig>, ParseError> {
        let config: McpConfig = serde_json::from_str(json)?;

        let mut results = Vec::new();
        for (name, server_def) in config.servers {
            let mcp_config = self.convert_server_definition(name, server_def)?;
            results.push(mcp_config);
        }

        Ok(results)
    }

    fn convert_server_definition(
        &self,
        name: String,
        def: ServerDefinition,
    ) -> Result<McpServerConfig, ParseError> {
        match def {
            ServerDefinition::Stdio { command, args, env } => {
                Ok(McpServerConfig::Stdio {
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
                })
            }

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

            ServerDefinition::InMemory { factory } => {
                let server = self.registry.create(&factory)?;
                Ok(McpServerConfig::InMemory { name, server })
            }
        }
    }
}

#[derive(Debug)]
pub enum ParseError {
    IoError(std::io::Error),
    JsonError(serde_json::Error),
    VarError(VarError),
    RegistryError(RegistryError),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::IoError(e) => write!(f, "Failed to read config file: {}", e),
            ParseError::JsonError(e) => write!(f, "Invalid JSON: {}", e),
            ParseError::VarError(e) => write!(f, "Variable expansion failed: {}", e),
            ParseError::RegistryError(e) => write!(f, "Registry error: {}", e),
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

impl From<RegistryError> for ParseError {
    fn from(error: RegistryError) -> Self {
        ParseError::RegistryError(error)
    }
}
