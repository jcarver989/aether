mod agent_builder;
mod core;
mod error;
pub mod middleware;
mod prompt;
mod substitution;

pub use agent_builder::*;
pub use agent_events::{AgentMessage, UserMessage};
pub use core::*;
pub use error::*;
pub use middleware::{AgentEvent, Middleware, MiddlewareAction};
pub use prompt::*;
pub use substitution::*;

use crate::StreamingModelProvider;

pub fn agent<T: StreamingModelProvider + 'static>(llm: T) -> AgentBuilder<T> {
    AgentBuilder::new(llm)
}
