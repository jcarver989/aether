use aether::agent::AgentMessage;
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
                .map(|arg| arg.name.as_ref())
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

/// Converts ACP ContentBlock to plain text for Aether agent
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
            acp::ContentBlock::Resource(_resource) => "[Embedded resource]".to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
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
            is_complete: _,
            model_name: _,
        } => Some(acp::SessionNotification {
            session_id,
            update: acp::SessionUpdate::AgentMessageChunk {
                content: acp::ContentBlock::Text(acp::TextContent {
                    annotations: None,
                    text: chunk.clone(),
                    meta: None,
                }),
            },
            meta: None,
        }),

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
            let progress_text = if let Some(msg) = message {
                format!(
                    "{} ({}/{})",
                    msg,
                    progress,
                    total
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "?".to_string())
                )
            } else {
                format!(
                    "Progress: {}/{}",
                    progress,
                    total
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "?".to_string())
                )
            };

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

        AgentMessage::Error { .. } | AgentMessage::Cancelled { .. } | AgentMessage::Done => {
            // These are terminal events that affect the prompt response, not session updates
            None
        }
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
