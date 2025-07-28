use aether::{
    action::Action,
    components::{chat::Chat, Component, content_block::ContentBlock},
    types::ChatMessage,
};

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
    }).unwrap();
    
    // Second chunk: tool name appears
    chat.update(Action::StreamToolCall {
        id: tool_call_id.to_string(),
        name: "TestTool".to_string(),
        arguments: String::new(),
    }).unwrap();
    
    // Third chunk: arguments start appearing
    chat.update(Action::StreamToolCall {
        id: tool_call_id.to_string(),
        name: "TestTool".to_string(),
        arguments: r#"{"param1": ""#.to_string(),
    }).unwrap();
    
    // Fourth chunk: arguments complete
    chat.update(Action::StreamToolCall {
        id: tool_call_id.to_string(),
        name: "TestTool".to_string(),
        arguments: r#"{"param1": "value1"}"#.to_string(),
    }).unwrap();
    
    // Count how many tool call messages we have with this ID
    let tool_call_count = chat.get_messages().iter()
        .filter(|msg| {
            matches!(msg, ChatMessage::ToolCall { id, .. } if id == tool_call_id)
        })
        .count();
    
    // Should only have ONE tool call message, not multiple
    assert_eq!(tool_call_count, 1, 
        "Expected exactly 1 tool call message, found {}: streaming chunks should update the same message, not create duplicates", 
        tool_call_count
    );
    
    // Verify the final tool call has the correct complete data
    let tool_call_msg = chat.get_messages().iter()
        .find(|msg| matches!(msg, ChatMessage::ToolCall { id, .. } if id == tool_call_id))
        .expect("Tool call message should exist");
    
    if let ChatMessage::ToolCall { name, params, .. } = tool_call_msg {
        assert_eq!(name, "TestTool", "Tool name should be fully updated");
        assert_eq!(params, r#"{"param1": "value1"}"#, "Tool arguments should be fully updated");
    } else {
        panic!("Expected tool call message");
    }
    
    // Verify content blocks also have only one tool call block
    let tool_call_blocks = chat.get_content_blocks().iter()
        .filter(|block| {
            matches!(block, ContentBlock::ToolCallBlock { id, .. } if id == tool_call_id)
        })
        .count();
    
    assert_eq!(tool_call_blocks, 1, 
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
    }).unwrap();
    
    chat.update(Action::StreamToolCall {
        id: "tool_2".to_string(),
        name: "SecondTool".to_string(),
        arguments: r#"{"param": "value2"}"#.to_string(),
    }).unwrap();
    
    // Should have exactly 2 different tool call messages
    let tool_call_count = chat.get_messages().iter()
        .filter(|msg| matches!(msg, ChatMessage::ToolCall { .. }))
        .count();
    
    assert_eq!(tool_call_count, 2, "Should have exactly 2 different tool calls");
    
    // Verify each tool call has correct data
    let tool_1 = chat.get_messages().iter()
        .find(|msg| matches!(msg, ChatMessage::ToolCall { id, .. } if id == "tool_1"))
        .expect("Tool 1 should exist");
    
    let tool_2 = chat.get_messages().iter()
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