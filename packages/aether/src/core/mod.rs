mod agent;
mod agent_builder;
mod error;
pub mod middleware;
mod prompt;
mod substitution;

pub use crate::events::{AgentMessage, UserMessage};
pub use agent::*;
pub use agent_builder::*;
pub use error::*;
pub use middleware::{AgentEvent, Middleware, MiddlewareAction};
pub use prompt::*;
pub use substitution::*;

use llm::StreamingModelProvider;

pub fn agent(llm: impl StreamingModelProvider + 'static) -> AgentBuilder {
    AgentBuilder::new(Box::new(llm))
}
