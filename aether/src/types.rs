use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatMessage {
    System { 
        content: String,
        timestamp: DateTime<Utc>,
    },
    User { 
        content: String,
        timestamp: DateTime<Utc>,
    },
    Assistant { 
        content: String,
        timestamp: DateTime<Utc>,
    },
    AssistantStreaming { 
        content: String,
        timestamp: DateTime<Utc>,
    },
    Tool { 
        tool_call_id: String, 
        content: String,
        timestamp: DateTime<Utc>,
    },
    ToolCall { 
        name: String, 
        params: String,
        timestamp: DateTime<Utc>,
    },
    ToolResult { 
        content: String,
        timestamp: DateTime<Utc>,
    },
    Error { 
        message: String,
        timestamp: DateTime<Utc>,
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