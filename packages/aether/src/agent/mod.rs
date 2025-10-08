mod agent;
mod agent_builder;
pub mod error;
mod messages;

pub use agent::*;
pub use agent_builder::*;
pub use error::{AgentError, Result};
pub use messages::*;

use crate::llm::StreamingModelProvider;

pub fn agent<T: StreamingModelProvider + 'static>(llm: T) -> AgentBuilder<T> {
    AgentBuilder::new(llm)
}
