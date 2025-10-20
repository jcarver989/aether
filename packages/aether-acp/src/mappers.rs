use aether::agent::AgentMessage;
use agent_client_protocol as acp;

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
                    text: chunk.clone().into(),
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
                            text: result.result.clone().into(),
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
                            text: error.error.clone().into(),
                            meta: None,
                        }),
                    }]),
                    ..Default::default()
                },
                meta: None,
            }),
            meta: None,
        }),

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
