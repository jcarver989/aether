use std::fmt;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AgentSpawnError {
    ProviderParse(String),
    McpConfigParse(String),
    McpSpawn(String),
    AgentSpawn(String),
}

impl fmt::Display for AgentSpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProviderParse(msg) => write!(f, "Failed to parse provider: {msg}"),
            Self::McpConfigParse(msg) => write!(f, "Failed to parse MCP config: {msg}"),
            Self::McpSpawn(msg) => write!(f, "Failed to spawn MCP: {msg}"),
            Self::AgentSpawn(msg) => write!(f, "Failed to spawn agent: {msg}"),
        }
    }
}

impl std::error::Error for AgentSpawnError {}
