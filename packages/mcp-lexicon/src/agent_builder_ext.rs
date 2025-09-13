use aether::{agent::AgentBuilder, llm::ModelProvider};

use crate::CodingMcp;

pub trait AgentBuilderExt {
    fn coding_tools(self) -> Self;
}

impl<T: ModelProvider + 'static> AgentBuilderExt for AgentBuilder<T> {
    fn coding_tools(self) -> Self {
        self.in_memory_mcp("coding", CodingMcp::new())
    }
}
