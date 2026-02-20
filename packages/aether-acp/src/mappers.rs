use aether::events::AgentMessage;
use aether::events::{ContextUsageParams, SubAgentProgressParams, SubAgentProgressPayload};
use agent_client_protocol::{
    self as acp, Content, ContentBlock, ContentChunk, HttpHeader, McpServer, SessionId,
    SessionNotification, SessionUpdate, StopReason, TextContent, ToolCall, ToolCallContent,
    ToolCallId, ToolCallStatus, ToolCallUpdate, ToolCallUpdateFields,
};
use llm::{ToolCallError, ToolCallRequest, ToolCallResult};
use mcp_utils::client::McpServerConfig;
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
        Stdio(stdio) => Some(McpServerConfig::Stdio {
            name: stdio.name,
            command: stdio.command.to_string_lossy().into_owned(),
            args: stdio.args,
            env: stdio.env.into_iter().map(|e| (e.name, e.value)).collect(),
        }),

        Http(http) => Some(McpServerConfig::Http {
            name: http.name,
            config: http_config(http.url, &http.headers),
        }),

        Sse(sse) => Some(McpServerConfig::Http {
            name: sse.name,
            config: http_config(sse.url, &sse.headers),
        }),

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

        AgentMessage::Thought { chunk, .. } => Some(map_thought_to_notification(session_id, chunk)),

        AgentMessage::ToolCall { request, .. } => {
            Some(map_tool_call_to_notification(session_id, request))
        }

        AgentMessage::ToolResult { result, .. } => {
            Some(map_tool_result_to_notification(session_id, result))
        }

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

        AgentMessage::ContextUsageUpdate { .. }
        | AgentMessage::Error { .. }
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
        _ => None,
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
            ToolCall::new(ToolCallId::new(request.id.clone()), request.name.clone())
                .status(acp::ToolCallStatus::InProgress)
                .raw_input(raw_input),
        ),
    )
}

fn map_tool_result_to_notification(
    session_id: SessionId,
    result: &ToolCallResult,
) -> SessionNotification {
    SessionNotification::new(
        session_id,
        SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
            ToolCallId::new(result.id.clone()),
            ToolCallUpdateFields::new()
                .status(ToolCallStatus::Completed)
                .content(vec![ToolCallContent::Content(Content::new(
                    ContentBlock::Text(TextContent::new(result.result.clone())),
                ))]),
        )),
    )
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
        event: payload.event,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether::events::SUB_AGENT_PROGRESS_METHOD;
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
        assert!(matches!(parsed.event, AgentMessage::Text { .. }));
    }

    #[test]
    fn test_thought_maps_to_agent_thought_chunk() {
        let session_id = acp::SessionId::new("test-session");
        let thought = AgentMessage::Thought {
            message_id: "msg_1".to_string(),
            chunk: "thinking...".to_string(),
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
            McpServerConfig::Stdio {
                name,
                command,
                args,
                env,
            } => {
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
            McpServerConfig::Http { name, config } => {
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
            McpServerConfig::Http { name, config } => {
                assert_eq!(name, "sse-server");
                assert_eq!(config.uri.as_ref(), "https://example.com/sse");
                assert_eq!(config.auth_header, None);
            }
            other => panic!("Expected Http, got {:?}", other),
        }
    }
}
