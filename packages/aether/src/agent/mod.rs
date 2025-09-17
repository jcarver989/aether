mod agent;
mod agent_builder;
mod agent_task;
mod elicitation_task;
mod messages;
mod tool_execution_task;

pub use agent::*;
pub use agent_builder::*;
pub use messages::*;

use crate::llm::ModelProvider;

pub fn agent<T: ModelProvider + 'static>(llm: T) -> AgentBuilder<T> {
    AgentBuilder::new(llm)
}
