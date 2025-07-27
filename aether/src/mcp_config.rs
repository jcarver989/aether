use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpServerConfig {
    // HTTP-based MCP server (for mesh/web servers)
    Http {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
    },
    // Process-based MCP server (for local tools)
    Process {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
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

impl McpServerConfig {
    pub fn is_http(&self) -> bool {
        matches!(self, McpServerConfig::Http { .. })
    }
    
    pub fn is_process(&self) -> bool {
        matches!(self, McpServerConfig::Process { .. })
    }
    
    pub fn url(&self) -> Option<&str> {
        match self {
            McpServerConfig::Http { url, .. } => Some(url),
            _ => None,
        }
    }
    
    pub fn command(&self) -> Option<&str> {
        match self {
            McpServerConfig::Process { command, .. } => Some(command),
            _ => None,
        }
    }
    
    pub fn args(&self) -> Option<&[String]> {
        match self {
            McpServerConfig::Process { args, .. } => Some(args),
            _ => None,
        }
    }
}