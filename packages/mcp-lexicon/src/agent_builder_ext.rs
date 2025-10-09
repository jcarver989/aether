use aether::{agent::AgentBuilder, llm::StreamingModelProvider, mcp::config::McpServerConfig};
use rmcp::ServiceExt;

use crate::CodingMcp;

pub trait AgentBuilderExt {
    fn coding_tools(self) -> Self;
}

impl<T: StreamingModelProvider + 'static> AgentBuilderExt for AgentBuilder<T> {
    fn coding_tools(self) -> Self {
        self.mcp(McpServerConfig::InMemory {
            name: "coding_mcp".to_string(),
            server: CodingMcp::new().into_dyn(),
        })
    }
}
