use super::AgentMessage;
use agent_client_protocol::ExtNotification;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Custom notification method for sub-agent progress updates.
/// Per ACP extensibility spec, custom notifications must start with underscore.
pub const SUB_AGENT_PROGRESS_METHOD: &str = "_aether/sub_agent_progress";

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
    /// The tool call ID that spawned this sub-agent.
    pub parent_tool_id: String,
    /// The sub-agent's task ID.
    pub task_id: String,
    /// The name of sub-agent (e.g., "codebase-explorer").
    pub agent_name: String,
    /// The event from sub-agent.
    pub event: AgentMessage,
}

impl From<SubAgentProgressParams> for ExtNotification {
    fn from(params: SubAgentProgressParams) -> Self {
        let raw_value = serde_json::value::to_raw_value(&params)
            .expect("SubAgentProgressParams is serializable");
        ExtNotification::new(SUB_AGENT_PROGRESS_METHOD, Arc::from(raw_value))
    }
}

#[cfg(test)]
mod tests {
    use super::{SUB_AGENT_PROGRESS_METHOD, SubAgentProgressParams, SubAgentProgressPayload};
    use crate::events::AgentMessage;
    use agent_client_protocol::ExtNotification;
    use llm::{ToolCallRequest, ToolCallResult};

    #[test]
    fn test_sub_agent_progress_method_has_underscore_prefix() {
        // ACP extensibility spec requires custom notifications start with underscore.
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
