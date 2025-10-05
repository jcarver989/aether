mod agent;
mod agent_builder;
pub mod error;
mod iteration_state;
mod messages;
mod tool_executor_task;

pub use agent::*;
pub use agent_builder::*;
pub use error::{AgentError, Result};
pub use messages::*;
pub use tool_executor_task::ToolExecutor;

use crate::llm::ModelProvider;

pub fn agent<T: ModelProvider + 'static>(llm: T) -> AgentBuilder<T> {
    AgentBuilder::new(llm)
}
