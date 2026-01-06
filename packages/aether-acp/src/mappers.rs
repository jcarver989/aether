use aether::agent::AgentMessage;
use agent_events::{ContextUsageParams, SubAgentProgressParams, SubAgentProgressPayload};
use agent_client_protocol as acp;
use rmcp::model::Prompt as McpPrompt;

/// Converts an MCP Prompt to an ACP AvailableCommand
///
/// Strips the MCP namespace from the prompt name (e.g., "mcp-lexicon__web" -> "web")
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
        if !args.is_empty() {
            // Create a hint from the argument names
            let hint = args
                .iter()
                .map(|arg| arg.name.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            Some(acp::AvailableCommandInput::Unstructured { hint })
        } else {
            // Even if no formal arguments, provide a generic hint
            Some(acp::AvailableCommandInput::Unstructured {
                hint: "optional arguments".to_string(),
            })
        }
    } else {
        // No arguments defined, provide a generic hint for optional input
        Some(acp::AvailableCommandInput::Unstructured {
            hint: "optional arguments".to_string(),
        })
    };

    acp::AvailableCommand {
        name: command_name,
        description: prompt
            .description
            .as_ref()
            .map(|d| d.to_string())
            .unwrap_or_else(|| "No description available".to_string()),
        input,
        meta: None,
    }
}

/// Converts ACP ContentBlock to plain text for Aether agent.
///
/// Embedded resources (e.g., file attachments) are formatted with their URI
/// and content for inclusion in the agent's context.
pub fn map_content_blocks_to_text(blocks: Vec<acp::ContentBlock>) -> String {
    blocks
        .into_iter()
        .map(|block| match block {
            acp::ContentBlock::Text(text) => text.text.to_string(),
            acp::ContentBlock::Image(_) => "[Image content]".to_string(),
            acp::ContentBlock::Audio(_) => "[Audio content]".to_string(),
            acp::ContentBlock::ResourceLink(link) => {
                format!("[Resource: {}]", link.uri)
            }
            acp::ContentBlock::Resource(resource) => format_embedded_resource(&resource),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Formats an embedded resource as text for inclusion in agent context.
pub fn format_embedded_resource(resource: &acp::EmbeddedResource) -> String {
    match &resource.resource {
        acp::EmbeddedResourceResource::TextResourceContents(text) => {
            format!("<file uri=\"{}\">\n{}\n</file>", text.uri, text.text)
        }
        acp::EmbeddedResourceResource::BlobResourceContents(blob) => {
            format!("[Binary resource: {}]", blob.uri)
        }
    }
}

/// Converts Aether AgentMessage to ACP SessionUpdate
pub fn map_agent_message_to_session_notification(
    session_id: acp::SessionId,
    msg: &AgentMessage,
) -> Option<acp::SessionNotification> {
    match msg {
        AgentMessage::Text {
            message_id: _,
            chunk,
            is_complete,
            model_name: _,
        } => {
            // Skip the final completion message to avoid sending duplicate content
            // The client has already received all the chunks during streaming
            if *is_complete {
                return None;
            }

            Some(acp::SessionNotification {
                session_id,
                update: acp::SessionUpdate::AgentMessageChunk {
                    content: acp::ContentBlock::Text(acp::TextContent {
                        annotations: None,
                        text: chunk.clone(),
                        meta: None,
                    }),
                },
                meta: None,
            })
        }

        AgentMessage::ToolCall { request, .. } => {
            let raw_input = serde_json::from_str(&request.arguments).ok();
            Some(acp::SessionNotification {
                session_id,
                update: acp::SessionUpdate::ToolCall(acp::ToolCall {
                    id: request.id.clone().into(),
                    title: request.name.clone(),
                    kind: acp::ToolKind::default(),
                    status: acp::ToolCallStatus::InProgress,
                    content: vec![],
                    locations: vec![],
                    raw_input,
                    raw_output: None,
                    meta: None,
                }),
                meta: None,
            })
        }

        AgentMessage::ToolResult { result, .. } => Some(acp::SessionNotification {
            session_id,
            update: acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate {
                id: result.id.clone().into(),
                fields: acp::ToolCallUpdateFields {
                    status: Some(acp::ToolCallStatus::Completed),
                    content: Some(vec![acp::ToolCallContent::Content {
                        content: acp::ContentBlock::Text(acp::TextContent {
                            annotations: None,
                            text: result.result.clone(),
                            meta: None,
                        }),
                    }]),
                    ..Default::default()
                },
                meta: None,
            }),
            meta: None,
        }),

        AgentMessage::ToolError { error, .. } => Some(acp::SessionNotification {
            session_id,
            update: acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate {
                id: error.id.clone().into(),
                fields: acp::ToolCallUpdateFields {
                    status: Some(acp::ToolCallStatus::Failed),
                    content: Some(vec![acp::ToolCallContent::Content {
                        content: acp::ContentBlock::Text(acp::TextContent {
                            annotations: None,
                            text: error.error.clone(),
                            meta: None,
                        }),
                    }]),
                    ..Default::default()
                },
                meta: None,
            }),
            meta: None,
        }),

        AgentMessage::ToolProgress {
            request,
            progress,
            total,
            message,
        } => {
            tracing::info!("Tool progress: {message:?}");

            if message
                .as_ref()
                .and_then(|msg_str| try_parse_sub_agent_progress(msg_str, request))
                .is_some()
            {
                return None;
            }

            let progress_text = message
                .as_ref()
                .map(|msg| {
                    format!(
                        "{} ({}/{})",
                        msg,
                        progress,
                        total
                            .map(|t| t.to_string())
                            .unwrap_or_else(|| "?".to_string())
                    )
                })
                .unwrap_or_else(|| {
                    format!(
                        "Progress: {}/{}",
                        progress,
                        total
                            .map(|t| t.to_string())
                            .unwrap_or_else(|| "?".to_string())
                    )
                });

            Some(acp::SessionNotification {
                session_id,
                update: acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate {
                    id: request.id.clone().into(),
                    fields: acp::ToolCallUpdateFields {
                        status: Some(acp::ToolCallStatus::InProgress),
                        content: Some(vec![acp::ToolCallContent::Content {
                            content: acp::ContentBlock::Text(acp::TextContent {
                                annotations: None,
                                text: progress_text,
                                meta: None,
                            }),
                        }]),
                        ..Default::default()
                    },
                    meta: None,
                }),
                meta: None,
            })
        }

        AgentMessage::ContextUsageUpdate { .. }
        | AgentMessage::Error { .. }
        | AgentMessage::Cancelled { .. }
        | AgentMessage::Done
        | AgentMessage::ContextCompactionStarted { .. }
        | AgentMessage::ContextCompactionResult { .. }
        | AgentMessage::AutoContinue { .. } => None,
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
        AgentMessage::ToolProgress { request, message, .. } => {
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
        AgentMessage::Done => acp::StopReason::EndTurn,
        AgentMessage::Cancelled { .. } => acp::StopReason::Cancelled,
        AgentMessage::Error { .. } => acp::StopReason::EndTurn, // Map error to EndTurn
        _ => acp::StopReason::EndTurn,
    }
}

/// Attempt to parse a tool progress message as sub-agent progress.
fn try_parse_sub_agent_progress(
    message: &str,
    request: &aether::llm::ToolCallRequest,
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
    use aether::llm::ToolCallRequest;
    use agent_events::SUB_AGENT_PROGRESS_METHOD;
    use std::sync::Arc;

    #[test]
    fn test_tool_progress_with_sub_agent_payload_emits_ext_notification() {
        let session_id = acp::SessionId(Arc::from("test-session"));

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
    fn test_tool_progress_with_invalid_json_falls_back_to_simple_message() {
        let session_id = acp::SessionId(Arc::from("test-session"));

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
                    if let acp::ToolCallContent::Content { content: block } = &content[0] {
                        if let acp::ContentBlock::Text(text) = block {
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
    fn test_format_embedded_resource_text() {
        let resource = acp::EmbeddedResource {
            resource: acp::EmbeddedResourceResource::TextResourceContents(
                acp::TextResourceContents {
                    uri: "file://test.rs".to_string(),
                    mime_type: None,
                    text: "let x = 1;".to_string(),
                    meta: None,
                },
            ),
            annotations: None,
            meta: None,
        };

        let result = format_embedded_resource(&resource);

        assert_eq!(result, "<file uri=\"file://test.rs\">\nlet x = 1;\n</file>");
    }

    #[test]
    fn test_map_content_blocks_to_text_with_embedded_resource() {
        let blocks = vec![
            acp::ContentBlock::Text(acp::TextContent {
                text: "Check this file:".to_string(),
                annotations: None,
                meta: None,
            }),
            acp::ContentBlock::Resource(acp::EmbeddedResource {
                resource: acp::EmbeddedResourceResource::TextResourceContents(
                    acp::TextResourceContents {
                        uri: "file://src/lib.rs".to_string(),
                        mime_type: Some("text/x-rust".to_string()),
                        text: "pub fn hello() {}".to_string(),
                        meta: None,
                    },
                ),
                annotations: None,
                meta: None,
            }),
        ];

        let result = map_content_blocks_to_text(blocks);

        assert!(result.contains("Check this file:"));
        assert!(result.contains("<file uri=\"file://src/lib.rs\">"));
        assert!(result.contains("pub fn hello() {}"));
        assert!(result.contains("</file>"));
    }
}
