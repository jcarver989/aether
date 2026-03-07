use serde::{Deserialize, Serialize};

use crate::types::IsoString;

use super::{ToolCallError, ToolCallRequest, ToolCallResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ChatMessage {
    System {
        content: String,
        timestamp: IsoString,
    },
    User {
        content: String,
        timestamp: IsoString,
    },
    Assistant {
        content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reasoning_content: Option<String>,
        timestamp: IsoString,
        tool_calls: Vec<ToolCallRequest>,
    },
    ToolCallResult(Result<ToolCallResult, ToolCallError>),
    Error {
        message: String,
        timestamp: IsoString,
    },
    /// A compacted summary of previous conversation history.
    /// This replaces multiple messages with a structured summary to reduce context usage.
    Summary {
        content: String,
        timestamp: IsoString,
        /// Number of messages that were compacted into this summary
        messages_compacted: usize,
    },
}

impl ChatMessage {
    /// Returns true if this message is a tool call result
    pub fn is_tool_result(&self) -> bool {
        matches!(self, ChatMessage::ToolCallResult(_))
    }

    /// Returns true if this message is a system prompt
    pub fn is_system(&self) -> bool {
        matches!(self, ChatMessage::System { .. })
    }

    /// Returns true if this message is a compacted summary
    pub fn is_summary(&self) -> bool {
        matches!(self, ChatMessage::Summary { .. })
    }

    /// Rough byte-size estimate of the message content for pre-flight context checks.
    /// Not meant to be exact — just close enough to detect overflow before calling the LLM.
    pub fn estimated_bytes(&self) -> usize {
        match self {
            ChatMessage::System { content, .. }
            | ChatMessage::User { content, .. }
            | ChatMessage::Error {
                message: content, ..
            }
            | ChatMessage::Summary { content, .. } => content.len(),
            ChatMessage::Assistant {
                content,
                reasoning_content,
                tool_calls,
                ..
            } => {
                content.len()
                    + reasoning_content.as_ref().map_or(0, String::len)
                    + tool_calls
                        .iter()
                        .map(|tc| tc.name.len() + tc.arguments.len())
                        .sum::<usize>()
            }
            ChatMessage::ToolCallResult(Ok(result)) => {
                result.name.len() + result.arguments.len() + result.result.len()
            }
            ChatMessage::ToolCallResult(Err(error)) => {
                error.name.len()
                    + error.arguments.as_ref().map_or(0, String::len)
                    + error.error.len()
            }
        }
    }

    /// Returns the timestamp of this message, if it has one
    pub fn timestamp(&self) -> Option<&IsoString> {
        match self {
            ChatMessage::System { timestamp, .. }
            | ChatMessage::User { timestamp, .. }
            | ChatMessage::Assistant { timestamp, .. }
            | ChatMessage::Error { timestamp, .. }
            | ChatMessage::Summary { timestamp, .. } => Some(timestamp),
            ChatMessage::ToolCallResult(_) => None,
        }
    }
}
