//! Shared wire-format types for Aether's custom ACP extension notifications.
//!
//! These types are used on both the agent (server) and client (UI) sides of the
//! ACP connection.

use agent_client_protocol::{AuthMethod, ExtNotification};
pub use mcp_utils::display_meta::{ToolDisplayMeta, ToolResultMeta};
use rmcp::model::ElicitationSchema;
use serde::{Deserialize, Serialize};
use serde_json::value::to_raw_value;
use std::fmt;
use std::sync::Arc;

pub use mcp_utils::status::{McpServerStatus, McpServerStatusEntry};

/// Custom notification methods for sub-agent progress updates.
/// Per ACP extensibility spec, custom notifications must start with underscore.
pub const SUB_AGENT_PROGRESS_METHOD: &str = "_aether/sub_agent_progress";
pub const CONTEXT_USAGE_METHOD: &str = "_aether/context_usage";
pub const CONTEXT_CLEARED_METHOD: &str = "_aether/context_cleared";
pub const MCP_MESSAGE_METHOD: &str = "_aether/mcp";
pub const AUTH_METHODS_UPDATED_METHOD: &str = "_aether/auth_methods_updated";

/// Custom `ext_method` for tunneling MCP elicitation through ACP.
/// Note: ACP auto-prefixes `ext_method` names with `_`, so the wire method
/// becomes `_aether/elicitation`.
pub const ELICITATION_METHOD: &str = "aether/elicitation";

/// Parameters for `_aether/context_usage` notifications.
///
/// `cache_read_tokens`, `cache_creation_tokens`, and `reasoning_tokens` come
/// from the most recent API response and are optional because not every
/// provider exposes them. They give clients enough signal to render
/// cache-hit ratios and reasoning-token spend without re-parsing a stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextUsageParams {
    pub usage_ratio: Option<f64>,
    pub tokens_used: u32,
    pub context_limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u32>,
}

/// Parameters for `_aether/context_cleared` notifications.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ContextClearedParams {}

/// Parameters for `_aether/auth_methods_updated` notifications.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthMethodsUpdatedParams {
    pub auth_methods: Vec<AuthMethod>,
}

/// Parameters sent via `ext_method` for `aether/elicitation`.
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

/// Server→client MCP extension notifications (relay → wisp).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum McpNotification {
    ServerStatus { servers: Vec<McpServerStatusEntry> },
}

impl From<McpNotification> for ExtNotification {
    fn from(msg: McpNotification) -> Self {
        ext_notification(MCP_MESSAGE_METHOD, &msg)
    }
}

/// Client→server MCP extension requests (wisp → relay).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum McpRequest {
    Authenticate { session_id: String, server_name: String },
}

impl From<McpRequest> for ExtNotification {
    fn from(msg: McpRequest) -> Self {
        ext_notification(MCP_MESSAGE_METHOD, &msg)
    }
}

/// Error returned when converting an `ExtNotification` into a typed MCP message.
#[derive(Debug)]
pub enum ExtNotificationParseError {
    WrongMethod,
    InvalidJson(serde_json::Error),
}

impl fmt::Display for ExtNotificationParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongMethod => write!(f, "notification method is not {MCP_MESSAGE_METHOD}"),
            Self::InvalidJson(e) => write!(f, "invalid JSON params: {e}"),
        }
    }
}

impl TryFrom<&ExtNotification> for McpRequest {
    type Error = ExtNotificationParseError;

    fn try_from(n: &ExtNotification) -> Result<Self, Self::Error> {
        if n.method.as_ref() != MCP_MESSAGE_METHOD {
            return Err(ExtNotificationParseError::WrongMethod);
        }
        serde_json::from_str(n.params.get()).map_err(ExtNotificationParseError::InvalidJson)
    }
}

impl TryFrom<&ExtNotification> for McpNotification {
    type Error = ExtNotificationParseError;

    fn try_from(n: &ExtNotification) -> Result<Self, Self::Error> {
        if n.method.as_ref() != MCP_MESSAGE_METHOD {
            return Err(ExtNotificationParseError::WrongMethod);
        }
        serde_json::from_str(n.params.get()).map_err(ExtNotificationParseError::InvalidJson)
    }
}

impl TryFrom<&ExtNotification> for AuthMethodsUpdatedParams {
    type Error = ExtNotificationParseError;

    fn try_from(n: &ExtNotification) -> Result<Self, Self::Error> {
        if n.method.as_ref() != AUTH_METHODS_UPDATED_METHOD {
            return Err(ExtNotificationParseError::WrongMethod);
        }
        serde_json::from_str(n.params.get()).map_err(ExtNotificationParseError::InvalidJson)
    }
}

fn ext_notification<T: Serialize>(method: &str, params: &T) -> ExtNotification {
    let raw_value = to_raw_value(params).expect("notification params are serializable");
    ExtNotification::new(method, Arc::from(raw_value))
}

impl From<ContextUsageParams> for ExtNotification {
    fn from(params: ContextUsageParams) -> Self {
        ext_notification(CONTEXT_USAGE_METHOD, &params)
    }
}

impl From<ContextClearedParams> for ExtNotification {
    fn from(params: ContextClearedParams) -> Self {
        ext_notification(CONTEXT_CLEARED_METHOD, &params)
    }
}

impl From<AuthMethodsUpdatedParams> for ExtNotification {
    fn from(params: AuthMethodsUpdatedParams) -> Self {
        ext_notification(AUTH_METHODS_UPDATED_METHOD, &params)
    }
}

/// Parameters for `_aether/sub_agent_progress` notifications.
///
/// This is the wire format sent from the ACP server (`aether-cli`) to clients like `wisp`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentProgressParams {
    pub parent_tool_id: String,
    pub task_id: String,
    pub agent_name: String,
    pub event: SubAgentEvent,
}

impl From<SubAgentProgressParams> for ExtNotification {
    fn from(params: SubAgentProgressParams) -> Self {
        ext_notification(SUB_AGENT_PROGRESS_METHOD, &params)
    }
}

/// Subset of agent message variants relevant for sub-agent status display.
///
/// The ACP server (`aether-cli`) converts `AgentMessage` to this type before
/// serializing, so the wire format only contains these known variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubAgentEvent {
    ToolCall { request: SubAgentToolRequest },
    ToolCallUpdate { update: SubAgentToolCallUpdate },
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
pub struct SubAgentToolCallUpdate {
    pub id: String,
    pub chunk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentToolResult {
    pub id: String,
    pub name: String,
    pub result_meta: Option<ToolResultMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentToolError {
    pub id: String,
    pub name: String,
}

#[cfg(test)]
mod tests {
    use agent_client_protocol::AuthMethodAgent;
    use serde_json::from_str;

    use super::*;

    #[test]
    fn method_constants_have_underscore_prefix() {
        assert!(SUB_AGENT_PROGRESS_METHOD.starts_with('_'));
        assert!(CONTEXT_USAGE_METHOD.starts_with('_'));
        assert!(CONTEXT_CLEARED_METHOD.starts_with('_'));
        assert!(MCP_MESSAGE_METHOD.starts_with('_'));
        assert!(AUTH_METHODS_UPDATED_METHOD.starts_with('_'));
    }

    #[test]
    fn mcp_request_authenticate_roundtrip() {
        let msg = McpRequest::Authenticate {
            session_id: "session-0".to_string(),
            server_name: "my oauth server".to_string(),
        };

        let notification: ExtNotification = msg.clone().into();
        assert_eq!(notification.method.as_ref(), MCP_MESSAGE_METHOD);

        let parsed: McpRequest = serde_json::from_str(notification.params.get()).expect("valid JSON");
        assert_eq!(parsed, msg);
    }

    #[test]
    fn mcp_notification_server_status_roundtrip() {
        let msg = McpNotification::ServerStatus {
            servers: vec![
                McpServerStatusEntry {
                    name: "github".to_string(),
                    status: McpServerStatus::Connected { tool_count: 5 },
                },
                McpServerStatusEntry { name: "linear".to_string(), status: McpServerStatus::NeedsOAuth },
                McpServerStatusEntry {
                    name: "slack".to_string(),
                    status: McpServerStatus::Failed { error: "connection timeout".to_string() },
                },
            ],
        };

        let notification: ExtNotification = msg.clone().into();
        assert_eq!(notification.method.as_ref(), MCP_MESSAGE_METHOD);

        let parsed: McpNotification = serde_json::from_str(notification.params.get()).expect("valid JSON");
        assert_eq!(parsed, msg);
    }

    #[test]
    fn auth_methods_updated_params_roundtrip() {
        let params = AuthMethodsUpdatedParams {
            auth_methods: vec![
                AuthMethod::Agent(AuthMethodAgent::new("anthropic", "Anthropic").description("authenticated")),
                AuthMethod::Agent(AuthMethodAgent::new("openrouter", "OpenRouter")),
            ],
        };

        let notification: ExtNotification = params.clone().into();
        let parsed: AuthMethodsUpdatedParams = from_str(notification.params.get()).expect("valid JSON");

        assert_eq!(parsed, params);
        assert_eq!(notification.method.as_ref(), AUTH_METHODS_UPDATED_METHOD);
    }

    #[test]
    fn mcp_server_status_entry_serde_roundtrip() {
        let entry = McpServerStatusEntry {
            name: "test-server".to_string(),
            status: McpServerStatus::Connected { tool_count: 3 },
        };

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: McpServerStatusEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, entry);
    }

    #[test]
    fn elicitation_params_roundtrip() {
        use rmcp::model::EnumSchema;

        let params = ElicitationParams {
            message: "Pick a color".to_string(),
            schema: ElicitationSchema::builder()
                .required_enum_schema(
                    "color",
                    EnumSchema::builder(vec!["red".into(), "green".into(), "blue".into()]).untitled().build(),
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
            usage_ratio: Some(0.75),
            tokens_used: 75000,
            context_limit: Some(100_000),
            cache_read_tokens: Some(40_000),
            cache_creation_tokens: Some(2_000),
            reasoning_tokens: Some(500),
        };

        let notification: ExtNotification = params.clone().into();
        assert_eq!(notification.method.as_ref(), CONTEXT_USAGE_METHOD);

        let parsed: ContextUsageParams = serde_json::from_str(notification.params.get()).expect("valid JSON");
        assert_eq!(parsed, params);
    }

    #[test]
    fn context_usage_params_omits_unset_optional_token_fields() {
        let params = ContextUsageParams {
            usage_ratio: Some(0.1),
            tokens_used: 100,
            context_limit: Some(1_000),
            cache_read_tokens: None,
            cache_creation_tokens: None,
            reasoning_tokens: None,
        };

        let notification: ExtNotification = params.clone().into();
        let raw = notification.params.get();
        assert!(!raw.contains("cache_read_tokens"));
        assert!(!raw.contains("cache_creation_tokens"));
        assert!(!raw.contains("reasoning_tokens"));
    }

    #[test]
    fn context_cleared_params_roundtrip() {
        let params = ContextClearedParams::default();

        let notification: ExtNotification = params.clone().into();
        assert_eq!(notification.method.as_ref(), CONTEXT_CLEARED_METHOD);

        let parsed: ContextClearedParams = serde_json::from_str(notification.params.get()).expect("valid JSON");
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

        let parsed: SubAgentProgressParams = serde_json::from_str(notification.params.get()).expect("valid JSON");
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
    fn deserialize_tool_call_update_event() {
        // "model_name" is present because the wire format comes from AgentMessage serialization;
        // SubAgentEvent::ToolCallUpdate has no model_name field, so serde silently ignores it.
        let json = r#"{"ToolCallUpdate":{"update":{"id":"c1","chunk":"{\"pattern\":\"test\"}"},"model_name":"m"}}"#;
        let event: SubAgentEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(event, SubAgentEvent::ToolCallUpdate { .. }));
    }

    #[test]
    fn deserialize_tool_result_event() {
        let json = r#"{"ToolResult":{"result":{"id":"c1","name":"grep","result_meta":{"display":{"title":"Grep","value":"'test' in src (3 matches)"}}}}}"#;
        let event: SubAgentEvent = serde_json::from_str(json).unwrap();
        match event {
            SubAgentEvent::ToolResult { result } => {
                let result_meta = result.result_meta.expect("expected result_meta");
                assert_eq!(result_meta.display.title, "Grep");
            }
            other => panic!("Expected ToolResult, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_tool_error_event() {
        let json = r#"{"ToolError":{"error":{"id":"c1","name":"grep"}}}"#;
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

    #[test]
    fn tool_result_meta_map_roundtrip() {
        let meta: ToolResultMeta = ToolDisplayMeta::new("Read file", "Cargo.toml, 156 lines").into();
        let map = meta.clone().into_map();
        let parsed = ToolResultMeta::from_map(&map).expect("should deserialize ToolResultMeta");
        assert_eq!(parsed, meta);
    }

    #[test]
    fn mcp_request_try_from_roundtrip() {
        let msg = McpRequest::Authenticate {
            session_id: "session-0".to_string(),
            server_name: "my oauth server".to_string(),
        };

        let notification: ExtNotification = msg.clone().into();
        let parsed = McpRequest::try_from(&notification).expect("should parse McpRequest");
        assert_eq!(parsed, msg);
    }

    #[test]
    fn mcp_notification_try_from_roundtrip() {
        let msg = McpNotification::ServerStatus {
            servers: vec![McpServerStatusEntry {
                name: "github".to_string(),
                status: McpServerStatus::Connected { tool_count: 5 },
            }],
        };

        let notification: ExtNotification = msg.clone().into();
        let parsed = McpNotification::try_from(&notification).expect("should parse McpNotification");
        assert_eq!(parsed, msg);
    }

    #[test]
    fn auth_methods_updated_try_from_roundtrip() {
        let params = AuthMethodsUpdatedParams {
            auth_methods: vec![AuthMethod::Agent(
                AuthMethodAgent::new("anthropic", "Anthropic").description("authenticated"),
            )],
        };

        let notification: ExtNotification = params.clone().into();
        let parsed = AuthMethodsUpdatedParams::try_from(&notification).expect("should parse auth methods");
        assert_eq!(parsed, params);
    }

    #[test]
    fn try_from_wrong_method_returns_error() {
        let notification = ext_notification(
            CONTEXT_USAGE_METHOD,
            &ContextUsageParams {
                usage_ratio: Some(0.5),
                tokens_used: 50000,
                context_limit: Some(100_000),
                cache_read_tokens: None,
                cache_creation_tokens: None,
                reasoning_tokens: None,
            },
        );

        let result = McpRequest::try_from(&notification);
        assert!(matches!(result, Err(ExtNotificationParseError::WrongMethod)));
    }

    #[test]
    fn try_from_invalid_json_returns_error() {
        let notification = ext_notification(MCP_MESSAGE_METHOD, &"not a valid McpRequest");

        let result = McpRequest::try_from(&notification);
        assert!(matches!(result, Err(ExtNotificationParseError::InvalidJson(_))));
    }

    #[test]
    fn ext_notification_parse_error_display() {
        let wrong = ExtNotificationParseError::WrongMethod;
        assert!(wrong.to_string().contains(MCP_MESSAGE_METHOD));

        let json_err = serde_json::from_str::<McpRequest>("{}").unwrap_err();
        let invalid = ExtNotificationParseError::InvalidJson(json_err);
        assert!(invalid.to_string().contains("invalid JSON"));
    }
}
