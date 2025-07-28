use aether::{
    action::{Action, ScrollDirection},
    components::{Component, chat::Chat, content_block::ContentBlock},
    types::ChatMessage,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[tokio::test]
async fn test_streaming_tool_call_no_duplicates() {
    let mut chat = Chat::new();

    // Register a dummy action handler
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    chat.register_action_handler(tx).unwrap();

    // Simulate streaming tool call chunks - this would have created duplicates before the fix
    let tool_call_id = "test_tool_123";

    // First chunk: empty name and arguments (typical during streaming)
    chat.update(Action::StreamToolCall {
        id: tool_call_id.to_string(),
        name: String::new(),
        arguments: String::new(),
    })
    .unwrap();

    // Second chunk: tool name appears
    chat.update(Action::StreamToolCall {
        id: tool_call_id.to_string(),
        name: "TestTool".to_string(),
        arguments: String::new(),
    })
    .unwrap();

    // Third chunk: arguments start appearing
    chat.update(Action::StreamToolCall {
        id: tool_call_id.to_string(),
        name: "TestTool".to_string(),
        arguments: r#"{"param1": ""#.to_string(),
    })
    .unwrap();

    // Fourth chunk: arguments complete
    chat.update(Action::StreamToolCall {
        id: tool_call_id.to_string(),
        name: "TestTool".to_string(),
        arguments: r#"{"param1": "value1"}"#.to_string(),
    })
    .unwrap();

    // Count how many tool call messages we have with this ID
    let tool_call_count = chat
        .get_messages()
        .iter()
        .filter(|msg| matches!(msg, ChatMessage::ToolCall { id, .. } if id == tool_call_id))
        .count();

    // Should only have ONE tool call message, not multiple
    assert_eq!(
        tool_call_count, 1,
        "Expected exactly 1 tool call message, found {}: streaming chunks should update the same message, not create duplicates",
        tool_call_count
    );

    // Verify the final tool call has the correct complete data
    let tool_call_msg = chat
        .get_messages()
        .iter()
        .find(|msg| matches!(msg, ChatMessage::ToolCall { id, .. } if id == tool_call_id))
        .expect("Tool call message should exist");

    if let ChatMessage::ToolCall { name, params, .. } = tool_call_msg {
        assert_eq!(name, "TestTool", "Tool name should be fully updated");
        assert_eq!(
            params, r#"{"param1": "value1"}"#,
            "Tool arguments should be fully updated"
        );
    } else {
        panic!("Expected tool call message");
    }

    // Verify content blocks also have only one tool call block
    let tool_call_blocks = chat
        .get_content_blocks()
        .iter()
        .filter(
            |block| matches!(block, ContentBlock::ToolCallBlock { id, .. } if id == tool_call_id),
        )
        .count();

    assert_eq!(
        tool_call_blocks, 1,
        "Expected exactly 1 tool call content block, found {}",
        tool_call_blocks
    );
}

#[tokio::test]
async fn test_multiple_different_tool_calls() {
    let mut chat = Chat::new();

    // Register a dummy action handler
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    chat.register_action_handler(tx).unwrap();

    // Simulate two different streaming tool calls
    chat.update(Action::StreamToolCall {
        id: "tool_1".to_string(),
        name: "FirstTool".to_string(),
        arguments: r#"{"param": "value1"}"#.to_string(),
    })
    .unwrap();

    chat.update(Action::StreamToolCall {
        id: "tool_2".to_string(),
        name: "SecondTool".to_string(),
        arguments: r#"{"param": "value2"}"#.to_string(),
    })
    .unwrap();

    // Should have exactly 2 different tool call messages
    let tool_call_count = chat
        .get_messages()
        .iter()
        .filter(|msg| matches!(msg, ChatMessage::ToolCall { .. }))
        .count();

    assert_eq!(
        tool_call_count, 2,
        "Should have exactly 2 different tool calls"
    );

    // Verify each tool call has correct data
    let tool_1 = chat
        .get_messages()
        .iter()
        .find(|msg| matches!(msg, ChatMessage::ToolCall { id, .. } if id == "tool_1"))
        .expect("Tool 1 should exist");

    let tool_2 = chat
        .get_messages()
        .iter()
        .find(|msg| matches!(msg, ChatMessage::ToolCall { id, .. } if id == "tool_2"))
        .expect("Tool 2 should exist");

    if let ChatMessage::ToolCall { name, params, .. } = tool_1 {
        assert_eq!(name, "FirstTool");
        assert_eq!(params, r#"{"param": "value1"}"#);
    }

    if let ChatMessage::ToolCall { name, params, .. } = tool_2 {
        assert_eq!(name, "SecondTool");
        assert_eq!(params, r#"{"param": "value2"}"#);
    }
}

#[tokio::test]
async fn test_arrow_key_scrolling() {
    let mut chat = Chat::new();

    // Register a dummy action handler
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    chat.register_action_handler(tx).unwrap();

    // Test regular Up arrow key
    let up_key = KeyEvent {
        code: KeyCode::Up,
        modifiers: KeyModifiers::NONE,
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    };

    let result = chat.handle_key_event(up_key).unwrap();
    assert_eq!(result, Some(Action::ScrollChat(ScrollDirection::Up)));

    // Test regular Down arrow key
    let down_key = KeyEvent {
        code: KeyCode::Down,
        modifiers: KeyModifiers::NONE,
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    };

    let result = chat.handle_key_event(down_key).unwrap();
    assert_eq!(result, Some(Action::ScrollChat(ScrollDirection::Down)));

    // Test Ctrl+Up still works
    let ctrl_up_key = KeyEvent {
        code: KeyCode::Up,
        modifiers: KeyModifiers::CONTROL,
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    };

    let result = chat.handle_key_event(ctrl_up_key).unwrap();
    assert_eq!(result, Some(Action::ScrollChat(ScrollDirection::Up)));

    // Test Ctrl+Down still works
    let ctrl_down_key = KeyEvent {
        code: KeyCode::Down,
        modifiers: KeyModifiers::CONTROL,
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    };

    let result = chat.handle_key_event(ctrl_down_key).unwrap();
    assert_eq!(result, Some(Action::ScrollChat(ScrollDirection::Down)));
}

#[tokio::test]
async fn test_streaming_message_content_updates_in_real_time() {
    let mut chat = Chat::new();

    // Register a dummy action handler
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    chat.register_action_handler(tx).unwrap();

    // Start streaming - this creates an initial empty AssistantStreaming message
    chat.update(Action::StartStreaming).unwrap();

    // Verify initial state: should have 1 message and 1 content block
    assert_eq!(chat.get_messages().len(), 1, "Should have 1 streaming message");
    assert_eq!(chat.get_content_blocks().len(), 1, "Should have 1 content block");
    
    // Check initial content block
    let initial_block = &chat.get_content_blocks()[0];
    if let ContentBlock::AssistantMessage { display_text, streaming, .. } = initial_block {
        assert_eq!(display_text, "", "Initial streaming content should be empty");
        assert!(*streaming, "Block should be marked as streaming");
    } else {
        panic!("Expected AssistantMessage content block, got {:?}", initial_block);
    }

    // Stream first chunk of content
    chat.update(Action::StreamContent("Hello".to_string())).unwrap();

    // Verify content updated without creating new messages/blocks
    assert_eq!(chat.get_messages().len(), 1, "Should still have exactly 1 message");
    assert_eq!(chat.get_content_blocks().len(), 1, "Should still have exactly 1 content block");
    
    let first_chunk_block = &chat.get_content_blocks()[0];
    if let ContentBlock::AssistantMessage { display_text, streaming, .. } = first_chunk_block {
        assert_eq!(display_text, "Hello", "Content should be updated to 'Hello'");
        assert!(*streaming, "Block should still be marked as streaming");
    } else {
        panic!("Expected AssistantMessage content block after first chunk");
    }

    // Stream second chunk - should append to existing content
    chat.update(Action::StreamContent(" world!".to_string())).unwrap();

    // Verify content appended correctly
    assert_eq!(chat.get_messages().len(), 1, "Should still have exactly 1 message");
    assert_eq!(chat.get_content_blocks().len(), 1, "Should still have exactly 1 content block");
    
    let second_chunk_block = &chat.get_content_blocks()[0];
    if let ContentBlock::AssistantMessage { display_text, streaming, .. } = second_chunk_block {
        assert_eq!(display_text, "Hello world!", "Content should be updated to 'Hello world!'");
        assert!(*streaming, "Block should still be marked as streaming");
    } else {
        panic!("Expected AssistantMessage content block after second chunk");
    }

    // Stream third chunk with more content
    chat.update(Action::StreamContent("\n\nThis is a multi-line response.".to_string())).unwrap();

    let third_chunk_block = &chat.get_content_blocks()[0];
    if let ContentBlock::AssistantMessage { display_text, streaming, .. } = third_chunk_block {
        assert_eq!(
            display_text, 
            "Hello world!\n\nThis is a multi-line response.",
            "Content should include all streamed chunks"
        );
        assert!(*streaming, "Block should still be marked as streaming");
    } else {
        panic!("Expected AssistantMessage content block after third chunk");
    }

    // Complete the stream - should convert to final message
    chat.update(Action::StreamComplete).unwrap();

    // Verify final state
    assert_eq!(chat.get_messages().len(), 1, "Should still have exactly 1 message");
    assert_eq!(chat.get_content_blocks().len(), 1, "Should still have exactly 1 content block");
    
    let final_block = &chat.get_content_blocks()[0];
    if let ContentBlock::AssistantMessage { display_text, streaming, .. } = final_block {
        assert_eq!(
            display_text, 
            "Hello world!\n\nThis is a multi-line response.",
            "Final content should match streamed content"
        );
        assert!(!*streaming, "Block should no longer be marked as streaming");
    } else {
        panic!("Expected AssistantMessage content block after stream completion");
    }

    // Verify the underlying message was also converted correctly
    if let Some(ChatMessage::Assistant { content, .. }) = chat.get_messages().last() {
        assert_eq!(
            content, 
            "Hello world!\n\nThis is a multi-line response.",
            "Final message content should match streamed content"
        );
    } else {
        panic!("Expected final message to be Assistant type, not streaming");
    }
}

#[tokio::test]
async fn test_streaming_with_multiple_messages_no_interference() {
    let mut chat = Chat::new();

    // Register a dummy action handler
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    chat.register_action_handler(tx).unwrap();

    // Add a regular user message first
    chat.update(Action::AddChatMessage(ChatMessage::User {
        content: "What is 2+2?".to_string(),
        timestamp: chrono::Utc::now(),
    })).unwrap();

    // Start streaming response
    chat.update(Action::StartStreaming).unwrap();
    
    // Should now have 2 messages and 2 content blocks
    assert_eq!(chat.get_messages().len(), 2, "Should have user message + streaming message");
    assert_eq!(chat.get_content_blocks().len(), 2, "Should have 2 content blocks");

    // Verify the user message wasn't affected
    let user_block = &chat.get_content_blocks()[0];
    if let ContentBlock::UserMessage { content, .. } = user_block {
        assert_eq!(content, "What is 2+2?", "User message should be unchanged");
    } else {
        panic!("First block should be user message");
    }

    // Stream content to the assistant message
    chat.update(Action::StreamContent("The answer is ".to_string())).unwrap();
    chat.update(Action::StreamContent("4".to_string())).unwrap();

    // Verify user message still unchanged, assistant message updated
    assert_eq!(chat.get_messages().len(), 2, "Should still have 2 messages");
    assert_eq!(chat.get_content_blocks().len(), 2, "Should still have 2 content blocks");

    let user_block_after = &chat.get_content_blocks()[0];
    if let ContentBlock::UserMessage { content, .. } = user_block_after {
        assert_eq!(content, "What is 2+2?", "User message should still be unchanged");
    } else {
        panic!("First block should still be user message");
    }

    let assistant_block = &chat.get_content_blocks()[1];
    if let ContentBlock::AssistantMessage { display_text, streaming, .. } = assistant_block {
        assert_eq!(display_text, "The answer is 4", "Assistant message should have streamed content");
        assert!(*streaming, "Assistant message should be marked as streaming");
    } else {
        panic!("Second block should be streaming assistant message");
    }

    // Complete streaming
    chat.update(Action::StreamComplete).unwrap();

    // Final verification
    assert_eq!(chat.get_messages().len(), 2, "Should still have 2 messages");
    assert_eq!(chat.get_content_blocks().len(), 2, "Should still have 2 content blocks");

    let final_assistant_block = &chat.get_content_blocks()[1];
    if let ContentBlock::AssistantMessage { display_text, streaming, .. } = final_assistant_block {
        assert_eq!(display_text, "The answer is 4", "Final assistant content should match");
        assert!(!*streaming, "Assistant message should no longer be streaming");
    } else {
        panic!("Second block should be completed assistant message");
    }
}
