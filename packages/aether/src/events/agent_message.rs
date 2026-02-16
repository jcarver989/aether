use llm::{ToolCallError, ToolCallRequest, ToolCallResult};
use serde::{Deserialize, Serialize};

/// Message from the agent to the user.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AgentMessage {
    Text {
        message_id: String,
        chunk: String,
        is_complete: bool,
        model_name: String,
    },

    Thought {
        message_id: String,
        chunk: String,
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

    /// Context compaction has been triggered.
    ContextCompactionStarted {
        message_count: usize,
    },

    /// Context was compacted to reduce token usage.
    ContextCompactionResult {
        summary: String,
        messages_removed: usize,
    },

    /// Context usage update for UI display.
    ContextUsageUpdate {
        /// Current usage ratio (0.0 - 1.0).
        usage_ratio: f64,
        /// Tokens used in current context.
        tokens_used: u32,
        /// Maximum context limit.
        context_limit: u32,
    },

    /// Agent is auto-continuing because LLM stopped without completion signal.
    AutoContinue {
        /// Current attempt number (1-indexed).
        attempt: u32,
        /// Maximum allowed attempts.
        max_attempts: u32,
    },

    /// The model was successfully switched.
    ModelSwitched {
        previous: String,
        new: String,
    },

    Done,
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

    pub fn thought(message_id: &str, chunk: &str, model_name: &str) -> Self {
        AgentMessage::Thought {
            message_id: message_id.to_string(),
            chunk: chunk.to_string(),
            model_name: model_name.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AgentMessage;

    #[test]
    fn test_model_switched_serde_roundtrip() {
        let msg = AgentMessage::ModelSwitched {
            previous: "anthropic:claude-3.5-sonnet".to_string(),
            new: "ollama:llama3.2".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_thought_serde_roundtrip() {
        let msg = AgentMessage::Thought {
            message_id: "msg_1".to_string(),
            chunk: "thinking".to_string(),
            model_name: "test-model".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }
}
