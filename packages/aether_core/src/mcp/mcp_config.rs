use crate::testing::InMemoryTransport;
use rmcp::RoleClient;
use std::collections::HashMap;

pub enum McpServerConfig {
    Http {
        url: String,
        headers: HashMap<String, String>,
    },

    Stdio {
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    },

    InMemory {
        transport: InMemoryTransport<RoleClient>,
    },
}

impl Default for McpServerConfig {
    fn default() -> Self {
        McpServerConfig::Http {
            url: String::new(),
            headers: HashMap::new(),
        }
    }
}

impl McpServerConfig {}
