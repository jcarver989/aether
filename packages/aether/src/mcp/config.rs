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
    /// Standard I/O transport - communicates through stdin/stdout
    Stdio {
        /// Command to execute (e.g., "npx", "node", "python")
        command: String,
        /// Arguments to pass to the command
        #[serde(default)]
        args: Vec<String>,
        /// Environment variables for the process
        #[serde(default)]
        env: HashMap<String, String>,
    },

    /// HTTP transport
    Http {
        /// Server URL
        url: String,
        /// HTTP headers
        #[serde(default)]
        headers: HashMap<String, String>,
    },

    /// Server-Sent Events transport (maps to HTTP internally)
    Sse {
        /// Server URL
        url: String,
        /// HTTP headers
        #[serde(default)]
        headers: HashMap<String, String>,
    },

    /// In-memory transport (Aether extension) - requires a registered factory
    #[serde(rename = "inmemory")]
    InMemory {
        /// Registry key for the server factory
        factory: String,
    },
}
