mod core;
mod agent_builder;
pub mod error;
mod messages;
pub mod middleware;
mod prompt;

pub use core::*;
pub use agent_builder::*;
pub use error::{AgentError, Result};
pub use messages::*;
pub use middleware::{AgentEvent, Middleware, MiddlewareAction};
pub use prompt::*;

use crate::llm::StreamingModelProvider;

pub fn agent<T: StreamingModelProvider + 'static>(llm: T) -> AgentBuilder<T> {
    AgentBuilder::new(llm)
}
