//! Tool display components for rendering tool call results.
//!
//! This module contains specialized components for rendering different
//! types of tool display metadata in a human-friendly way.

pub mod bash_display;
pub mod file_op_display;
pub mod sub_agent_display;
pub mod todo_display;
pub mod types;
pub use bash_display::BashDisplay;
pub use file_op_display::{EditFileDisplay, ReadFileDisplay, WriteFileDisplay};
pub use sub_agent_display::{AgentMessageList, SubAgentDisplay};
pub use todo_display::TodoDisplay;
pub use types::ToolDisplayMeta;

/// Truncate a string for display, adding "..." if truncated.
pub fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}
