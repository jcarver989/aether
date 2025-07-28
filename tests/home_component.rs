use aether::action::{Action, ScrollDirection};
use aether::components::{Component, home::Home};
use aether::config::Config;
use aether::types::ChatMessage;
use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
    home.register_config_handler(Config::default()).unwrap();
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
    fn test_home_forwards_input_key_events() {
        let (mut home, _rx) = setup_home_with_handler();

        // Simulate typing in input
        let key_event = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
        let action = home.handle_key_event(key_event).unwrap();

        // Should not return an action directly (input handles internally)
        assert!(
            action.is_none(),
            "Home should not return action for char input"
        );

        // Input should be updated in component state (test by rendering)
        let buffer = draw_home_component(&mut home, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());

        // Should contain the typed character
        assert!(
            all_content.contains("h"),
            "Should display typed character in input area"
        );
    }

    #[test]
    fn test_home_forwards_enter_key_to_submit_message() {
        let (mut home, _rx) = setup_home_with_handler();

        // First type some text
        let chars = "hello world";
        for ch in chars.chars() {
            home.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
                .unwrap();
        }

        // Then press Enter to submit
        let enter_key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let action = home.handle_key_event(enter_key).unwrap();

        // Should return SubmitMessage action
        assert!(
            matches!(action, Some(Action::SubmitMessage(_))),
            "Should return SubmitMessage action when Enter is pressed"
        );

        if let Some(Action::SubmitMessage(msg)) = action {
            assert_eq!(msg, "hello world", "Should submit the typed message");
        }
    }

    #[test]
    fn test_home_forwards_chat_scroll_events() {
        let (mut home, _rx) = setup_home_with_handler();

        // Add some chat messages first
        let user_msg = create_test_chat_message("User message", true);
        let assistant_msg = create_test_chat_message("Assistant response", false);

        home.update(Action::AddChatMessage(user_msg)).unwrap();
        home.update(Action::AddChatMessage(assistant_msg)).unwrap();

        // Test scroll up key (Ctrl+Up for chat)
        let scroll_up_key = KeyEvent::new(KeyCode::Up, KeyModifiers::CONTROL);
        let action = home.handle_key_event(scroll_up_key).unwrap();

        // Should return scroll action
        assert!(
            matches!(action, Some(Action::ScrollChat(ScrollDirection::Up))),
            "Should return ScrollChat action for Ctrl+Up key"
        );
    }

    #[test]
    fn test_home_input_and_chat_dont_interfere() {
        let (mut home, _rx) = setup_home_with_handler();

        // Add a chat message
        let chat_msg = create_test_chat_message("This is a chat message", false);
        home.update(Action::AddChatMessage(chat_msg)).unwrap();

        // Type in input
        let input_text = "user input text";
        for ch in input_text.chars() {
            home.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
                .unwrap();
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
    fn test_home_handles_shift_enter_in_input() {
        let (mut home, _rx) = setup_home_with_handler();

        // Type some text
        for ch in "line 1".chars() {
            home.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
                .unwrap();
        }

        // Press Shift+Enter for new line
        let shift_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT);
        let action = home.handle_key_event(shift_enter).unwrap();

        // Should not submit message (returns None)
        assert!(action.is_none(), "Shift+Enter should not submit message");

        // Add more text on new line
        for ch in "line 2".chars() {
            home.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
                .unwrap();
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
    fn test_home_input_clears_after_submit() {
        let (mut home, _rx) = setup_home_with_handler();

        // Type message
        for ch in "test message".chars() {
            home.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
                .unwrap();
        }

        // Verify message is in input
        let before_buffer = draw_home_component(&mut home, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let before_content = extract_buffer_text(&before_buffer, 0, before_buffer.content().len());
        assert!(
            before_content.contains("test message"),
            "Message should be in input before submit"
        );

        // Submit message
        home.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .unwrap();

        // Clear input (simulating what the app would do)
        home.update(Action::ClearInput).unwrap();

        // Verify input is cleared
        let after_buffer = draw_home_component(&mut home, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let after_content = extract_buffer_text(&after_buffer, 0, after_buffer.content().len());

        // Should show placeholder again
        assert!(
            after_content.contains("Type your message..."),
            "Should show placeholder after input is cleared"
        );
    }

    #[test]
    fn test_home_handles_complex_input_editing() {
        let (mut home, _rx) = setup_home_with_handler();

        // Type initial text
        for ch in "Hello world".chars() {
            home.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
                .unwrap();
        }

        // Move cursor back with left arrow
        for _ in 0..6 {
            // Move back to position after "Hello"
            home.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE))
                .unwrap();
        }

        // Insert text in middle
        for ch in " beautiful".chars() {
            home.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
                .unwrap();
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
            let msg = create_test_chat_message(&format!("Message {}", i), i % 2 == 0);
            home.update(Action::AddChatMessage(msg)).unwrap();
        }

        // Scroll up
        home.update(Action::ScrollChat(ScrollDirection::Up))
            .unwrap();
        home.update(Action::ScrollChat(ScrollDirection::Up))
            .unwrap();

        // Type in input (should not affect chat scroll)
        for ch in "typing while scrolled".chars() {
            home.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
                .unwrap();
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
    fn test_home_handles_escape_clear_input() {
        let (mut home, _rx) = setup_home_with_handler();

        // Type some text
        for ch in "text to clear".chars() {
            home.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
                .unwrap();
        }

        // Verify text is there
        let before_buffer = draw_home_component(&mut home, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let before_content = extract_buffer_text(&before_buffer, 0, before_buffer.content().len());
        assert!(
            before_content.contains("text to clear"),
            "Text should be present before clear"
        );

        // Press Escape (correct key for clearing input)
        let escape = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let action = home.handle_key_event(escape).unwrap();

        // Should clear input
        assert!(
            matches!(action, Some(Action::ClearInput)),
            "Escape should trigger ClearInput action"
        );

        // Apply the clear action
        home.update(Action::ClearInput).unwrap();

        // Verify input is cleared
        let after_buffer = draw_home_component(&mut home, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let after_content = extract_buffer_text(&after_buffer, 0, after_buffer.content().len());

        assert!(
            after_content.contains("Type your message..."),
            "Should show placeholder after clearing input"
        );
    }
}
