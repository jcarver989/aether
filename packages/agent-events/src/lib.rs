//! Shared types for agent events.
//!
//! This crate provides types used across multiple Aether packages:
//! - Tool execution types (ToolCallRequest, ToolCallResult, ToolCallError)
//! - Agent message types (AgentMessage, UserMessage)
//! - ACP protocol extension types (ContextUsageParams, SubAgentProgressParams)

use agent_client_protocol::ExtNotification;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Custom notification method for context usage updates.
/// Per ACP extensibility spec, custom notifications must start with underscore.
pub const CONTEXT_USAGE_METHOD: &str = "_aether/context_usage";

/// Custom notification method for sub-agent progress updates.
/// Per ACP extensibility spec, custom notifications must start with underscore.
pub const SUB_AGENT_PROGRESS_METHOD: &str = "_aether/sub_agent_progress";

/// Tool call request from the LLM
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// Successful result of a tool call
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub result: String,
}

/// Error result of a tool call
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallError {
    pub id: String,
    pub name: String,
    pub arguments: Option<String>,
    pub error: String,
}

/// Message from the agent to the user
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

    /// Context usage update for UI display
    ContextUsageUpdate {
        /// Current usage ratio (0.0 - 1.0)
        usage_ratio: f64,
        /// Tokens used in current context
        tokens_used: u32,
        /// Maximum context limit
        context_limit: u32,
    },

    /// Agent is auto-continuing because LLM stopped without completion signal
    AutoContinue {
        /// Current attempt number (1-indexed)
        attempt: u32,
        /// Maximum allowed attempts
        max_attempts: u32,
    },

    Done,
}

/// Message from the user to the agent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserMessage {
    Text { content: String },
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
    pub fn text(content: &str) -> Self {
        UserMessage::Text {
            content: content.to_string(),
        }
    }
}

impl From<&str> for UserMessage {
    fn from(value: &str) -> Self {
        UserMessage::Text {
            content: value.to_string(),
        }
    }
}

/// Parameters for context usage update notifications.
///
/// This type is used for serialization/deserialization of `_aether/context_usage`
/// custom notification payload on both agent (server) and client sides.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextUsageParams {
    pub usage_ratio: f64,
    pub tokens_used: u32,
    pub context_limit: u32,
}

impl From<ContextUsageParams> for ExtNotification {
    fn from(params: ContextUsageParams) -> Self {
        let raw_value =
            serde_json::value::to_raw_value(&params).expect("ContextUsageParams is serializable");
        ExtNotification {
            method: Arc::from(CONTEXT_USAGE_METHOD),
            params: Arc::from(raw_value),
        }
    }
}

/// Payload for sub-agent progress updates emitted by MCP tools.
///
/// This payload is embedded in MCP progress messages and does not include
/// the parent tool call ID (that is provided by the caller).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubAgentProgressPayload {
    pub task_id: String,
    pub agent_name: String,
    pub event: AgentMessage,
}

/// Parameters for sub-agent progress update notifications.
///
/// This type is used for serialization/deserialization of `_aether/sub_agent_progress`
/// custom notification payload on both agent (server) and client sides.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubAgentProgressParams {
    /// The tool call ID that spawned this sub-agent
    pub parent_tool_id: String,
    /// The sub-agent's task ID
    pub task_id: String,
    /// The name of sub-agent (e.g., "codebase-explorer")
    pub agent_name: String,
    /// The event from sub-agent
    pub event: AgentMessage,
}

impl From<SubAgentProgressParams> for ExtNotification {
    fn from(params: SubAgentProgressParams) -> Self {
        let raw_value = serde_json::value::to_raw_value(&params)
            .expect("SubAgentProgressParams is serializable");
        ExtNotification {
            method: Arc::from(SUB_AGENT_PROGRESS_METHOD),
            params: Arc::from(raw_value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_usage_params_roundtrip() {
        let params = ContextUsageParams {
            usage_ratio: 0.75,
            tokens_used: 75000,
            context_limit: 100000,
        };

        let notification: ExtNotification = params.clone().into();

        assert_eq!(notification.method.as_ref(), CONTEXT_USAGE_METHOD);

        let parsed: ContextUsageParams =
            serde_json::from_str(notification.params.get()).expect("valid JSON");
        assert_eq!(parsed, params);
    }

    #[test]
    fn test_context_usage_method_has_underscore_prefix() {
        // ACP extensibility spec requires custom notifications start with underscore
        assert!(
            CONTEXT_USAGE_METHOD.starts_with('_'),
            "Custom notification methods must start with underscore per ACP spec"
        );
    }

    #[test]
    fn test_sub_agent_progress_method_has_underscore_prefix() {
        // ACP extensibility spec requires custom notifications start with underscore
        assert!(
            SUB_AGENT_PROGRESS_METHOD.starts_with('_'),
            "Custom notification methods must start with underscore per ACP spec"
        );
    }

    #[test]
    fn test_sub_agent_progress_payload_roundtrip() {
        let payload = SubAgentProgressPayload {
            task_id: "task_123".to_string(),
            agent_name: "explorer".to_string(),
            event: AgentMessage::Done,
        };

        let json = serde_json::to_string(&payload).expect("serializable");
        let parsed: SubAgentProgressPayload = serde_json::from_str(&json).expect("deserializable");

        assert_eq!(payload, parsed);
    }

    #[test]
    fn test_sub_agent_progress_params_roundtrip() {
        let params = SubAgentProgressParams {
            parent_tool_id: "call_parent".to_string(),
            task_id: "task_abc123".to_string(),
            agent_name: "codebase-explorer".to_string(),
            event: AgentMessage::Text {
                message_id: "msg_1".to_string(),
                chunk: "Working on it...".to_string(),
                is_complete: false,
                model_name: "test-model".to_string(),
            },
        };

        let notification: ExtNotification = params.clone().into();

        assert_eq!(notification.method.as_ref(), SUB_AGENT_PROGRESS_METHOD);

        let parsed: SubAgentProgressParams =
            serde_json::from_str(notification.params.get()).expect("valid JSON");
        assert_eq!(parsed, params);
    }

    #[test]
    fn test_sub_agent_progress_params_with_tool_call() {
        let params = SubAgentProgressParams {
            parent_tool_id: "call_parent".to_string(),
            task_id: "task_abc123".to_string(),
            agent_name: "codebase-explorer".to_string(),
            event: AgentMessage::ToolCall {
                request: ToolCallRequest {
                    id: "call_sub".to_string(),
                    name: "grep".to_string(),
                    arguments: r#"{"pattern": "test"}"#.to_string(),
                },
                model_name: "test-model".to_string(),
            },
        };

        let notification: ExtNotification = params.clone().into();

        let parsed: SubAgentProgressParams =
            serde_json::from_str(notification.params.get()).expect("valid JSON");
        assert_eq!(parsed, params);
    }

    #[test]
    fn test_sub_agent_progress_params_with_tool_result() {
        let params = SubAgentProgressParams {
            parent_tool_id: "call_parent".to_string(),
            task_id: "task_abc123".to_string(),
            agent_name: "codebase-explorer".to_string(),
            event: AgentMessage::ToolResult {
                result: ToolCallResult {
                    id: "call_sub".to_string(),
                    name: "grep".to_string(),
                    arguments: "{}".to_string(),
                    result: "Found 5 matches".to_string(),
                },
                model_name: "test-model".to_string(),
            },
        };

        let notification: ExtNotification = params.clone().into();

        let parsed: SubAgentProgressParams =
            serde_json::from_str(notification.params.get()).expect("valid JSON");
        assert_eq!(parsed, params);
    }
}
