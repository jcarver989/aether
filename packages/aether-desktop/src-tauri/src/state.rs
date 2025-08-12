use std::sync::Arc;
use tokio::sync::Mutex;
use aether_core::{
    agent::Agent,
    llm::LlmProvider,
    tools::ToolRegistry,
};

pub struct AgentState {
    pub agent: Arc<Mutex<Option<Agent<Box<dyn LlmProvider>>>>>,
    pub tool_registry: Arc<Mutex<ToolRegistry>>,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            agent: Arc::new(Mutex::new(None)),
            tool_registry: Arc::new(Mutex::new(ToolRegistry::new())),
        }
    }
}