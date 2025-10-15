use aether::{agent::AgentBuilder, llm::StreamingModelProvider};

pub trait AgentBuilderExt {
    fn coding_tools(self) -> Self;
}

impl<T: StreamingModelProvider + 'static> AgentBuilderExt for AgentBuilder<T> {
    fn coding_tools(self) -> Self {
        // TODO: Fix this - the mcp() method was removed during Agent refactoring
        // This needs to be updated to use the new MCP configuration pattern
        // See git commit cfb3447 where the API changed
        self
    }
}
