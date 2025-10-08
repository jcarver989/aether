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
