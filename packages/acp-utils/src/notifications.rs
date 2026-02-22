//! Shared wire-format types for Aether's custom ACP extension notifications.
//!
//! These types are used on both the agent (server) and client (UI) sides of the
//! ACP connection.

use agent_client_protocol::ExtNotification;
use rmcp::model::ElicitationSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Custom notification methods for sub-agent progress updates.
/// Per ACP extensibility spec, custom notifications must start with underscore.
pub const SUB_AGENT_PROGRESS_METHOD: &str = "_aether/sub_agent_progress";
pub const CONTEXT_USAGE_METHOD: &str = "_aether/context_usage";

/// Custom ext_method for tunneling MCP elicitation through ACP.
/// Note: ACP auto-prefixes ext_method names with `_`, so the wire method
/// becomes `_aether/elicitation`.
pub const ELICITATION_METHOD: &str = "aether/elicitation";

/// Parameters for `_aether/context_usage` notifications.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextUsageParams {
    pub usage_ratio: f64,
    pub tokens_used: u32,
    pub context_limit: u32,
}

/// Parameters sent via ext_method for `aether/elicitation`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ElicitationParams {
    pub message: String,
    pub schema: ElicitationSchema,
}

pub use rmcp::model::ElicitationAction;

/// Response returned from the client for an elicitation request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ElicitationResponse {
    pub action: ElicitationAction,
    /// Structured form data when action is "accept".
    pub content: Option<serde_json::Value>,
}

impl From<ContextUsageParams> for ExtNotification {
    fn from(params: ContextUsageParams) -> Self {
        let raw_value =
            serde_json::value::to_raw_value(&params).expect("ContextUsageParams is serializable");
        ExtNotification::new(CONTEXT_USAGE_METHOD, Arc::from(raw_value))
    }
}

/// Parameters for `_aether/sub_agent_progress` notifications.
///
/// This is the wire format sent from `aether-acp` to clients like `wisp`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentProgressParams {
    pub parent_tool_id: String,
    pub task_id: String,
    pub agent_name: String,
    pub event: SubAgentEvent,
}

impl From<SubAgentProgressParams> for ExtNotification {
    fn from(params: SubAgentProgressParams) -> Self {
        let raw_value = serde_json::value::to_raw_value(&params)
            .expect("SubAgentProgressParams is serializable");
        ExtNotification::new(SUB_AGENT_PROGRESS_METHOD, Arc::from(raw_value))
    }
}

/// Subset of agent message variants relevant for sub-agent status display.
///
/// The server (`aether-acp`) converts `AgentMessage` to this type before
/// serializing, so the wire format only contains these known variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubAgentEvent {
    ToolCall { request: SubAgentToolRequest },
    ToolResult { result: SubAgentToolResult },
    ToolError { error: SubAgentToolError },
    Done,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentToolRequest {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentToolResult {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentToolError {
    pub id: String,
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_constants_have_underscore_prefix() {
        assert!(SUB_AGENT_PROGRESS_METHOD.starts_with('_'));
        assert!(CONTEXT_USAGE_METHOD.starts_with('_'));
    }

    #[test]
    fn elicitation_params_roundtrip() {
        use rmcp::model::EnumSchema;

        let params = ElicitationParams {
            message: "Pick a color".to_string(),
            schema: ElicitationSchema::builder()
                .required_enum_schema(
                    "color",
                    EnumSchema::builder(vec!["red".into(), "green".into(), "blue".into()])
                        .untitled()
                        .build(),
                )
                .build()
                .unwrap(),
        };

        let json = serde_json::to_string(&params).unwrap();
        let parsed: ElicitationParams = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, params);
    }

    #[test]
    fn context_usage_params_roundtrip() {
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
    fn sub_agent_progress_params_roundtrip() {
        let params = SubAgentProgressParams {
            parent_tool_id: "call_123".to_string(),
            task_id: "task_abc".to_string(),
            agent_name: "explorer".to_string(),
            event: SubAgentEvent::Done,
        };

        let notification: ExtNotification = params.into();
        assert_eq!(notification.method.as_ref(), SUB_AGENT_PROGRESS_METHOD);

        let parsed: SubAgentProgressParams =
            serde_json::from_str(notification.params.get()).expect("valid JSON");
        assert!(matches!(parsed.event, SubAgentEvent::Done));
        assert_eq!(parsed.parent_tool_id, "call_123");
    }

    #[test]
    fn deserialize_tool_call_event() {
        let json = r#"{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{\"pattern\":\"test\"}"},"model_name":"m"}}"#;
        let event: SubAgentEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(event, SubAgentEvent::ToolCall { .. }));
    }

    #[test]
    fn deserialize_tool_result_event() {
        let json = r#"{"ToolResult":{"result":{"id":"c1","name":"grep","arguments":"{}","result":"ok"},"model_name":"m"}}"#;
        let event: SubAgentEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(event, SubAgentEvent::ToolResult { .. }));
    }

    #[test]
    fn deserialize_tool_error_event() {
        let json = r#"{"ToolError":{"error":{"id":"c1","name":"grep","arguments":"{}","error":"not found"},"model_name":"m"}}"#;
        let event: SubAgentEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(event, SubAgentEvent::ToolError { .. }));
    }

    #[test]
    fn deserialize_done_event() {
        let event: SubAgentEvent = serde_json::from_str(r#""Done""#).unwrap();
        assert!(matches!(event, SubAgentEvent::Done));
    }

    #[test]
    fn deserialize_other_variant() {
        let event: SubAgentEvent = serde_json::from_str(r#""Other""#).unwrap();
        assert!(matches!(event, SubAgentEvent::Other));
    }
}
