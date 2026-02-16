//! Shared types for agent events.
//!
//! This module provides types used across multiple Aether packages:
//! - Agent message types (`AgentMessage`, `UserMessage`)
//! - ACP protocol extension payloads (`ContextUsageParams`, `SubAgentProgressParams`)

mod agent_message;
mod context_usage;
mod sub_agent_progress;
mod user_message;

pub use agent_message::AgentMessage;
pub use context_usage::{CONTEXT_USAGE_METHOD, ContextUsageParams};
pub use sub_agent_progress::{
    SUB_AGENT_PROGRESS_METHOD, SubAgentProgressParams, SubAgentProgressPayload,
};
pub use user_message::UserMessage;
