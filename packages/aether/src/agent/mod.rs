mod agent;
mod agent_builder;
mod agent_task;
pub mod error;
mod messages;
mod process_llm_stream_task;
mod tool_executor_task;
// mod process_user_message_task;  // Temporarily commented out during refactor

pub use agent::*;
pub use agent_builder::*;
pub use error::{AgentError, Result};
pub use messages::*;

use crate::llm::ModelProvider;

pub fn agent<T: ModelProvider + 'static>(llm: T) -> AgentBuilder<T> {
    AgentBuilder::new(llm)
}
