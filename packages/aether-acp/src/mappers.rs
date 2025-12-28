use aether::agent::{AgentMessage, FileAttachment};
use agent_client_protocol as acp;
use rmcp::model::Prompt as McpPrompt;

/// Converts a FileAttachment to an ACP EmbeddedResource ContentBlock.
///
/// This maps the file attachment format used internally by Aether to the
/// ACP protocol format for embedded resources, allowing file contents to
/// be transmitted along with user messages.
pub fn map_file_attachment_to_embedded_resource(file: &FileAttachment) -> acp::ContentBlock {
    acp::ContentBlock::Resource(acp::EmbeddedResource {
        resource: acp::EmbeddedResourceResource::TextResourceContents(
            acp::TextResourceContents {
                uri: format!("file://{}", file.path),
                mime_type: file.mime_type.clone(),
                text: file.content.clone(),
                meta: None,
            },
        ),
        annotations: None,
        meta: None,
    })
}

/// Converts a list of FileAttachments to ACP ContentBlocks.
pub fn map_file_attachments_to_content_blocks(files: &[FileAttachment]) -> Vec<acp::ContentBlock> {
    files.iter().map(map_file_attachment_to_embedded_resource).collect()
}

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
            acp::ContentBlock::Resource(resource) => {
                format_embedded_resource(&resource)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Formats an embedded resource as text for inclusion in agent context.
pub fn format_embedded_resource(resource: &acp::EmbeddedResource) -> String {
    match &resource.resource {
        acp::EmbeddedResourceResource::TextResourceContents(text) => {
            format!(
                "<file uri=\"{}\">\n{}\n</file>",
                text.uri, text.text
            )
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

            let content = message
                .as_ref()
                // Sub agents serialize AgentMessage in content
                .and_then(|msg_str| try_parse_agent_message_content(msg_str))
                // Normal progress notifications
                .unwrap_or_else(|| {
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

                    acp::ContentBlock::Text(acp::TextContent {
                        annotations: None,
                        text: progress_text,
                        meta: None,
                    })
                });

            Some(acp::SessionNotification {
                session_id,
                update: acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate {
                    id: request.id.clone().into(),
                    fields: acp::ToolCallUpdateFields {
                        status: Some(acp::ToolCallStatus::InProgress),
                        content: Some(vec![acp::ToolCallContent::Content { content }]),
                        ..Default::default()
                    },
                    meta: None,
                }),
                meta: None,
            })
        }

        AgentMessage::Error { .. }
        | AgentMessage::Cancelled { .. }
        | AgentMessage::Done
        | AgentMessage::ContextCompactionStarted { .. }
        | AgentMessage::ContextCompactionResult { .. } => None,
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

/// Attempts to parse a serialized AgentMessage and extract its content as an ACP ContentBlock
///
/// This is used primarily for sub-agent messages that are serialized in tool progress notifications.
/// Returns Some(ContentBlock) if the message can be parsed and contains displayable content,
/// None otherwise.
fn try_parse_agent_message_content(message_str: &str) -> Option<acp::ContentBlock> {
    serde_json::from_str::<AgentMessage>(message_str)
        .ok()
        .and_then(|agent_msg| match agent_msg {
            AgentMessage::Text { chunk, .. } => Some(acp::ContentBlock::Text(acp::TextContent {
                annotations: None,
                text: chunk,
                meta: None,
            })),
            AgentMessage::ToolResult { result, .. } => {
                let text = format!("Tool '{}' completed:\n{}", result.name, result.result);
                Some(acp::ContentBlock::Text(acp::TextContent {
                    annotations: None,
                    text,
                    meta: None,
                }))
            }
            AgentMessage::ToolError { error, .. } => {
                let text = format!("Tool '{}' failed:\n{}", error.name, error.error);
                Some(acp::ContentBlock::Text(acp::TextContent {
                    annotations: None,
                    text,
                    meta: None,
                }))
            }
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether::llm::ToolCallRequest;
    use std::sync::Arc;

    #[test]
    fn test_try_parse_agent_message_content_with_text() {
        let serialized = r#"{"Text":{"message_id":"test-id","chunk":"Hello World","is_complete":false,"model_name":"TestModel"}}"#;

        let result = try_parse_agent_message_content(serialized);

        assert!(result.is_some());
        if let Some(acp::ContentBlock::Text(text)) = result {
            assert_eq!(text.text, "Hello World");
        } else {
            panic!("Expected Text content block");
        }
    }

    #[test]
    fn test_try_parse_agent_message_content_with_non_text() {
        let serialized = r#"{"Done":null}"#;

        let result = try_parse_agent_message_content(serialized);

        assert!(result.is_none(), "Non-text messages should return None");
    }

    #[test]
    fn test_try_parse_agent_message_content_with_invalid_json() {
        let invalid = "not valid json at all";

        let result = try_parse_agent_message_content(invalid);

        assert!(result.is_none(), "Invalid JSON should return None");
    }

    #[test]
    fn test_try_parse_agent_message_content_with_tool_result() {
        // Create an actual ToolCallResult and serialize it to see the structure
        use aether::llm::ToolCallResult;
        let tool_result = AgentMessage::ToolResult {
            result: ToolCallResult {
                id: "call_123".to_string(),
                name: "read_file".to_string(),
                arguments: "{}".to_string(),
                result: "File contents here".to_string(),
            },
            model_name: "TestModel".to_string(),
        };

        let serialized = serde_json::to_string(&tool_result).unwrap();
        let result = try_parse_agent_message_content(&serialized);

        assert!(result.is_some(), "Serialized: {}", serialized);
        if let Some(acp::ContentBlock::Text(text)) = result {
            assert!(text.text.contains("Tool 'read_file' completed:"));
            assert!(text.text.contains("File contents here"));
        } else {
            panic!("Expected Text content block");
        }
    }

    #[test]
    fn test_try_parse_agent_message_content_with_tool_error() {
        // Create an actual ToolCallError and serialize it to see the structure
        use aether::llm::ToolCallError;
        let tool_error = AgentMessage::ToolError {
            error: ToolCallError {
                id: "call_456".to_string(),
                name: "write_file".to_string(),
                arguments: Some("{}".to_string()),
                error: "Permission denied".to_string(),
            },
            model_name: "TestModel".to_string(),
        };

        let serialized = serde_json::to_string(&tool_error).unwrap();
        let result = try_parse_agent_message_content(&serialized);

        assert!(result.is_some(), "Serialized: {}", serialized);
        if let Some(acp::ContentBlock::Text(text)) = result {
            assert!(text.text.contains("Tool 'write_file' failed:"));
            assert!(text.text.contains("Permission denied"));
        } else {
            panic!("Expected Text content block");
        }
    }

    #[test]
    fn test_tool_progress_with_serialized_agent_message() {
        let session_id = acp::SessionId(Arc::from("test-session"));

        // Simulate a tool progress message with a serialized Text AgentMessage
        let serialized_msg = r#"{"Text":{"message_id":"75ce3bed-b1cd-469f-9142-7039847f5b00","chunk":"Hello","is_complete":false,"model_name":"LlamaCpp"}}"#;

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

        // Should return a notification
        assert!(notification.is_some());

        let notification = notification.unwrap();

        // Should be a ToolCallUpdate
        match notification.update {
            acp::SessionUpdate::ToolCallUpdate(update) => {
                assert_eq!(update.id, acp::ToolCallId::from("call_123"));
                assert_eq!(update.fields.status, Some(acp::ToolCallStatus::InProgress));

                // Should have content with the deserialized text, not the raw JSON
                if let Some(content) = &update.fields.content {
                    assert!(!content.is_empty());
                    if let acp::ToolCallContent::Content { content: block } = &content[0] {
                        if let acp::ContentBlock::Text(text) = block {
                            // Should contain "Hello" from the chunk, not the raw JSON
                            assert!(
                                text.text.contains("Hello"),
                                "Expected 'Hello' in text, got: {}",
                                text.text
                            );
                            assert!(
                                !text.text.contains("message_id"),
                                "Should not contain raw JSON"
                            );
                        } else {
                            panic!("Expected Text content block");
                        }
                    }
                }
            }
            _ => panic!("Expected ToolCallUpdate"),
        }
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
    fn test_map_file_attachment_to_embedded_resource() {
        use std::path::PathBuf;

        let attachment = FileAttachment {
            path: "src/main.rs".to_string(),
            absolute_path: PathBuf::from("/home/user/project/src/main.rs"),
            content: "fn main() { println!(\"Hello\"); }".to_string(),
            mime_type: Some("text/x-rust".to_string()),
        };

        let result = map_file_attachment_to_embedded_resource(&attachment);

        let acp::ContentBlock::Resource(embedded) = result else {
            panic!("Expected Resource content block");
        };
        let acp::EmbeddedResourceResource::TextResourceContents(text) = embedded.resource else {
            panic!("Expected TextResourceContents");
        };

        assert_eq!(text.uri, "file://src/main.rs");
        assert_eq!(text.text, "fn main() { println!(\"Hello\"); }");
        assert_eq!(text.mime_type, Some("text/x-rust".to_string()));
    }

    #[test]
    fn test_map_file_attachments_to_content_blocks() {
        use std::path::PathBuf;

        let attachments = vec![
            FileAttachment {
                path: "file1.rs".to_string(),
                absolute_path: PathBuf::from("/path/file1.rs"),
                content: "content1".to_string(),
                mime_type: None,
            },
            FileAttachment {
                path: "file2.rs".to_string(),
                absolute_path: PathBuf::from("/path/file2.rs"),
                content: "content2".to_string(),
                mime_type: Some("text/plain".to_string()),
            },
        ];

        let result = map_file_attachments_to_content_blocks(&attachments);

        assert_eq!(result.len(), 2);
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
