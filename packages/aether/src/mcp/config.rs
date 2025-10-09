use rmcp::{
    RoleServer, service::DynService,
    transport::streamable_http_client::StreamableHttpClientTransportConfig,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level MCP configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpConfig {
    pub servers: HashMap<String, ServerDefinition>,
}

/// Server connection definition
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ServerDefinition {
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
