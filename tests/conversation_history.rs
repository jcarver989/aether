mod utils;

use crate::utils::*;
use aether::llm::provider::ChatMessage as LlmChatMessage;
use aether::types::ChatMessage;
use chrono::Utc;

// Helper function to create a test app instance (without actually initializing the TUI)
fn create_test_conversation_history() -> Vec<ChatMessage> {
    vec![
        ChatMessage::User {
            content: "Take a screenshot of google.com".to_string(),
            timestamp: Utc::now(),
        },
        ChatMessage::Assistant {
            content: "I'll take a screenshot of google.com for you.".to_string(),
            timestamp: Utc::now(),
        },
        ChatMessage::ToolCall {
            id: TEST_TOOL_ID.to_string(),
            name: "browser_take_screenshot".to_string(),
            params: r#"{"url": "https://google.com"}"#.to_string(),
            timestamp: Utc::now(),
        },
        ChatMessage::ToolResult {
            tool_call_id: TEST_TOOL_ID.to_string(),
            content: r#"{"screenshot_path": "/tmp/google_screenshot.png", "success": true}"#
                .to_string(),
            timestamp: Utc::now(),
        },
    ]
}

fn convert_to_llm_message(message: &ChatMessage) -> Option<LlmChatMessage> {
    match message {
        ChatMessage::System { content, .. } => Some(LlmChatMessage::System {
            content: content.clone(),
        }),
        ChatMessage::User { content, .. } => Some(LlmChatMessage::User {
            content: content.clone(),
        }),
        ChatMessage::Assistant { content, .. } => Some(LlmChatMessage::Assistant {
            content: content.clone(),
            tool_calls: None,
        }),
        ChatMessage::ToolResult {
            tool_call_id,
            content,
            ..
        } => Some(LlmChatMessage::Tool {
            tool_call_id: tool_call_id.clone(),
            content: content.clone(),
        }),
        // Skip these message types in LLM context
        ChatMessage::AssistantStreaming { .. }
        | ChatMessage::Tool { .. }
        | ChatMessage::ToolCall { .. }
        | ChatMessage::Error { .. } => None,
    }
}

#[test]
fn test_conversation_history_includes_tool_results() {
    let conversation_history = create_test_conversation_history();

    // Convert to LLM messages like the App would do
    let llm_messages: Vec<LlmChatMessage> = conversation_history
        .iter()
        .filter_map(convert_to_llm_message)
        .collect();

    // Verify we have the expected messages
    assert_eq!(llm_messages.len(), 3); // User, Assistant, ToolResult (ToolCall is skipped)

    // Check that tool result is included
    let has_tool_result = llm_messages.iter().any(|msg| {
        matches!(msg, LlmChatMessage::Tool { content, .. } if content.contains("screenshot_path"))
    });
    assert!(
        has_tool_result,
        "Tool result should be included in LLM context"
    );

    // Check that user message is preserved
    let has_user_message = llm_messages.iter().any(|msg| {
        matches!(msg, LlmChatMessage::User { content } if content.contains("Take a screenshot"))
    });
    assert!(
        has_user_message,
        "User message should be preserved in LLM context"
    );

    // Check that assistant message is preserved
    let has_assistant_message = llm_messages.iter().any(|msg| {
        matches!(msg, LlmChatMessage::Assistant { content, .. } if content.contains("I'll take a screenshot"))
    });
    assert!(
        has_assistant_message,
        "Assistant message should be preserved in LLM context"
    );
}

#[test]
fn test_raw_tool_call_messages_not_duplicated() {
    let conversation_history = create_test_conversation_history();

    // Convert to LLM messages
    let llm_messages: Vec<LlmChatMessage> = conversation_history
        .iter()
        .filter_map(convert_to_llm_message)
        .collect();

    // Verify raw tool call messages are NOT included as separate Tool messages  
    // (they should be grouped with Assistant messages instead)
    let has_raw_tool_call_message = llm_messages.iter().any(|msg| {
        matches!(msg, LlmChatMessage::Tool { content, .. } if content.contains("browser_take_screenshot"))
    });
    assert!(
        !has_raw_tool_call_message,
        "Raw tool call messages should not appear as separate Tool messages"
    );
}

#[test]
fn test_message_conversion_preserves_content() {
    let test_cases = vec![
        ChatMessage::User {
            content: "Test user message".to_string(),
            timestamp: Utc::now(),
        },
        ChatMessage::Assistant {
            content: "Test assistant response".to_string(),
            timestamp: Utc::now(),
        },
        ChatMessage::ToolResult {
            tool_call_id: "test_call_id".to_string(),
            content: "Test tool result".to_string(),
            timestamp: Utc::now(),
        },
    ];

    for message in test_cases {
        let llm_message = convert_to_llm_message(&message);
        assert!(
            llm_message.is_some(),
            "Message should be converted successfully"
        );

        let content = match &message {
            ChatMessage::User { content, .. } => content,
            ChatMessage::Assistant { content, .. } => content,
            ChatMessage::ToolResult { content, .. } => content,
            _ => panic!("Unexpected message type"),
        };

        // Verify content is preserved
        match llm_message.unwrap() {
            LlmChatMessage::User {
                content: llm_content,
            }
            | LlmChatMessage::Assistant {
                content: llm_content,
                ..
            }
            | LlmChatMessage::Tool {
                content: llm_content,
                ..
            } => {
                assert_eq!(
                    &llm_content, content,
                    "Content should be preserved during conversion"
                );
            }
            _ => panic!("Unexpected LLM message type"),
        }
    }
}

#[test]
fn test_assistant_messages_include_tool_calls() {
    let conversation_history = create_test_conversation_history();
    
    // Convert to LLM messages using the new conversion function
    // This simulates what the App does
    let mut llm_messages = Vec::new();
    let mut i = 0;
    
    while i < conversation_history.len() {
        let message = &conversation_history[i];
        
        match message {
            ChatMessage::User { content, .. } => {
                llm_messages.push(LlmChatMessage::User {
                    content: content.clone(),
                });
            }
            ChatMessage::Assistant { content, .. } => {
                // Look ahead to see if there are tool calls following this assistant message
                let mut tool_calls = Vec::new();
                let mut j = i + 1;
                
                // Collect consecutive tool calls after this assistant message
                while j < conversation_history.len() {
                    if let ChatMessage::ToolCall { id, name, params, .. } = &conversation_history[j] {
                        // Parse params back to JSON
                        if let Ok(arguments) = serde_json::from_str::<serde_json::Value>(params) {
                            tool_calls.push(create_test_tool_call(id, name, arguments));
                        }
                        j += 1;
                    } else {
                        break;
                    }
                }
                
                llm_messages.push(LlmChatMessage::Assistant {
                    content: content.clone(),
                    tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
                });
                
                // Skip the tool calls we already processed
                i = j - 1;
            }
            ChatMessage::ToolResult { tool_call_id, content, .. } => {
                llm_messages.push(LlmChatMessage::Tool {
                    tool_call_id: tool_call_id.clone(),
                    content: content.clone(),
                });
            }
            _ => {} // Skip other message types
        }
        i += 1;
    }
    
    // Verify we have the expected messages
    assert_eq!(llm_messages.len(), 3); // User, Assistant (with tool calls), ToolResult
    
    // Check that assistant message includes tool calls
    let assistant_with_tools = llm_messages.iter().find(|msg| {
        matches!(msg, LlmChatMessage::Assistant { tool_calls: Some(_), .. })
    });
    assert!(assistant_with_tools.is_some(), "Assistant message should include tool calls");
    
    // Verify the tool call details
    if let Some(LlmChatMessage::Assistant { tool_calls: Some(tool_calls), .. }) = assistant_with_tools {
        assert_eq!(tool_calls.len(), 1, "Should have one tool call");
        let tool_call = &tool_calls[0];
        assert_eq!(tool_call.id, TEST_TOOL_ID, "Tool call ID should be preserved");
        assert_eq!(tool_call.name, "browser_take_screenshot", "Tool call name should be preserved");
    }
}

#[test]
fn test_tool_call_id_preservation() {
    let tool_call_id = "call_abc123".to_string();
    let tool_result = ChatMessage::ToolResult {
        tool_call_id: tool_call_id.clone(),
        content: "Tool execution successful".to_string(),
        timestamp: Utc::now(),
    };

    let llm_message = convert_to_llm_message(&tool_result);
    assert!(
        llm_message.is_some(),
        "ToolResult should be converted to LLM message"
    );

    match llm_message.unwrap() {
        LlmChatMessage::Tool {
            tool_call_id: converted_id,
            content,
        } => {
            assert_eq!(
                converted_id, tool_call_id,
                "Tool call ID should be preserved"
            );
            assert_eq!(
                content, "Tool execution successful",
                "Content should be preserved"
            );
        }
        _ => panic!("ToolResult should be converted to LlmChatMessage::Tool"),
    }
}
