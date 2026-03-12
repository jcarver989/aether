use serde::{Deserialize, Serialize};

use super::ToolCallRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    Length,
    ToolCalls,
    ContentFilter,
    FunctionCall,
    Error,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LlmResponse {
    Start {
        message_id: String,
    },
    Text {
        chunk: String,
    },
    Reasoning {
        chunk: String,
    },
    EncryptedReasoning {
        id: String,
        content: String,
    },
    ToolRequestStart {
        id: String,
        name: String,
    },
    ToolRequestArg {
        id: String,
        chunk: String,
    },
    ToolRequestComplete {
        tool_call: ToolCallRequest,
    },
    Done {
        stop_reason: Option<StopReason>,
    },
    Error {
        message: String,
    },
    Usage {
        input_tokens: u32,
        output_tokens: u32,
    },
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

    pub fn reasoning(chunk: &str) -> Self {
        Self::Reasoning {
            chunk: chunk.to_string(),
        }
    }

    pub fn encrypted_reasoning(id: &str, encrypted: &str) -> Self {
        Self::EncryptedReasoning {
            id: id.to_string(),
            content: encrypted.to_string(),
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

    pub fn usage(input_tokens: u32, output_tokens: u32) -> Self {
        Self::Usage {
            input_tokens,
            output_tokens,
        }
    }

    pub fn done() -> Self {
        Self::Done { stop_reason: None }
    }

    pub fn done_with_stop_reason(stop_reason: StopReason) -> Self {
        Self::Done {
            stop_reason: Some(stop_reason),
        }
    }
}
