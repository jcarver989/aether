// Tests for the Home component in the new action-based architecture.
// Key events are handled centrally in app.rs, and components only respond
// to actions via the update() method.

use aether::action::{Action, ScrollDirection, CursorDirection};
use aether::components::{Component, home::Home};
use aether::config::Config;
use aether::types::ChatMessage;
use chrono::Utc;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
use serde_json::json;
use tokio::sync::mpsc;

// Test buffer dimensions
const TEST_BUFFER_WIDTH: u16 = 80;
const TEST_BUFFER_HEIGHT: u16 = 24;

/// Helper function to extract text content from a buffer range
fn extract_buffer_text(buffer: &Buffer, start: usize, end: usize) -> String {
    buffer.content()[start..end]
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

/// Helper function to extract a single line from the buffer
fn extract_buffer_line(buffer: &Buffer, line: usize, width: usize) -> String {
    let start = line * width;
    let end = start + width;
    extract_buffer_text(buffer, start, end)
}

/// Helper function to create terminal and draw home component
fn draw_home_component(home: &mut Home, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let area = frame.area();
            home.draw(frame, area).unwrap();
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

/// Helper function to setup a home component with action handler
fn setup_home_with_handler() -> (Home, mpsc::UnboundedReceiver<Action>) {
    let mut home = Home::new();
    let (tx, rx) = mpsc::unbounded_channel();
    home.register_action_handler(tx).unwrap();
    home.register_config_handler(std::sync::Arc::new(Config::default())).unwrap();
    (home, rx)
}

/// Helper function to create test chat message
fn create_test_chat_message(content: &str, is_user: bool) -> ChatMessage {
    let timestamp = Utc::now();
    if is_user {
        ChatMessage::User {
            content: content.to_string(),
            timestamp,
        }
    } else {
        ChatMessage::Assistant {
            content: content.to_string(),
            timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_home_new_creates_empty_state() {
        let mut home = Home::new();
        let buffer = draw_home_component(&mut home, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // Should render empty chat and input areas
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());

        // Should find input placeholder
        assert!(
            all_content.contains("Type your message..."),
            "Should display input placeholder when empty"
        );

        // Should contain prompt marker
        assert!(
            all_content.contains("> "),
            "Should display input prompt marker"
        );
    }

    #[test]
    fn test_home_layout_renders_chat_and_input_areas() {
        let mut home = Home::new();
        let buffer = draw_home_component(&mut home, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // Should have both chat and input borders
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());

        // Should find input area title
        assert!(
            all_content.contains("Input"),
            "Should display Input section title"
        );

        // Should have border elements
        assert!(
            all_content.contains("┌") && all_content.contains("┐"),
            "Should display top borders"
        );
        assert!(
            all_content.contains("└") && all_content.contains("┘"),
            "Should display bottom borders"
        );
    }

    #[test]
    fn test_home_processes_insert_char_action() {
        let (mut home, _rx) = setup_home_with_handler();

        // Process InsertChar action directly (as app.rs would do)
        let result = home.update(Action::InsertChar('h')).unwrap();
        assert_eq!(result, None, "Home should not return additional action for InsertChar");

        // After processing action, input should show the character
        let buffer = draw_home_component(&mut home, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());

        assert!(
            all_content.contains("> h"),
            "Should display typed character after action processing"
        );
        assert!(
            !all_content.contains("Type your message..."),
            "Should not display placeholder after typing"
        );
    }

    #[test]
    fn test_home_processes_try_submit_message_action() {
        let (mut home, _rx) = setup_home_with_handler();

        // First type some text by processing InsertChar actions
        let chars = "hello world";
        for ch in chars.chars() {
            home.update(Action::InsertChar(ch)).unwrap();
        }

        // Process TrySubmitMessage action (emitted when Enter is pressed)
        let result = home.update(Action::TrySubmitMessage).unwrap();
        
        // Should return SubmitMessage action since input is not empty
        assert!(
            matches!(result, Some(Action::SubmitMessage(_))),
            "TrySubmitMessage should return SubmitMessage when input is not empty"
        );

        if let Some(Action::SubmitMessage(msg)) = result {
            assert_eq!(msg, "hello world", "Should submit the typed message");
        }
    }

    #[test]
    fn test_home_processes_scroll_chat_action() {
        let (mut home, _rx) = setup_home_with_handler();

        // Add some chat messages first
        let user_msg = create_test_chat_message("User message", true);
        let assistant_msg = create_test_chat_message("Assistant response", false);

        home.update(Action::AddChatMessage(user_msg)).unwrap();
        home.update(Action::AddChatMessage(assistant_msg)).unwrap();

        // Process ScrollChat action directly (as app.rs would do)
        let result = home.update(Action::ScrollChat(ScrollDirection::Up)).unwrap();
        
        // Home forwards to child components, should not return additional action
        assert_eq!(result, None, "ScrollChat action should be handled by child components");

        // Verify chat is still functional after scroll action
        let buffer = draw_home_component(&mut home, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());
        
        // Should still show messages
        assert!(
            all_content.contains("User message") || all_content.contains("Assistant response"),
            "Chat messages should still be visible after scroll"
        );
    }

    #[test]
    fn test_home_input_and_chat_dont_interfere() {
        let (mut home, _rx) = setup_home_with_handler();

        // Add a chat message
        let chat_msg = create_test_chat_message("This is a chat message", false);
        home.update(Action::AddChatMessage(chat_msg)).unwrap();

        // Type in input by processing InsertChar actions
        let input_text = "user input text";
        for ch in input_text.chars() {
            home.update(Action::InsertChar(ch)).unwrap();
        }

        // Render and check both are present independently
        let buffer = draw_home_component(&mut home, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());

        // Both chat message and input should be visible
        assert!(
            all_content.contains("This is a chat message"),
            "Chat message should be visible"
        );
        assert!(
            all_content.contains("user input text"),
            "Input text should be visible"
        );

        // They should appear in different areas (chat above, input below)
        let lines: Vec<String> = (0..TEST_BUFFER_HEIGHT)
            .map(|i| extract_buffer_line(&buffer, i as usize, TEST_BUFFER_WIDTH as usize))
            .collect();

        let chat_line_idx = lines
            .iter()
            .position(|line| line.contains("This is a chat message"));
        let input_line_idx = lines
            .iter()
            .position(|line| line.contains("user input text"));

        assert!(
            chat_line_idx.is_some(),
            "Chat message should be found in buffer"
        );
        assert!(
            input_line_idx.is_some(),
            "Input text should be found in buffer"
        );
        assert!(
            chat_line_idx.unwrap() < input_line_idx.unwrap(),
            "Chat should appear above input"
        );
    }

    #[test]
    fn test_home_processes_insert_newline_action() {
        let (mut home, _rx) = setup_home_with_handler();

        // Type some text by processing InsertChar actions
        for ch in "line 1".chars() {
            home.update(Action::InsertChar(ch)).unwrap();
        }

        // Process InsertNewline action (emitted when Shift+Enter is pressed)
        let result = home.update(Action::InsertNewline).unwrap();
        assert_eq!(result, None, "InsertNewline should be handled by child components");

        // Add more text on new line by processing InsertChar actions
        for ch in "line 2".chars() {
            home.update(Action::InsertChar(ch)).unwrap();
        }

        // Check buffer shows both lines
        let buffer = draw_home_component(&mut home, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());

        assert!(all_content.contains("line 1"), "Should contain first line");
        assert!(all_content.contains("line 2"), "Should contain second line");
        assert!(all_content.contains("2 lines"), "Should show line count");
    }

    #[test]
    fn test_home_updates_chat_with_new_messages() {
        let (mut home, _rx) = setup_home_with_handler();

        // Initially empty
        let empty_buffer = draw_home_component(&mut home, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let _empty_content = extract_buffer_text(&empty_buffer, 0, empty_buffer.content().len());

        // Add user message
        let user_msg = create_test_chat_message("Hello assistant!", true);
        home.update(Action::AddChatMessage(user_msg)).unwrap();

        // Add assistant response
        let assistant_msg = create_test_chat_message("Hello! How can I help you?", false);
        home.update(Action::AddChatMessage(assistant_msg)).unwrap();

        // Check both messages appear in buffer
        let updated_buffer = draw_home_component(&mut home, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let updated_content =
            extract_buffer_text(&updated_buffer, 0, updated_buffer.content().len());

        assert!(
            updated_content.contains("Hello assistant!"),
            "Should display user message"
        );
        assert!(
            updated_content.contains("Hello! How can I help you?"),
            "Should display assistant message"
        );
    }

    #[test]
    fn test_home_handles_tool_call_display() {
        let (mut home, _rx) = setup_home_with_handler();

        // Add a tool call message
        let tool_call_msg = ChatMessage::ToolCall {
            id: "call_123".to_string(),
            name: "test_tool".to_string(),
            params: json!({"param": "value"}).to_string(),
            timestamp: Utc::now(),
        };
        home.update(Action::AddChatMessage(tool_call_msg)).unwrap();

        // Add tool result
        let tool_result_msg = ChatMessage::ToolResult {
            tool_call_id: "call_123".to_string(),
            content: "Tool execution result".to_string(),
            timestamp: Utc::now(),
        };
        home.update(Action::AddChatMessage(tool_result_msg))
            .unwrap();

        // Check buffer contains tool call information
        let buffer = draw_home_component(&mut home, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());

        assert!(
            all_content.contains("test_tool"),
            "Should display tool name"
        );
        assert!(
            all_content.contains("Tool execution result"),
            "Should display tool result"
        );
    }

    #[test]
    fn test_home_input_clears_after_submit_message_action() {
        let (mut home, _rx) = setup_home_with_handler();

        // Type message by processing InsertChar actions
        for ch in "test message".chars() {
            home.update(Action::InsertChar(ch)).unwrap();
        }

        // Verify message is in input
        let before_buffer = draw_home_component(&mut home, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let before_content = extract_buffer_text(&before_buffer, 0, before_buffer.content().len());
        assert!(
            before_content.contains("test message"),
            "Message should be in input before submit"
        );

        // Process SubmitMessage action (as app.rs would do after receiving TrySubmitMessage)
        home.update(Action::SubmitMessage("test message".to_string())).unwrap();

        // Verify input is cleared after SubmitMessage
        let after_buffer = draw_home_component(&mut home, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let after_content = extract_buffer_text(&after_buffer, 0, after_buffer.content().len());

        // Should show placeholder again
        assert!(
            after_content.contains("Type your message..."),
            "Should show placeholder after input is cleared"
        );
    }

    #[test]
    fn test_home_processes_complex_input_editing_actions() {
        let (mut home, _rx) = setup_home_with_handler();

        // Type initial text by processing InsertChar actions
        for ch in "Hello world".chars() {
            home.update(Action::InsertChar(ch)).unwrap();
        }

        // Move cursor back by processing MoveCursor actions
        for _ in 0..6 {
            // Move back to position after "Hello"
            home.update(Action::MoveCursor(CursorDirection::Left)).unwrap();
        }

        // Insert text in middle by processing InsertChar actions
        for ch in " beautiful".chars() {
            home.update(Action::InsertChar(ch)).unwrap();
        }

        // Check result
        let buffer = draw_home_component(&mut home, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());

        assert!(
            all_content.contains("Hello beautiful world"),
            "Should handle complex text editing correctly"
        );
    }

    #[test]
    fn test_home_preserves_chat_scroll_position() {
        let (mut home, _rx) = setup_home_with_handler();

        // Add many messages to enable scrolling
        for i in 0..10 {
            let msg = create_test_chat_message(&format!("Message {i}"), i % 2 == 0);
            home.update(Action::AddChatMessage(msg)).unwrap();
        }

        // Scroll up by processing ScrollChat actions
        home.update(Action::ScrollChat(ScrollDirection::Up))
            .unwrap();
        home.update(Action::ScrollChat(ScrollDirection::Up))
            .unwrap();

        // Type in input (should not affect chat scroll) by processing InsertChar actions
        for ch in "typing while scrolled".chars() {
            home.update(Action::InsertChar(ch)).unwrap();
        }

        // Buffer should show both input text and maintain scroll position
        let buffer = draw_home_component(&mut home, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());

        assert!(
            all_content.contains("typing while scrolled"),
            "Input should be visible while chat is scrolled"
        );

        // The latest messages might not be visible due to scroll
        // This is more of a state consistency test
    }

    #[test]
    fn test_home_processes_clear_input_action() {
        let (mut home, _rx) = setup_home_with_handler();

        // Type some text by processing InsertChar actions
        for ch in "text to clear".chars() {
            home.update(Action::InsertChar(ch)).unwrap();
        }

        // Verify text is there
        let before_buffer = draw_home_component(&mut home, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let before_content = extract_buffer_text(&before_buffer, 0, before_buffer.content().len());
        assert!(
            before_content.contains("text to clear"),
            "Text should be present before clear"
        );

        // Process ClearInput action (emitted when Escape is pressed)
        let result = home.update(Action::ClearInput).unwrap();
        assert_eq!(result, None, "ClearInput should be handled by child components");

        // Verify input is cleared
        let after_buffer = draw_home_component(&mut home, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let after_content = extract_buffer_text(&after_buffer, 0, after_buffer.content().len());

        assert!(
            after_content.contains("Type your message..."),
            "Should show placeholder after clearing input"
        );
    }

    #[test]
    fn test_single_action_produces_single_character() {
        let (mut home, _rx) = setup_home_with_handler();

        // Process a single InsertChar action
        let result = home.update(Action::InsertChar('h')).unwrap();
        assert_eq!(result, None, "InsertChar should be handled by child components");

        // Verify only one character was inserted by checking the input area
        let buffer = draw_home_component(&mut home, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let content = extract_buffer_text(&buffer, 0, buffer.content().len());
        
        // Look for the input line with "> h" (there should only be one occurrence)
        assert!(
            content.contains("> h"),
            "Should contain input with 'h'"
        );
        
        // Count occurrences of "> h" to ensure no duplication in the input
        let input_h_count = content.matches("> h").count();
        assert_eq!(
            input_h_count, 1,
            "Should only have one '> h' input pattern, found {} occurrences. Content: {}",
            input_h_count, content
        );
        
        // Verify placeholder is not shown
        assert!(
            !content.contains("Type your message..."),
            "Should not show placeholder after inserting character"
        );
    }
}
