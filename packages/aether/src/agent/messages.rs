use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::llm::{ToolCallError, ToolCallRequest, ToolCallResult};

/// A file attachment that can be included with a user message.
///
/// File attachments are used by the `@file` mention feature to include
/// file contents in the message sent to the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileAttachment {
    /// The display path (typically relative to the working directory)
    pub path: String,
    /// The absolute path for reading the file content
    pub absolute_path: PathBuf,
    /// The file content (read at send time)
    pub content: String,
    /// Optional MIME type (e.g., "text/plain", "application/json")
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AgentMessage {
    Text {
        message_id: String,
        chunk: String,
        is_complete: bool,
        model_name: String,
    },

    ToolCall {
        request: ToolCallRequest,
        model_name: String,
    },

    ToolProgress {
        request: ToolCallRequest,
        progress: f64,
        total: Option<f64>,
        message: Option<String>,
    },

    ToolResult {
        result: ToolCallResult,
        model_name: String,
    },

    ToolError {
        error: ToolCallError,
        model_name: String,
    },

    Error {
        message: String,
    },

    Cancelled {
        message: String,
    },

    /// Context compaction has been triggered
    ContextCompactionStarted {
        message_count: usize,
    },

    /// Context was compacted to reduce token usage
    ContextCompactionResult {
        summary: String,
        messages_removed: usize,
    },

    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserMessage {
    Text {
        content: String,
        /// Optional file attachments included via @-mentions
        attachments: Vec<FileAttachment>,
    },
    Cancel,
}

impl AgentMessage {
    pub fn text(message_id: &str, chunk: &str, is_complete: bool, model_name: &str) -> Self {
        AgentMessage::Text {
            message_id: message_id.to_string(),
            chunk: chunk.to_string(),
            is_complete,
            model_name: model_name.to_string(),
        }
    }
}

impl UserMessage {
    /// Creates a text message without any file attachments.
    pub fn text(content: &str) -> Self {
        UserMessage::Text {
            content: content.to_string(),
            attachments: Vec::new(),
        }
    }

    /// Creates a text message with file attachments.
    pub fn text_with_attachments(content: &str, attachments: Vec<FileAttachment>) -> Self {
        UserMessage::Text {
            content: content.to_string(),
            attachments,
        }
    }
}

impl From<&str> for UserMessage {
    fn from(value: &str) -> Self {
        UserMessage::Text {
            content: value.to_string(),
            attachments: Vec::new(),
        }
    }
}
