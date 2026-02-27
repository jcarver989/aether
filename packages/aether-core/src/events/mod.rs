//! Shared types for agent events.
//!
//! This module provides types used across multiple Aether packages:
//! - Agent message types (`AgentMessage`, `UserMessage`)
//! - ACP protocol extension payloads (`SubAgentProgressPayload`)

mod agent_message;
mod sub_agent_progress;
mod user_message;

pub use agent_message::AgentMessage;
pub use sub_agent_progress::{SUB_AGENT_PROGRESS_METHOD, SubAgentProgressPayload};
pub use user_message::UserMessage;
