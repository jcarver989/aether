mod agent;
mod agent_builder;
pub mod error;
mod llm_stream_processor;
mod messages;
mod process_llm_stream_task;
mod tool_executor_task;

pub use agent::*;
pub use agent_builder::*;
pub use error::{AgentError, Result};
pub use llm_stream_processor::LlmStreamProcessor;
pub use messages::*;
pub use tool_executor_task::ToolExecutor;

use crate::llm::ModelProvider;

pub fn agent<T: ModelProvider + 'static>(llm: T) -> AgentBuilder<T> {
    AgentBuilder::new(llm)
}
