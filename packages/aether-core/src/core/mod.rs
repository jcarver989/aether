mod agent;
mod agent_builder;
mod error;
mod prompt;
mod retry_config;

pub use crate::events::{AgentMessage, UserMessage};
pub use agent::*;
pub use agent_builder::*;
pub use error::*;
pub use prompt::*;
pub use retry_config::RetryConfig;

use llm::StreamingModelProvider;
use std::sync::Arc;

pub fn agent(llm: impl StreamingModelProvider + 'static) -> AgentBuilder {
    AgentBuilder::new(Arc::new(llm))
}
