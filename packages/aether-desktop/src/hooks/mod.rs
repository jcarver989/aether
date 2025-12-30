//! Custom hooks for the Aether desktop application.
//!
//! These hooks encapsulate reusable stateful logic, separating business
//! logic from view rendering.

mod use_agent_chat;
mod use_autocomplete;

pub use use_agent_chat::{AgentChatController, InputMode, use_agent_chat};
