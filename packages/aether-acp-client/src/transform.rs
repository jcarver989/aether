//! Event transformation utilities for ACP protocol events.
//!
//! This module provides types and functions for transforming raw ACP protocol
//! events into higher-level events suitable for UI consumption.

use crate::client::{OutputStream, RawAgentEvent};
use agent_client_protocol::{
    AvailableCommand, ContentBlock, RequestPermissionRequest, RequestPermissionResponse,
    SessionNotification, SessionUpdate, ToolCall, ToolCallContent, ToolCallStatus,
    ToolCallUpdateFields,
};
use tokio::sync::oneshot;
use tracing::{debug, info};

/// Protocol-level events from ACP without application-specific routing info.
///
/// These events represent the transformed ACP protocol events. Applications
/// typically wrap these with additional context (like agent IDs) for routing.
#[derive(Debug)]
pub enum AcpEvent {
    /// Append text chunk to the current streaming message.
    MessageChunk { text: String },
    /// Mark current streaming message as complete.
    MessageComplete,
    /// A new tool call started.
    ToolCallStarted {
        tool_id: String,
        tool_call: ToolCall,
    },
    /// Tool call fields updated (but not completed/failed).
    ToolCallUpdated {
        tool_id: String,
        fields: ToolCallUpdateFields,
    },
    /// Tool call completed successfully.
    ToolCallCompleted { tool_id: String, result: String },
    /// Tool call failed.
    ToolCallFailed { tool_id: String, error: String },
    /// Permission request needs user response.
    PermissionRequest {
        request: RequestPermissionRequest,
        response_tx: oneshot::Sender<RequestPermissionResponse>,
    },
    /// Available slash commands updated.
    AvailableCommandsUpdate { commands: Vec<AvailableCommand> },
    /// Terminal output chunk received from a spawned process.
    TerminalOutput {
        terminal_id: String,
        output: String,
        stream: OutputStream,
    },
}

/// Transform a raw ACP event into protocol-level events.
///
/// This handles the conversion from low-level protocol events to
/// higher-level events suitable for UI consumption.
///
/// # Arguments
/// * `raw_event` - The raw event from the ACP client
///
/// # Returns
/// A vector of transformed events (may be empty for ignored events)
pub fn transform_raw_event(raw_event: RawAgentEvent) -> Vec<AcpEvent> {
    match raw_event {
        RawAgentEvent::SessionNotification(notif) => transform_session_notification(notif),
        RawAgentEvent::PermissionRequest {
            request,
            response_tx,
        } => {
            vec![AcpEvent::PermissionRequest {
                request,
                response_tx,
            }]
        }
        RawAgentEvent::TerminalOutput {
            terminal_id,
            output,
            stream,
        } => {
            vec![AcpEvent::TerminalOutput {
                terminal_id,
                output,
                stream,
            }]
        }
    }
}

/// Transform a session notification into protocol-level events.
///
/// # Arguments
/// * `notif` - The session notification from the agent
///
/// # Returns
/// A vector of transformed events (may be empty for ignored notifications)
pub fn transform_session_notification(notif: SessionNotification) -> Vec<AcpEvent> {
    match notif.update {
        SessionUpdate::AgentMessageChunk { content } => {
            if let ContentBlock::Text(text_content) = content {
                vec![AcpEvent::MessageChunk {
                    text: text_content.text,
                }]
            } else {
                vec![]
            }
        }

        SessionUpdate::UserMessageChunk { content } => {
            if let ContentBlock::Text(text_content) = content {
                debug!("User message chunk: {}", text_content.text);
            }
            vec![]
        }

        SessionUpdate::AgentThoughtChunk { content } => {
            if let ContentBlock::Text(text_content) = content {
                debug!("Agent thought: {}", text_content.text);
            }
            vec![]
        }

        SessionUpdate::ToolCall(tc) => {
            let tool_id = tc.id.0.to_string();
            info!("Tool call started: {} - {}", tool_id, tc.title);

            vec![AcpEvent::ToolCallStarted {
                tool_id,
                tool_call: tc,
            }]
        }

        SessionUpdate::ToolCallUpdate(update) => {
            let tool_id = update.id.0.to_string();
            debug!("Tool call update: {} - {:?}", tool_id, update.fields.status);

            if let Some(status) = &update.fields.status {
                match status {
                    ToolCallStatus::Completed => {
                        let content = extract_tool_content(&update.fields)
                            .unwrap_or_else(|| "Completed".to_string());

                        vec![AcpEvent::ToolCallCompleted {
                            tool_id,
                            result: content,
                        }]
                    }
                    ToolCallStatus::Failed => {
                        let error_msg = extract_tool_content(&update.fields)
                            .unwrap_or_else(|| "Unknown error".to_string());

                        vec![AcpEvent::ToolCallFailed {
                            tool_id,
                            error: error_msg,
                        }]
                    }
                    _ => {
                        vec![AcpEvent::ToolCallUpdated {
                            tool_id,
                            fields: update.fields,
                        }]
                    }
                }
            } else {
                vec![AcpEvent::ToolCallUpdated {
                    tool_id,
                    fields: update.fields,
                }]
            }
        }

        SessionUpdate::Plan(plan) => {
            debug!("Received plan: {:?}", plan);
            vec![]
        }

        SessionUpdate::AvailableCommandsUpdate { available_commands } => {
            debug!("Available commands updated: {:?}", available_commands);
            vec![AcpEvent::AvailableCommandsUpdate {
                commands: available_commands,
            }]
        }

        SessionUpdate::CurrentModeUpdate { current_mode_id } => {
            debug!("Mode changed to: {}", current_mode_id);
            vec![]
        }
    }
}

/// Extract text content from tool call update fields.
///
/// Searches through the content blocks for a text block and returns its text.
pub fn extract_tool_content(fields: &ToolCallUpdateFields) -> Option<String> {
    fields.content.as_ref().and_then(|contents| {
        contents.iter().find_map(|c| match c {
            ToolCallContent::Content { content } => {
                if let ContentBlock::Text(t) = content {
                    Some(t.text.clone())
                } else {
                    None
                }
            }
            _ => None,
        })
    })
}
