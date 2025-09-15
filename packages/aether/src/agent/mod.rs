mod agent;
mod agent_builder;
mod messages;

pub use agent::*;
pub use agent_builder::*;
pub use messages::*;

use crate::llm::ModelProvider;

pub fn agent<T: ModelProvider + 'static>(llm: T) -> AgentBuilder<T> {
    AgentBuilder::new(llm)
}
