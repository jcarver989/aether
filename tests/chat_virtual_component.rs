use aether::{
    action::{Action, ScrollDirection},
    components::{Component, chat_virtual::ChatVirtual},
    types::ChatMessage,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::TestBackend, Terminal};
use tokio::sync::mpsc;
use color_eyre::Result;

#[tokio::test]
async fn test_chat_virtual_creation() -> Result<()> {
    let _chat = ChatVirtual::new();
    let _chat_default = ChatVirtual::default();
    
    // Both should create instances without panicking
    Ok(())
}

#[tokio::test]
async fn test_component_trait_registration() -> Result<()> {
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut chat = ChatVirtual::new();
    
    // Should register handlers without errors
    chat.register_action_handler(tx)?;
    chat.register_config_handler(std::sync::Arc::new(aether::config::Config::default()))?;
    
    Ok(())
}

#[tokio::test]
async fn test_add_and_clear_messages() -> Result<()> {
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut chat = ChatVirtual::new();
    chat.register_action_handler(tx)?;
    
    // Add messages
    chat.update(Action::AddChatMessage(ChatMessage::User {
        content: "Hello".to_string(),
        timestamp: chrono::Utc::now(),
    }))?;
    
    chat.update(Action::AddChatMessage(ChatMessage::Assistant {
        content: "Hi there!".to_string(),
        timestamp: chrono::Utc::now(),
    }))?;
    
    chat.update(Action::AddChatMessage(ChatMessage::System {
        content: "System message".to_string(),
        timestamp: chrono::Utc::now(),
    }))?;
    
    // Clear the chat
    chat.update(Action::ClearChat)?;
    
    // Should not panic after clear
    Ok(())
}

#[tokio::test]
async fn test_streaming_message_flow() -> Result<()> {
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut chat = ChatVirtual::new();
    chat.register_action_handler(tx)?;
    
    // Test streaming lifecycle
    chat.update(Action::StartStreaming)?;
    chat.update(Action::StreamContent("Hello".to_string()))?;
    chat.update(Action::StreamContent(" world".to_string()))?;
    chat.update(Action::StreamComplete)?;
    
    // Should handle streaming without panicking
    Ok(())
}

#[tokio::test]
async fn test_tool_call_handling() -> Result<()> {
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut chat = ChatVirtual::new();
    chat.register_action_handler(tx)?;
    
    // Add tool calls
    chat.update(Action::StreamToolCall {
        id: "tool1".to_string(),
        name: "search".to_string(),
        arguments: "{\"query\": \"test\"}".to_string(),
    })?;
    
    // Update the same tool call
    chat.update(Action::StreamToolCall {
        id: "tool1".to_string(),
        name: "search".to_string(),
        arguments: "{\"query\": \"updated test\"}".to_string(),
    })?;
    
    // Add different tool call
    chat.update(Action::StreamToolCall {
        id: "tool2".to_string(),
        name: "calculate".to_string(),
        arguments: "{\"expression\": \"2+2\"}".to_string(),
    })?;
    
    // Should handle tool calls without errors
    Ok(())
}

#[tokio::test]
async fn test_error_handling() -> Result<()> {
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut chat = ChatVirtual::new();
    chat.register_action_handler(tx)?;
    
    // Add errors
    chat.update(Action::Error("Something went wrong".to_string()))?;
    chat.update(Action::Error("Another error".to_string()))?;
    
    // Should handle errors without panicking
    Ok(())
}

#[tokio::test]
async fn test_key_event_handling() -> Result<()> {
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut chat = ChatVirtual::new();
    chat.register_action_handler(tx)?;
    
    // Test various key events
    let up_action = chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE))?;
    assert_eq!(up_action, Some(Action::ScrollChat(ScrollDirection::Up)));
    
    let down_action = chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))?;
    assert_eq!(down_action, Some(Action::ScrollChat(ScrollDirection::Down)));
    
    let page_up_action = chat.handle_key_event(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE))?;
    assert_eq!(page_up_action, Some(Action::ScrollChat(ScrollDirection::PageUp)));
    
    let page_down_action = chat.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE))?;
    assert_eq!(page_down_action, Some(Action::ScrollChat(ScrollDirection::PageDown)));
    
    // Test unrelated key
    let enter_action = chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    assert_eq!(enter_action, None);
    
    Ok(())
}

#[tokio::test]
async fn test_scroll_action_handling() -> Result<()> {
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut chat = ChatVirtual::new();
    chat.register_action_handler(tx)?;
    
    // Add many messages to enable scrolling
    for i in 0..20 {
        chat.update(Action::AddChatMessage(ChatMessage::User {
            content: format!("Message {}", i),
            timestamp: chrono::Utc::now(),
        }))?;
    }
    
    // Test scroll actions
    let result = chat.update(Action::ScrollChat(ScrollDirection::Up))?;
    assert_eq!(result, None); // Actions are handled internally
    
    let result = chat.update(Action::ScrollChat(ScrollDirection::Down))?;
    assert_eq!(result, None);
    
    let result = chat.update(Action::ScrollChat(ScrollDirection::PageUp))?;
    assert_eq!(result, None);
    
    let result = chat.update(Action::ScrollChat(ScrollDirection::PageDown))?;
    assert_eq!(result, None);
    
    Ok(())
}

#[tokio::test]
async fn test_rendering_with_various_message_types() -> Result<()> {
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut chat = ChatVirtual::new();
    chat.register_action_handler(tx)?;
    
    // Add different types of messages
    let messages = vec![
        ChatMessage::System {
            content: "System message".to_string(),
            timestamp: chrono::Utc::now(),
        },
        ChatMessage::User {
            content: "User message\nwith multiple lines\nand more text".to_string(),
            timestamp: chrono::Utc::now(),
        },
        ChatMessage::Assistant {
            content: "Assistant response with some longer text".to_string(),
            timestamp: chrono::Utc::now(),
        },
        ChatMessage::AssistantStreaming {
            content: "Currently streaming...".to_string(),
            timestamp: chrono::Utc::now(),
        },
        ChatMessage::ToolCall {
            id: "tool1".to_string(),
            name: "search".to_string(),
            params: "{\"query\": \"test\"}".to_string(),
            timestamp: chrono::Utc::now(),
        },
        ChatMessage::ToolResult {
            tool_call_id: "tool1".to_string(),
            content: "Search results\nwith multiple lines\nof output".to_string(),
            timestamp: chrono::Utc::now(),
        },
        ChatMessage::Error {
            message: "An error occurred\nwith details on multiple lines".to_string(),
            timestamp: chrono::Utc::now(),
        },
    ];
    
    for message in messages {
        chat.update(Action::AddChatMessage(message))?;
    }
    
    // Test rendering with a mock terminal - should not panic
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend)?;
    
    terminal.draw(|frame| {
        chat.draw(frame, frame.area()).unwrap();
    })?;
    
    // Test rendering with small terminal
    let small_backend = TestBackend::new(40, 10);
    let mut small_terminal = Terminal::new(small_backend)?;
    
    small_terminal.draw(|frame| {
        chat.draw(frame, frame.area()).unwrap();
    })?;
    
    Ok(())
}

#[tokio::test]
async fn test_content_block_item_height_calculation() -> Result<()> {
    use aether::components::chat_virtual::ContentBlockItem;
    use aether::components::virtual_scroll::VirtualScrollItem;
    use aether::components::content_block::ContentBlock;
    
    // Test height calculation for different content types
    let system_block = ContentBlockItem {
        block: ContentBlock::SystemMessage {
            content: "Single line".to_string(),
            timestamp: chrono::Utc::now(),
        },
        selected: false,
    };
    
    let height = system_block.height(80);
    assert!(height >= 3); // At least title + content + separator
    
    let multiline_block = ContentBlockItem {
        block: ContentBlock::UserMessage {
            content: "Line 1\nLine 2\nLine 3".to_string(),
            timestamp: chrono::Utc::now(),
        },
        selected: false,
    };
    
    let multiline_height = multiline_block.height(80);
    assert!(multiline_height > height); // Should be taller than single line
    
    Ok(())
}

#[tokio::test]
async fn test_content_block_item_rendering() -> Result<()> {
    use aether::components::chat_virtual::ContentBlockItem;
    use aether::components::virtual_scroll::VirtualScrollItem;
    use aether::components::content_block::ContentBlock;
    use ratatui::layout::Rect;
    
    let block_item = ContentBlockItem {
        block: ContentBlock::UserMessage {
            content: "Test message".to_string(),
            timestamp: chrono::Utc::now(),
        },
        selected: false,
    };
    
    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend)?;
    
    // Should render without panicking
    terminal.draw(|frame| {
        let area = Rect::new(0, 0, 80, 5);
        block_item.render(area, frame.buffer_mut());
    })?;
    
    // Test selected state
    let selected_block = ContentBlockItem {
        block: ContentBlock::AssistantMessage {
            display_text: "Selected message".to_string(),
            streaming: false,
            content: vec![],
            timestamp: chrono::Utc::now(),
        },
        selected: true,
    };
    
    terminal.draw(|frame| {
        let area = Rect::new(0, 0, 80, 5);
        selected_block.render(area, frame.buffer_mut());
    })?;
    
    Ok(())
}

#[tokio::test]
async fn test_mixed_content_workflow() -> Result<()> {
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut chat = ChatVirtual::new();
    chat.register_action_handler(tx)?;
    
    // Simulate a realistic conversation workflow
    
    // User asks a question
    chat.update(Action::AddChatMessage(ChatMessage::User {
        content: "Can you search for information about Rust?".to_string(),
        timestamp: chrono::Utc::now(),
    }))?;
    
    // Assistant starts responding
    chat.update(Action::StartStreaming)?;
    chat.update(Action::StreamContent("I'll search for information about Rust for you.".to_string()))?;
    chat.update(Action::StreamComplete)?;
    
    // Tool call is made
    chat.update(Action::StreamToolCall {
        id: "search_1".to_string(),
        name: "web_search".to_string(),
        arguments: "{\"query\": \"Rust programming language\"}".to_string(),
    })?;
    
    // Tool result comes back
    chat.update(Action::AddChatMessage(ChatMessage::ToolResult {
        tool_call_id: "search_1".to_string(),
        content: "Rust is a systems programming language...".to_string(),
        timestamp: chrono::Utc::now(),
    }))?;
    
    // Assistant provides final response
    chat.update(Action::AddChatMessage(ChatMessage::Assistant {
        content: "Based on the search results, Rust is a modern systems programming language...".to_string(),
        timestamp: chrono::Utc::now(),
    }))?;
    
    // Test error scenario
    chat.update(Action::Error("Network connection lost".to_string()))?;
    
    // Test rendering the entire conversation
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend)?;
    
    terminal.draw(|frame| {
        chat.draw(frame, frame.area()).unwrap();
    })?;
    
    // Test scrolling through the conversation
    chat.update(Action::ScrollChat(ScrollDirection::Up))?;
    chat.update(Action::ScrollChat(ScrollDirection::Down))?;
    chat.update(Action::ScrollChat(ScrollDirection::PageUp))?;
    chat.update(Action::ScrollChat(ScrollDirection::PageDown))?;
    
    // Test clearing everything
    chat.update(Action::ClearChat)?;
    
    // Render after clear
    terminal.draw(|frame| {
        chat.draw(frame, frame.area()).unwrap();
    })?;
    
    Ok(())
}