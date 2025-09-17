mod agent;
mod agent_builder;
mod elicitation_task;
mod messages;
// mod process_user_message_task;  // Temporarily commented out during refactor

pub use agent::*;
pub use agent_builder::*;
pub use messages::*;

use crate::llm::ModelProvider;

pub fn agent<T: ModelProvider + 'static>(llm: T) -> AgentBuilder<T> {
    AgentBuilder::new(llm)
}
