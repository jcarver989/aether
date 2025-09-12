use crate::{mcp::builtin_servers::CodingMcp, testing::InMemoryTransport};
use rmcp::{RoleClient, ServerHandler};
use std::collections::HashMap;

#[derive(Debug)]
pub enum BuiltinServer {
    Coding(CodingMcp),
}

impl BuiltinServer {
    pub fn coding() -> McpServerConfig {
        McpServerConfig::InMemoryServer {
            name: "coding".to_string(),
            server: BuiltinServer::Coding(CodingMcp::new()),
        }
    }
}

impl ServerHandler for BuiltinServer {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        match self {
            BuiltinServer::Coding(server) => ServerHandler::get_info(server),
        }
    }
}

pub enum McpServerConfig {
    Http {
        name: String,
        url: String,
        headers: HashMap<String, String>,
    },

    Stdio {
        name: String,
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    },

    InMemory {
        name: String,
        transport: InMemoryTransport<RoleClient>,
    },

    InMemoryServer {
        name: String,
        server: BuiltinServer,
    },
}

impl Default for McpServerConfig {
    fn default() -> Self {
        McpServerConfig::Http {
            name: String::new(),
            url: String::new(),
            headers: HashMap::new(),
        }
    }
}
