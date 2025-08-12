use std::time::SystemTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatMessage {
    System {
        content: String,
        timestamp: SystemTime,
    },
    User {
        content: String,
        timestamp: SystemTime,
    },
    Assistant {
        content: String,
        timestamp: SystemTime,
    },
    AssistantStreaming {
        content: String,
        timestamp: SystemTime,
    },
    Tool {
        tool_call_id: String,
        content: String,
        timestamp: SystemTime,
    },
    ToolCall {
        id: String,
        name: String,
        params: String,
        timestamp: SystemTime,
    },
    ToolResult {
        tool_call_id: String,
        content: String,
        timestamp: SystemTime,
    },
    Error {
        message: String,
        timestamp: SystemTime,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCallState {
    Pending,
    Running,
    Completed,
    Failed,
}
