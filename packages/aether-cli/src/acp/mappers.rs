use acp_utils::notifications::{ContextClearedParams, ContextUsageParams, SubAgentProgressParams};
use aether_core::events::{AgentMessage, SubAgentProgressPayload};
use agent_client_protocol::{
    self as acp, Content, ContentBlock, ContentChunk, HttpHeader, McpServer, PlanEntry,
    PlanEntryPriority, PlanEntryStatus, SessionId, SessionNotification, SessionUpdate, StopReason,
    TextContent, ToolCall, ToolCallContent, ToolCallId, ToolCallStatus, ToolCallUpdate,
    ToolCallUpdateFields,
};
use llm::{ToolCallError, ToolCallRequest, ToolCallResult};
use mcp_utils::client::{McpServerConfig, ServerConfig};
use mcp_utils::display_meta::{PlanMetaStatus, ToolResultMeta};
use rmcp::model::Prompt as McpPrompt;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;

/// Converts an MCP Prompt to an ACP `AvailableCommand`
///
/// Strips the MCP namespace from the prompt name (e.g., "`coding__web`" -> "web")
/// and creates a slash command that clients can invoke.
pub fn map_mcp_prompt_to_available_command(prompt: &McpPrompt) -> acp::AvailableCommand {
    // Extract the base command name by removing the namespace prefix
    let command_name = prompt
        .name
        .split("__")
        .last()
        .unwrap_or(prompt.name.as_ref())
        .to_string();

    // Determine if the command takes input based on whether it has arguments
    // For slash commands, we always allow input (arguments after the command name)
    let input = if let Some(args) = &prompt.arguments {
        if args.is_empty() {
            // Even if no formal arguments, provide a generic hint
            Some(acp::AvailableCommandInput::Unstructured(
                acp::UnstructuredCommandInput::new("optional arguments"),
            ))
        } else {
            // Create a hint from the argument names
            let hint = args
                .iter()
                .map(|arg| arg.name.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            Some(acp::AvailableCommandInput::Unstructured(
                acp::UnstructuredCommandInput::new(hint),
            ))
        }
    } else {
        // No arguments defined, provide a generic hint for optional input
        Some(acp::AvailableCommandInput::Unstructured(
            acp::UnstructuredCommandInput::new("optional arguments"),
        ))
    };

    let description = prompt
        .description
        .clone()
        .unwrap_or_else(|| "No description available".to_string());

    acp::AvailableCommand::new(command_name, description).input(input)
}

/// Maps ACP MCP server definitions to internal `McpServerConfig`, skipping unsupported transports.
pub fn map_acp_mcp_servers(servers: Vec<McpServer>) -> Vec<McpServerConfig> {
    servers
        .into_iter()
        .filter_map(|s| {
            try_map_mcp_server(s).or_else(|| {
                tracing::warn!("Unsupported ACP MCP server transport, skipping");
                None
            })
        })
        .collect()
}

fn try_map_mcp_server(server: McpServer) -> Option<McpServerConfig> {
    use McpServer::{Http, Sse, Stdio};
    match server {
        Stdio(stdio) => Some(
            ServerConfig::Stdio {
                name: stdio.name,
                command: stdio.command.to_string_lossy().into_owned(),
                args: stdio.args,
                env: stdio.env.into_iter().map(|e| (e.name, e.value)).collect(),
            }
            .into(),
        ),

        Http(http) => Some(
            ServerConfig::Http {
                name: http.name,
                config: http_config(http.url, &http.headers),
            }
            .into(),
        ),

        Sse(sse) => Some(
            ServerConfig::Http {
                name: sse.name,
                config: http_config(sse.url, &sse.headers),
            }
            .into(),
        ),

        _ => None,
    }
}

fn http_config(url: String, headers: &[HttpHeader]) -> StreamableHttpClientTransportConfig {
    let auth_header = headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("authorization"))
        .map(|h| h.value.clone());

    StreamableHttpClientTransportConfig {
        uri: url.into(),
        auth_header,
        ..Default::default()
    }
}

/// Converts Aether `AgentMessage` to ACP `SessionUpdate`
pub fn map_agent_message_to_session_notification(
    session_id: SessionId,
    msg: &AgentMessage,
) -> Option<SessionNotification> {
    match msg {
        AgentMessage::Text {
            chunk, is_complete, ..
        } => map_text_to_notification(session_id, chunk, *is_complete),

        AgentMessage::Thought {
            chunk,
            is_complete: false,
            ..
        } => Some(map_thought_to_notification(session_id, chunk)),

        AgentMessage::ToolCall { request, .. } => {
            Some(map_tool_call_to_notification(session_id, request))
        }

        AgentMessage::ToolResult {
            result,
            result_meta,
            ..
        } => Some(map_tool_result_to_notification(
            session_id,
            result,
            result_meta.as_ref(),
        )),

        AgentMessage::ToolError { error, .. } => {
            Some(map_tool_error_to_notification(session_id, error))
        }

        AgentMessage::ToolProgress {
            request,
            progress,
            total,
            message,
        } => map_tool_progress_to_notification(
            session_id,
            request,
            *progress,
            *total,
            message.as_ref(),
        ),

        AgentMessage::Error { message } => Some(acp::SessionNotification::new(
            session_id,
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new(format!("[Error] {message}")),
            ))),
        )),

        AgentMessage::Thought {
            is_complete: true, ..
        }
        | AgentMessage::ContextUsageUpdate { .. }
        | AgentMessage::ContextCleared
        | AgentMessage::Cancelled { .. }
        | AgentMessage::Done
        | AgentMessage::ContextCompactionStarted { .. }
        | AgentMessage::ContextCompactionResult { .. }
        | AgentMessage::AutoContinue { .. }
        | AgentMessage::ModelSwitched { .. } => None,
    }
}

pub fn try_into_ext_notification(msg: &AgentMessage) -> Option<acp::ExtNotification> {
    match msg {
        AgentMessage::ContextUsageUpdate {
            usage_ratio,
            tokens_used,
            context_limit,
        } => {
            let params = ContextUsageParams {
                usage_ratio: *usage_ratio,
                tokens_used: *tokens_used,
                context_limit: *context_limit,
            };
            Some(params.into())
        }
        AgentMessage::ToolProgress {
            request, message, ..
        } => {
            let msg_str = message.as_ref()?;
            let params = try_parse_sub_agent_progress(msg_str, request)?;
            Some(params.into())
        }
        AgentMessage::ContextCleared => Some(ContextClearedParams::default().into()),
        _ => None,
    }
}

/// If the tool result carries plan metadata, build a `SessionUpdate::Plan` notification.
pub fn try_extract_plan_notification(
    session_id: SessionId,
    result_meta: Option<&ToolResultMeta>,
) -> Option<SessionNotification> {
    let plan_meta = result_meta?.plan.as_ref()?;
    let entries = plan_meta
        .entries
        .iter()
        .map(|e| {
            PlanEntry::new(
                e.content.clone(),
                PlanEntryPriority::Medium,
                plan_status_to_acp(e.status),
            )
        })
        .collect();
    Some(SessionNotification::new(
        session_id,
        SessionUpdate::Plan(acp::Plan::new(entries)),
    ))
}

/// Convert internal plan status to ACP protocol status.
fn plan_status_to_acp(status: PlanMetaStatus) -> PlanEntryStatus {
    match status {
        PlanMetaStatus::InProgress => PlanEntryStatus::InProgress,
        PlanMetaStatus::Completed => PlanEntryStatus::Completed,
        PlanMetaStatus::Pending => PlanEntryStatus::Pending,
    }
}

/// Determines the stop reason from the final agent message
pub fn map_agent_message_to_stop_reason(msg: &AgentMessage) -> acp::StopReason {
    match msg {
        AgentMessage::Cancelled { .. } => StopReason::Cancelled,
        _ => StopReason::EndTurn,
    }
}

fn map_text_to_notification(
    session_id: SessionId,
    chunk: &str,
    is_complete: bool,
) -> Option<SessionNotification> {
    // Skip the final completion message to avoid sending duplicate content.
    // The client has already received all the chunks during streaming.
    if is_complete {
        return None;
    }

    Some(acp::SessionNotification::new(
        session_id,
        SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(TextContent::new(
            chunk.to_owned(),
        )))),
    ))
}

fn map_thought_to_notification(session_id: SessionId, chunk: &str) -> SessionNotification {
    acp::SessionNotification::new(
        session_id,
        SessionUpdate::AgentThoughtChunk(ContentChunk::new(ContentBlock::Text(TextContent::new(
            chunk.to_owned(),
        )))),
    )
}

fn map_tool_call_to_notification(
    session_id: SessionId,
    request: &ToolCallRequest,
) -> SessionNotification {
    let raw_input = serde_json::from_str(&request.arguments).ok();
    SessionNotification::new(
        session_id,
        SessionUpdate::ToolCall(
            ToolCall::new(
                ToolCallId::new(request.id.clone()),
                humanize_tool_name(&request.name),
            )
            .status(acp::ToolCallStatus::InProgress)
            .raw_input(raw_input),
        ),
    )
}

/// Produces the initial human-readable title for a tool call (e.g., "Read file").
/// This is sent when the tool call starts.
fn humanize_tool_name(name: &str) -> String {
    let base = name.split("__").last().unwrap_or(name);
    let mut result = base.replace('_', " ");
    if let Some(first) = result.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    result
}

fn map_tool_result_to_notification(
    session_id: SessionId,
    result: &ToolCallResult,
    result_meta: Option<&ToolResultMeta>,
) -> SessionNotification {
    let fields = ToolCallUpdateFields::new()
        .status(ToolCallStatus::Completed)
        .content(vec![ToolCallContent::Content(Content::new(
            ContentBlock::Text(TextContent::new(result.result.clone())),
        ))]);

    let mut update = ToolCallUpdate::new(ToolCallId::new(result.id.clone()), fields);

    if let Some(rm) = result_meta {
        update = update.meta(rm.clone().into_map());
    }

    SessionNotification::new(session_id, SessionUpdate::ToolCallUpdate(update))
}

fn map_tool_error_to_notification(
    session_id: SessionId,
    error: &ToolCallError,
) -> SessionNotification {
    SessionNotification::new(
        session_id,
        SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
            ToolCallId::new(error.id.clone()),
            ToolCallUpdateFields::new()
                .status(ToolCallStatus::Failed)
                .content(vec![ToolCallContent::Content(Content::new(
                    ContentBlock::Text(TextContent::new(error.error.clone())),
                ))]),
        )),
    )
}

fn map_tool_progress_to_notification(
    session_id: SessionId,
    request: &ToolCallRequest,
    progress: f64,
    total: Option<f64>,
    message: Option<&String>,
) -> Option<SessionNotification> {
    tracing::info!("Tool progress: {message:?}");

    if message
        .and_then(|msg_str| try_parse_sub_agent_progress(msg_str, request))
        .is_some()
    {
        return None;
    }

    if let Some(result_meta) = message.and_then(|m| try_parse_display_meta(m)) {
        return Some(SessionNotification::new(
            session_id,
            SessionUpdate::ToolCallUpdate(
                ToolCallUpdate::new(
                    ToolCallId::new(request.id.clone()),
                    ToolCallUpdateFields::new().status(ToolCallStatus::InProgress),
                )
                .meta(result_meta.into_map()),
            ),
        ));
    }

    let total_str = total.map_or_else(|| "?".to_string(), |t| t.to_string());
    let progress_text = message.map_or_else(
        || format!("Progress: {progress}/{total_str}"),
        |msg| format!("{msg} ({progress}/{total_str})"),
    );

    Some(SessionNotification::new(
        session_id,
        SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
            ToolCallId::new(request.id.clone()),
            ToolCallUpdateFields::new()
                .status(ToolCallStatus::InProgress)
                .content(vec![ToolCallContent::Content(Content::new(
                    ContentBlock::Text(TextContent::new(progress_text)),
                ))]),
        )),
    ))
}

fn try_parse_display_meta(message: &str) -> Option<ToolResultMeta> {
    serde_json::from_str::<ToolResultMeta>(message).ok()
}

/// Attempt to parse a tool progress message as sub-agent progress.
fn try_parse_sub_agent_progress(
    message: &str,
    request: &llm::ToolCallRequest,
) -> Option<SubAgentProgressParams> {
    let payload: SubAgentProgressPayload = serde_json::from_str(message).ok()?;

    Some(SubAgentProgressParams {
        parent_tool_id: request.id.clone(),
        task_id: payload.task_id,
        agent_name: payload.agent_name,
        event: (&payload.event).into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::events::SUB_AGENT_PROGRESS_METHOD;
    use llm::ToolCallRequest;

    #[test]
    fn test_tool_progress_with_sub_agent_payload_emits_ext_notification() {
        let session_id = acp::SessionId::new("test-session");

        let payload = SubAgentProgressPayload {
            task_id: "task_1".to_string(),
            agent_name: "sub-agent".to_string(),
            event: AgentMessage::Text {
                message_id: "msg_1".to_string(),
                chunk: "Hello".to_string(),
                is_complete: false,
                model_name: "TestModel".to_string(),
            },
        };
        let serialized_msg = serde_json::to_string(&payload).unwrap();

        let tool_progress = AgentMessage::ToolProgress {
            request: ToolCallRequest {
                id: "call_123".to_string(),
                name: "plugins__spawn_subagent".to_string(),
                arguments: "{}".to_string(),
            },
            progress: 42.0,
            total: Some(100.0),
            message: Some(serialized_msg.to_string()),
        };

        let notification =
            map_agent_message_to_session_notification(session_id.clone(), &tool_progress);

        assert!(notification.is_none());

        let ext = try_into_ext_notification(&tool_progress).expect("ext notification");
        assert_eq!(ext.method.as_ref(), SUB_AGENT_PROGRESS_METHOD);

        let parsed: SubAgentProgressParams =
            serde_json::from_str(ext.params.get()).expect("valid JSON");
        assert_eq!(parsed.parent_tool_id, "call_123");
        assert_eq!(parsed.task_id, "task_1");
        assert_eq!(parsed.agent_name, "sub-agent");
        assert!(matches!(
            parsed.event,
            acp_utils::notifications::SubAgentEvent::Other
        ));
    }

    #[test]
    fn test_thought_maps_to_agent_thought_chunk() {
        let session_id = acp::SessionId::new("test-session");
        let thought = AgentMessage::Thought {
            message_id: "msg_1".to_string(),
            chunk: "thinking...".to_string(),
            is_complete: false,
            model_name: "TestModel".to_string(),
        };

        let notification =
            map_agent_message_to_session_notification(session_id, &thought).expect("notification");

        match notification.update {
            acp::SessionUpdate::AgentThoughtChunk(chunk) => match chunk.content {
                acp::ContentBlock::Text(text) => assert_eq!(text.text, "thinking..."),
                other => panic!("Expected text content, got {other:?}"),
            },
            other => panic!("Expected AgentThoughtChunk, got {other:?}"),
        }
    }

    #[test]
    fn test_context_cleared_maps_to_ext_notification() {
        let ext = try_into_ext_notification(&AgentMessage::ContextCleared)
            .expect("context cleared should emit ext notification");
        assert_eq!(
            ext.method.as_ref(),
            acp_utils::notifications::CONTEXT_CLEARED_METHOD
        );

        let parsed: acp_utils::notifications::ContextClearedParams =
            serde_json::from_str(ext.params.get()).expect("valid JSON");
        assert_eq!(
            parsed,
            acp_utils::notifications::ContextClearedParams::default()
        );
    }

    #[test]
    fn test_tool_progress_with_invalid_json_falls_back_to_simple_message() {
        let session_id = acp::SessionId::new("test-session");

        // Simulate a tool progress message with invalid JSON
        let tool_progress = AgentMessage::ToolProgress {
            request: ToolCallRequest {
                id: "call_456".to_string(),
                name: "some_tool".to_string(),
                arguments: "{}".to_string(),
            },
            progress: 50.0,
            total: None,
            message: Some("not valid json".to_string()),
        };

        let notification =
            map_agent_message_to_session_notification(session_id.clone(), &tool_progress);

        assert!(notification.is_some());

        // Should still produce a notification with the message as-is
        let notification = notification.unwrap();
        match notification.update {
            acp::SessionUpdate::ToolCallUpdate(update) => {
                if let Some(content) = &update.fields.content {
                    if let acp::ToolCallContent::Content(c) = &content[0] {
                        if let acp::ContentBlock::Text(text) = &c.content {
                            // Should contain the original message
                            assert!(text.text.contains("not valid json"));
                        }
                    }
                }
            }
            _ => panic!("Expected ToolCallUpdate"),
        }
    }

    #[test]
    fn test_map_acp_stdio_server() {
        let server = acp::McpServer::Stdio(
            acp::McpServerStdio::new("my-server", "/usr/bin/server")
                .args(vec!["--port".into(), "8080".into()])
                .env(vec![acp::EnvVariable::new("FOO", "bar")]),
        );

        let configs = map_acp_mcp_servers(vec![server]);
        assert_eq!(configs.len(), 1);

        match &configs[0] {
            McpServerConfig::Server(ServerConfig::Stdio {
                name,
                command,
                args,
                env,
            }) => {
                assert_eq!(name, "my-server");
                assert_eq!(command, "/usr/bin/server");
                assert_eq!(args, &["--port", "8080"]);
                assert_eq!(env.get("FOO").unwrap(), "bar");
            }
            other => panic!("Expected Stdio, got {:?}", other),
        }
    }

    #[test]
    fn test_map_acp_http_server() {
        let server = acp::McpServer::Http(
            acp::McpServerHttp::new("http-server", "https://example.com/mcp").headers(vec![
                acp::HttpHeader::new("Authorization", "Bearer token123"),
            ]),
        );

        let configs = map_acp_mcp_servers(vec![server]);
        assert_eq!(configs.len(), 1);

        match &configs[0] {
            McpServerConfig::Server(ServerConfig::Http { name, config }) => {
                assert_eq!(name, "http-server");
                assert_eq!(config.uri.as_ref(), "https://example.com/mcp");
                assert_eq!(config.auth_header.as_deref(), Some("Bearer token123"));
            }
            other => panic!("Expected Http, got {:?}", other),
        }
    }

    #[test]
    fn test_map_acp_sse_server() {
        let server = acp::McpServer::Sse(acp::McpServerSse::new(
            "sse-server",
            "https://example.com/sse",
        ));

        let configs = map_acp_mcp_servers(vec![server]);
        assert_eq!(configs.len(), 1);

        match &configs[0] {
            McpServerConfig::Server(ServerConfig::Http { name, config }) => {
                assert_eq!(name, "sse-server");
                assert_eq!(config.uri.as_ref(), "https://example.com/sse");
                assert_eq!(config.auth_header, None);
            }
            other => panic!("Expected Http, got {:?}", other),
        }
    }

    #[test]
    fn test_humanize_tool_name_strips_namespace() {
        assert_eq!(humanize_tool_name("coding__read_file"), "Read file");
    }

    #[test]
    fn test_humanize_tool_name_no_namespace() {
        assert_eq!(humanize_tool_name("read_file"), "Read file");
    }

    #[test]
    fn test_humanize_tool_name_single_word() {
        assert_eq!(humanize_tool_name("bash"), "Bash");
    }

    #[test]
    fn test_humanize_tool_name_deeply_nested() {
        assert_eq!(
            humanize_tool_name("plugins__coding__read_file"),
            "Read file"
        );
    }

    #[test]
    fn test_result_with_result_meta_sets_meta() {
        use mcp_utils::display_meta::ToolDisplayMeta;

        let session_id = acp::SessionId::new("test-session");
        let result = ToolCallResult {
            id: "call_1".to_string(),
            name: "coding__read_file".to_string(),
            arguments: "{}".to_string(),
            result: "file contents".to_string(),
        };
        let rm: ToolResultMeta = ToolDisplayMeta::new("Read file", "Cargo.toml, 156 lines").into();

        let notification = map_tool_result_to_notification(session_id, &result, Some(&rm));
        match notification.update {
            acp::SessionUpdate::ToolCallUpdate(update) => {
                assert!(update.fields.title.is_none());
                let meta = update.meta.expect("meta should be present");
                let tc_meta =
                    ToolResultMeta::from_map(&meta).expect("should deserialize to ToolResultMeta");
                assert_eq!(tc_meta.display.title, "Read file");
                assert_eq!(tc_meta.display.value, "Cargo.toml, 156 lines");
            }
            other => panic!("Expected ToolCallUpdate, got {other:?}"),
        }
    }

    #[test]
    fn test_result_without_result_meta() {
        let session_id = acp::SessionId::new("test-session");
        let result = ToolCallResult {
            id: "call_1".to_string(),
            name: "external__some_tool".to_string(),
            arguments: "{}".to_string(),
            result: "ok".to_string(),
        };

        let notification = map_tool_result_to_notification(session_id, &result, None);
        match notification.update {
            acp::SessionUpdate::ToolCallUpdate(update) => {
                assert!(update.fields.title.is_none());
                assert!(update.meta.is_none());
            }
            other => panic!("Expected ToolCallUpdate, got {other:?}"),
        }
    }

    #[test]
    fn test_plan_notification_extracted_from_result_meta() {
        use mcp_utils::display_meta::{PlanMeta, PlanMetaEntry, PlanMetaStatus, ToolDisplayMeta};

        let session_id = acp::SessionId::new("test-session");
        let meta = ToolResultMeta::with_plan(
            ToolDisplayMeta::new("Todo", "Research AI agents"),
            PlanMeta {
                entries: vec![
                    PlanMetaEntry {
                        content: "Research AI agents".to_string(),
                        status: PlanMetaStatus::InProgress,
                    },
                    PlanMetaEntry {
                        content: "Write tests".to_string(),
                        status: PlanMetaStatus::Pending,
                    },
                ],
            },
        );

        let notification =
            try_extract_plan_notification(session_id, Some(&meta)).expect("should produce plan");
        match notification.update {
            acp::SessionUpdate::Plan(plan) => {
                assert_eq!(plan.entries.len(), 2);
                assert_eq!(plan.entries[0].content, "Research AI agents");
                assert_eq!(plan.entries[0].status, acp::PlanEntryStatus::InProgress);
                assert_eq!(plan.entries[1].content, "Write tests");
                assert_eq!(plan.entries[1].status, acp::PlanEntryStatus::Pending);
            }
            other => panic!("Expected Plan, got {other:?}"),
        }
    }

    #[test]
    fn test_plan_notification_none_when_no_plan() {
        use mcp_utils::display_meta::ToolDisplayMeta;

        let session_id = acp::SessionId::new("test-session");
        let meta: ToolResultMeta = ToolDisplayMeta::new("Read file", "main.rs").into();
        assert!(try_extract_plan_notification(session_id, Some(&meta)).is_none());
    }

    #[test]
    fn test_plan_notification_none_when_no_meta() {
        let session_id = acp::SessionId::new("test-session");
        assert!(try_extract_plan_notification(session_id, None).is_none());
    }

    #[test]
    fn test_tool_progress_with_display_meta_emits_meta_update() {
        use mcp_utils::display_meta::ToolDisplayMeta;

        let session_id = acp::SessionId::new("test-session");
        let meta = ToolResultMeta::from(ToolDisplayMeta::new("Read file", "main.rs"));
        let serialized = serde_json::to_string(&meta).unwrap();

        let request = ToolCallRequest {
            id: "call_789".to_string(),
            name: "coding__read_file".to_string(),
            arguments: "{}".to_string(),
        };

        let notification =
            map_tool_progress_to_notification(session_id, &request, 0.0, None, Some(&serialized))
                .expect("should produce notification");

        match notification.update {
            acp::SessionUpdate::ToolCallUpdate(update) => {
                assert_eq!(&*update.tool_call_id.0, "call_789");
                let meta_map = update.meta.expect("meta should be present");
                let parsed =
                    ToolResultMeta::from_map(&meta_map).expect("should parse as ToolResultMeta");
                assert_eq!(parsed.display.title, "Read file");
                assert_eq!(parsed.display.value, "main.rs");
                assert_eq!(update.fields.status, Some(acp::ToolCallStatus::InProgress));
                // Should NOT have content (no text progress fallback)
                assert!(update.fields.content.is_none());
            }
            other => panic!("Expected ToolCallUpdate, got {other:?}"),
        }
    }
}
