use serde::{Deserialize, Serialize};

use super::ToolCallRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LlmResponse {
    Start { message_id: String },
    Text { chunk: String },
    ToolRequestStart { id: String, name: String },
    ToolRequestArg { id: String, chunk: String },
    ToolRequestComplete { tool_call: ToolCallRequest },
    Done,
    Error { message: String },
}

impl LlmResponse {
    pub fn start(message_id: &str) -> Self {
        Self::Start {
            message_id: message_id.to_string(),
        }
    }

    pub fn text(chunk: &str) -> Self {
        Self::Text {
            chunk: chunk.to_string(),
        }
    }

    pub fn tool_request_start(id: &str, name: &str) -> Self {
        Self::ToolRequestStart {
            id: id.to_string(),
            name: name.to_string(),
        }
    }

    pub fn tool_request_arg(id: &str, chunk: &str) -> Self {
        Self::ToolRequestArg {
            id: id.to_string(),
            chunk: chunk.to_string(),
        }
    }

    pub fn tool_request_complete(id: &str, name: &str, arguments: &str) -> Self {
        Self::ToolRequestComplete {
            tool_call: ToolCallRequest {
                id: id.to_string(),
                name: name.to_string(),
                arguments: arguments.to_string(),
            },
        }
    }
}
