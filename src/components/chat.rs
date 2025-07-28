use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::{Rect, Size},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, StatefulWidget},
};
use tokio::sync::mpsc::UnboundedSender;
use tui_scrollview::{ScrollView, ScrollViewState};

use super::Component;
use crate::{
    action::{Action, ScrollDirection},
    config::Config,
    theme::Theme,
    types::ChatMessage,
};

pub struct Chat {
    messages: Vec<ChatMessage>,
    scroll_state: ScrollViewState,
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    theme: Theme,
    auto_scroll: bool,
    cached_content: Option<Text<'static>>,
    content_dirty: bool,
    last_content_height: u16,
}

impl Default for Chat {
    fn default() -> Self {
        Self::new()
    }
}

impl Chat {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            scroll_state: ScrollViewState::default(),
            command_tx: None,
            config: Config::default(),
            theme: Theme::default(),
            auto_scroll: true,
            cached_content: None,
            content_dirty: true,
            last_content_height: 0,
        }
    }

    fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
        self.auto_scroll = true;
        self.content_dirty = true;
    }

    fn auto_scroll_to_bottom(&mut self) {
        self.auto_scroll = true;
    }

    fn clear_messages(&mut self) {
        self.messages.clear();
        self.scroll_state = ScrollViewState::default();
        self.auto_scroll = true;
        self.content_dirty = true;
        self.cached_content = None;
    }

    fn format_message(&self, message: &ChatMessage) -> Vec<Line<'static>> {
        match message {
            ChatMessage::System { content, timestamp } => {
                vec![Line::from(vec![
                    Span::styled(
                        "System",
                        Style::default()
                            .fg(self.theme.system_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" ("),
                    Span::styled(
                        timestamp.format("%H:%M:%S").to_string(),
                        Style::default().fg(self.theme.muted),
                    ),
                    Span::raw("): "),
                    Span::styled(content.clone(), Style::default().fg(self.theme.subtle)),
                ])]
            }
            ChatMessage::User { content, timestamp } => {
                vec![Line::from(vec![
                    Span::styled(
                        "You",
                        Style::default()
                            .fg(self.theme.user_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" ("),
                    Span::styled(
                        timestamp.format("%H:%M:%S").to_string(),
                        Style::default().fg(self.theme.muted),
                    ),
                    Span::raw("): "),
                    Span::styled(content.clone(), Style::default().fg(self.theme.foreground)),
                ])]
            }
            ChatMessage::Assistant { content, timestamp } => {
                let mut lines = vec![Line::from(vec![
                    Span::styled(
                        "Assistant",
                        Style::default()
                            .fg(self.theme.assistant_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" ("),
                    Span::styled(
                        timestamp.format("%H:%M:%S").to_string(),
                        Style::default().fg(self.theme.muted),
                    ),
                    Span::raw("): "),
                ])];

                let formatted_lines = self.format_assistant_content(content);
                lines.extend(formatted_lines);
                lines
            }
            ChatMessage::AssistantStreaming { content, timestamp } => {
                let mut lines = vec![Line::from(vec![
                    Span::styled(
                        "Assistant",
                        Style::default()
                            .fg(self.theme.assistant_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" ("),
                    Span::styled(
                        timestamp.format("%H:%M:%S").to_string(),
                        Style::default().fg(self.theme.muted),
                    ),
                    Span::raw("): "),
                ])];

                let mut formatted_lines = self.format_assistant_content(content);
                // Add cursor indicator for streaming
                if let Some(last_line) = formatted_lines.last_mut() {
                    let mut spans = last_line.spans.clone();
                    spans.push(Span::styled(
                        " ▋",
                        Style::default().fg(self.theme.cursor_color),
                    ));
                    *last_line = Line::from(spans);
                } else {
                    formatted_lines.push(Line::from(Span::styled(
                        " ▋",
                        Style::default().fg(self.theme.cursor_color),
                    )));
                }
                lines.extend(formatted_lines);
                lines
            }
            ChatMessage::Tool {
                tool_call_id,
                content,
                timestamp,
            } => {
                vec![Line::from(vec![
                    Span::styled(
                        "Tool",
                        Style::default()
                            .fg(self.theme.tool_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" ("),
                    Span::styled(tool_call_id.clone(), Style::default().fg(self.theme.subtle)),
                    Span::raw(") "),
                    Span::styled(
                        timestamp.format("%H:%M:%S").to_string(),
                        Style::default().fg(self.theme.muted),
                    ),
                    Span::raw(": "),
                    Span::styled(content.clone(), Style::default().fg(self.theme.success)),
                ])]
            }
            ChatMessage::ToolCall {
                id,
                name,
                params,
                timestamp,
            } => {
                vec![Line::from(vec![
                    Span::styled(
                        "Tool Call",
                        Style::default()
                            .fg(self.theme.tool_call_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" ("),
                    Span::styled(id.clone(), Style::default().fg(self.theme.subtle)),
                    Span::raw(") "),
                    Span::styled(
                        timestamp.format("%H:%M:%S").to_string(),
                        Style::default().fg(self.theme.muted),
                    ),
                    Span::raw(": "),
                    Span::styled(
                        format!("{}({})", name, params),
                        Style::default().fg(self.theme.foreground),
                    ),
                ])]
            }
            ChatMessage::ToolResult {
                tool_call_id,
                content,
                timestamp,
            } => {
                vec![Line::from(vec![
                    Span::styled(
                        "Result",
                        Style::default()
                            .fg(self.theme.tool_result_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" ("),
                    Span::styled(tool_call_id.clone(), Style::default().fg(self.theme.subtle)),
                    Span::raw(") "),
                    Span::styled(
                        timestamp.format("%H:%M:%S").to_string(),
                        Style::default().fg(self.theme.muted),
                    ),
                    Span::raw(": "),
                    Span::styled(content.clone(), Style::default().fg(self.theme.foreground)),
                ])]
            }
            ChatMessage::Error { message, timestamp } => {
                vec![Line::from(vec![
                    Span::styled(
                        "Error",
                        Style::default()
                            .fg(self.theme.error)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" ("),
                    Span::styled(
                        timestamp.format("%H:%M:%S").to_string(),
                        Style::default().fg(self.theme.muted),
                    ),
                    Span::raw("): "),
                    Span::styled(message.clone(), Style::default().fg(self.theme.error)),
                ])]
            }
        }
    }

    fn format_assistant_content(&self, content: &str) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let mut in_code_block = false;
        let mut code_language = String::new();

        // Limit content to prevent performance issues
        const MAX_LINES: usize = 1000;
        let mut line_count = 0;
        let mut truncated = false;

        for line in content.lines() {
            if line_count >= MAX_LINES {
                truncated = true;
                break;
            }
            if line.starts_with("```") {
                if in_code_block {
                    in_code_block = false;
                    code_language.clear();
                    lines.push(Line::from(Span::styled(
                        "```",
                        Style::default().fg(self.theme.muted),
                    )));
                } else {
                    in_code_block = true;
                    code_language = line.trim_start_matches("```").to_string();
                    lines.push(Line::from(vec![
                        Span::styled("```", Style::default().fg(self.theme.muted)),
                        Span::styled(
                            code_language.clone(),
                            Style::default().fg(self.theme.warning),
                        ),
                    ]));
                }
            } else if in_code_block {
                lines.push(Line::from(Span::styled(
                    format!("  {}", line),
                    Style::default()
                        .fg(self.theme.code_fg)
                        .bg(self.theme.code_bg),
                )));
            } else {
                let formatted_line = self.format_markdown_line(line);
                lines.push(formatted_line);
            }
            line_count += 1;
        }

        if truncated {
            lines.push(Line::from(Span::styled(
                "... [Content truncated for performance]",
                Style::default().fg(self.theme.muted),
            )));
        }

        lines
    }

    fn format_markdown_line(&self, line: &str) -> Line<'static> {
        let mut spans = Vec::new();
        let mut chars = line.chars().peekable();
        let mut current_text = String::new();
        let mut in_bold = false;
        let mut in_italic = false;
        let mut in_code = false;

        while let Some(ch) = chars.next() {
            match ch {
                '*' if chars.peek() == Some(&'*') && !in_code => {
                    if !current_text.is_empty() {
                        let style = if in_italic {
                            Style::default().add_modifier(Modifier::ITALIC)
                        } else {
                            Style::default()
                        };
                        spans.push(Span::styled(current_text.clone(), style));
                        current_text.clear();
                    }
                    chars.next();
                    in_bold = !in_bold;
                }
                '*' if !in_code => {
                    if !current_text.is_empty() {
                        let style = if in_bold {
                            Style::default().add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        };
                        spans.push(Span::styled(current_text.clone(), style));
                        current_text.clear();
                    }
                    in_italic = !in_italic;
                }
                '`' if !in_bold && !in_italic => {
                    if !current_text.is_empty() {
                        spans.push(Span::raw(current_text.clone()));
                        current_text.clear();
                    }
                    in_code = !in_code;
                }
                _ => {
                    current_text.push(ch);
                }
            }
        }

        if !current_text.is_empty() {
            let mut style = Style::default();
            if in_bold {
                style = style.add_modifier(Modifier::BOLD);
            }
            if in_italic {
                style = style.add_modifier(Modifier::ITALIC);
            }
            if in_code {
                style = style.fg(self.theme.code_fg).bg(self.theme.code_bg);
            }

            spans.push(Span::styled(current_text, style));
        }

        if spans.is_empty() {
            Line::from("")
        } else {
            Line::from(spans)
        }
    }

    fn create_message_content(&mut self) -> &Text<'static> {
        if self.content_dirty || self.cached_content.is_none() {
            let mut all_lines = Vec::new();

            for (i, message) in self.messages.iter().enumerate() {
                let message_lines = self.format_message(message);
                all_lines.extend(message_lines);

                // Add spacing between messages (except for last message)
                if i < self.messages.len() - 1 {
                    all_lines.push(Line::from(""));
                }
            }

            self.cached_content = Some(Text::from(all_lines));
            self.content_dirty = false;
        }

        self.cached_content.as_ref().unwrap()
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Size};

    // Test buffer dimensions for chat testing
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

    /// Helper function to create terminal and draw chat component
    fn draw_chat_component(chat: &mut Chat, width: u16, height: u16) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let area = frame.area();
                chat.draw(frame, area).unwrap();
            })
            .unwrap();

        terminal.backend().buffer().clone()
    }

    /// Helper function to create a test chat message
    fn create_test_user_message(content: &str) -> ChatMessage {
        ChatMessage::User {
            content: content.to_string(),
            timestamp: Utc::now(),
        }
    }

    /// Helper function to create a test assistant message
    fn create_test_assistant_message(content: &str) -> ChatMessage {
        ChatMessage::Assistant {
            content: content.to_string(),
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_chat_new() {
        let chat = Chat::new();
        assert!(chat.messages.is_empty());
        assert!(chat.command_tx.is_none());
        assert!(chat.auto_scroll);
        assert!(chat.content_dirty);
        assert_eq!(chat.last_content_height, 0);
        assert!(chat.cached_content.is_none());
    }

    #[test]
    fn test_chat_default() {
        let chat = Chat::default();
        assert!(chat.messages.is_empty());
        assert!(chat.command_tx.is_none());
        assert!(chat.auto_scroll);
        assert!(chat.content_dirty);
        assert_eq!(chat.last_content_height, 0);
        assert!(chat.cached_content.is_none());
    }

    #[test]
    fn test_draw_shows_tool_call_in_buffer() {
        let mut chat = Chat::new();

        // Stream a tool call
        chat.update(Action::StreamToolCall {
            id: "call_123".to_string(),
            name: "get_weather".to_string(),
            arguments: "{\"location\": \"San Francisco\"}".to_string(),
        })
        .unwrap();

        let buffer = draw_chat_component(&mut chat, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // Should find tool call in buffer
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());
        assert!(
            all_content.contains("Tool Call"),
            "Should find 'Tool Call' in buffer"
        );
        assert!(
            all_content.contains("get_weather"),
            "Should find tool name in buffer"
        );
        assert!(
            all_content.contains("call_123"),
            "Should find tool call ID in buffer"
        );
    }

    #[test]
    fn test_draw_shows_updated_tool_call_arguments() {
        let mut chat = Chat::new();

        // Stream initial tool call
        chat.update(Action::StreamToolCall {
            id: "call_123".to_string(),
            name: "test_tool".to_string(),
            arguments: "{}".to_string(),
        })
        .unwrap();

        // Update with more arguments
        chat.update(Action::StreamToolCall {
            id: "call_123".to_string(),
            name: "test_tool".to_string(),
            arguments: "{\"param\": \"value\"}".to_string(),
        })
        .unwrap();

        let buffer = draw_chat_component(&mut chat, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // Should find updated arguments in buffer
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());
        assert!(
            all_content.contains("test_tool"),
            "Should find tool name in buffer"
        );
        assert!(
            all_content.contains("param"),
            "Should find parameter name in buffer"
        );
        assert!(
            all_content.contains("value"),
            "Should find parameter value in buffer"
        );
    }

    #[test]
    fn test_key_event_handling() {
        let mut chat = Chat::new();

        // Test Ctrl+Up
        let key_event = KeyEvent::new(KeyCode::Up, KeyModifiers::CONTROL);
        let result = chat.handle_key_event(key_event).unwrap();
        assert_eq!(result, Some(Action::ScrollChat(ScrollDirection::Up)));

        // Test Ctrl+Down
        let key_event = KeyEvent::new(KeyCode::Down, KeyModifiers::CONTROL);
        let result = chat.handle_key_event(key_event).unwrap();
        assert_eq!(result, Some(Action::ScrollChat(ScrollDirection::Down)));

        // Test PageUp
        let key_event = KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE);
        let result = chat.handle_key_event(key_event).unwrap();
        assert_eq!(result, Some(Action::ScrollChat(ScrollDirection::PageUp)));

        // Test PageDown
        let key_event = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
        let result = chat.handle_key_event(key_event).unwrap();
        assert_eq!(result, Some(Action::ScrollChat(ScrollDirection::PageDown)));

        // Test unhandled key
        let key_event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let result = chat.handle_key_event(key_event).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_mouse_event_handling() {
        let mut chat = Chat::new();

        // Test scroll up
        let mouse_event = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        let result = chat.handle_mouse_event(mouse_event).unwrap();
        assert_eq!(result, Some(Action::ScrollChat(ScrollDirection::Up)));

        // Test scroll down
        let mouse_event = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        let result = chat.handle_mouse_event(mouse_event).unwrap();
        assert_eq!(result, Some(Action::ScrollChat(ScrollDirection::Down)));

        // Test unhandled mouse event
        let mouse_event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        let result = chat.handle_mouse_event(mouse_event).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_update_returns_none_for_all_actions() {
        let mut chat = Chat::new();

        // Test various actions return None (no further actions emitted)
        let actions = vec![
            Action::Tick,
            Action::Render,
            Action::AddChatMessage(create_test_user_message("test")),
            Action::ClearChat,
            Action::ScrollChat(ScrollDirection::Up),
            Action::StartStreaming,
            Action::StreamContent("test".to_string()),
            Action::StreamComplete,
            Action::Error("test error".to_string()),
        ];

        for action in actions {
            let result = chat.update(action);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), None);
        }
    }

    #[test]
    fn test_draw_renders_empty_chat_with_borders() {
        let mut chat = Chat::new();
        let buffer = draw_chat_component(&mut chat, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // First line should contain the top border with "Chat" title
        let first_line = extract_buffer_line(&buffer, 0, TEST_BUFFER_WIDTH as usize);
        assert!(first_line.contains("┌"), "Should contain top-left corner");
        assert!(first_line.contains("Chat"), "Should contain Chat title");
        assert!(first_line.contains("┐"), "Should contain top-right corner");

        // Last line should contain the bottom border
        let last_line = extract_buffer_line(
            &buffer,
            (TEST_BUFFER_HEIGHT - 1) as usize,
            TEST_BUFFER_WIDTH as usize,
        );
        assert!(last_line.contains("└"), "Should contain bottom-left corner");
        assert!(
            last_line.contains("┘"),
            "Should contain bottom-right corner"
        );

        // Middle lines should have side borders
        for line_num in 1..(TEST_BUFFER_HEIGHT - 1) {
            let line = extract_buffer_line(&buffer, line_num as usize, TEST_BUFFER_WIDTH as usize);
            assert!(
                line.starts_with('│'),
                "Line {} should start with left border",
                line_num
            );
            assert!(
                line.ends_with('│'),
                "Line {} should end with right border",
                line_num
            );
        }
    }

    #[test]
    fn test_draw_renders_user_message_content() {
        let mut chat = Chat::new();

        // Add a test message
        let message = create_test_user_message("Hello");
        chat.update(Action::AddChatMessage(message)).unwrap();

        let buffer = draw_chat_component(&mut chat, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // Should find user message in buffer content
        let mut found_user_line = false;
        for line_num in 1..(TEST_BUFFER_HEIGHT - 1) {
            let line = extract_buffer_line(&buffer, line_num as usize, TEST_BUFFER_WIDTH as usize);

            if line.contains("You") && line.contains("Hello") {
                found_user_line = true;
                break;
            }
        }
        assert!(
            found_user_line,
            "Should find user message with 'You' and 'Hello' in buffer"
        );
    }

    #[test]
    fn test_draw_renders_assistant_message_content() {
        let mut chat = Chat::new();

        // Add an assistant message
        let message = create_test_assistant_message("I can help you with that.");
        chat.update(Action::AddChatMessage(message)).unwrap();

        let buffer = draw_chat_component(&mut chat, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // Should find assistant message header
        let mut found_assistant_line = false;
        for line_num in 1..(TEST_BUFFER_HEIGHT - 1) {
            let line = extract_buffer_line(&buffer, line_num as usize, TEST_BUFFER_WIDTH as usize);

            if line.contains("Assistant") {
                found_assistant_line = true;
                break;
            }
        }
        assert!(
            found_assistant_line,
            "Should find assistant message header in buffer"
        );

        // Check for message content
        let mut found_content = false;
        for line_num in 1..(TEST_BUFFER_HEIGHT - 1) {
            let line = extract_buffer_line(&buffer, line_num as usize, TEST_BUFFER_WIDTH as usize);

            if line.contains("I can help you with that") {
                found_content = true;
                break;
            }
        }
        assert!(
            found_content,
            "Should find assistant message content in buffer"
        );
    }

    #[test]
    fn test_draw_renders_multiple_messages_in_order() {
        let mut chat = Chat::new();

        // Add multiple messages
        let user_msg = create_test_user_message("First message");
        let assistant_msg = create_test_assistant_message("Response");
        let user_msg2 = create_test_user_message("Second message");

        chat.update(Action::AddChatMessage(user_msg)).unwrap();
        chat.update(Action::AddChatMessage(assistant_msg)).unwrap();
        chat.update(Action::AddChatMessage(user_msg2)).unwrap();

        let buffer = draw_chat_component(&mut chat, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // Extract all buffer content
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());

        // Messages should appear in order
        let first_pos = all_content.find("First message").unwrap_or(usize::MAX);
        let response_pos = all_content.find("Response").unwrap_or(usize::MAX);
        let second_pos = all_content.find("Second message").unwrap_or(usize::MAX);

        assert!(
            first_pos < response_pos,
            "First message should appear before response"
        );
        assert!(
            response_pos < second_pos,
            "Response should appear before second message"
        );
    }

    #[test]
    fn test_draw_renders_streaming_message_with_cursor() {
        let mut chat = Chat::new();

        // Start streaming and add some content
        chat.update(Action::StartStreaming).unwrap();
        chat.update(Action::StreamContent("Streaming".to_string()))
            .unwrap();

        let buffer = draw_chat_component(&mut chat, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // Should find streaming content with cursor
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());
        assert!(
            all_content.contains("Streaming"),
            "Should find streaming content in buffer"
        );
        assert!(all_content.contains("▋"), "Should find cursor in buffer");
    }

    #[test]
    fn test_draw_renders_error_message() {
        let mut chat = Chat::new();

        chat.update(Action::Error("Something went wrong".to_string()))
            .unwrap();

        let buffer = draw_chat_component(&mut chat, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // Should find error message
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());
        assert!(
            all_content.contains("Error"),
            "Should find 'Error' in buffer"
        );
        assert!(
            all_content.contains("Something went wrong"),
            "Should find error message in buffer"
        );
    }

    #[test]
    fn test_draw_empty_chat_content_area() {
        let mut chat = Chat::new();
        let buffer = draw_chat_component(&mut chat, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // Extract all content and check it only contains borders and spaces
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());

        // Should not contain any message content
        assert!(
            !all_content.contains("You"),
            "Empty chat should not contain 'You'"
        );
        assert!(
            !all_content.contains("Assistant"),
            "Empty chat should not contain 'Assistant'"
        );
        assert!(
            !all_content.contains("Error"),
            "Empty chat should not contain 'Error'"
        );

        // Should contain borders
        assert!(all_content.contains("Chat"), "Should contain Chat title");
        assert!(all_content.contains("┌"), "Should contain top border");
        assert!(all_content.contains("└"), "Should contain bottom border");
    }

    #[test]
    fn test_draw_renders_code_blocks_in_buffer() {
        let mut chat = Chat::new();

        // Add assistant message with code block
        let message = create_test_assistant_message("Here's code:\n```rust\nfn main() {}\n```");
        chat.update(Action::AddChatMessage(message)).unwrap();

        let buffer = draw_chat_component(&mut chat, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // Should find code block markers and content in buffer
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());

        assert!(
            all_content.contains("```"),
            "Should find code block markers in buffer"
        );
        assert!(
            all_content.contains("rust"),
            "Should find rust language marker in buffer"
        );
        assert!(
            all_content.contains("fn main()"),
            "Should find code content in buffer"
        );
    }

    #[test]
    fn test_draw_renders_markdown_formatting_in_buffer() {
        let mut chat = Chat::new();

        // Add message with markdown formatting
        let message = create_test_assistant_message("This is **bold** and *italic* and `code`");
        chat.update(Action::AddChatMessage(message)).unwrap();

        let buffer = draw_chat_component(&mut chat, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // Should find the formatted content in buffer
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());

        assert!(
            all_content.contains("bold"),
            "Should find 'bold' text in buffer"
        );
        assert!(
            all_content.contains("italic"),
            "Should find 'italic' text in buffer"
        );
        assert!(
            all_content.contains("code"),
            "Should find 'code' text in buffer"
        );
    }

    #[test]
    fn test_component_trait_methods_default_implementations() {
        let mut chat = Chat::new();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

        // Test register_action_handler
        let result = chat.register_action_handler(tx.clone());
        assert!(result.is_ok());
        assert!(chat.command_tx.is_some());

        // Test register_config_handler
        let config = Config::default();
        let result = chat.register_config_handler(config);
        assert!(result.is_ok());

        // Test init (default implementation)
        let size = Size::new(80, 24);
        let result = chat.init(size);
        assert!(result.is_ok());

        // Test handle_events (default implementation)
        let result = chat.handle_events(None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_draw_after_clear_shows_empty_buffer() {
        let mut chat = Chat::new();

        // Add some messages first
        chat.update(Action::AddChatMessage(create_test_user_message(
            "Message 1",
        )))
        .unwrap();
        chat.update(Action::AddChatMessage(create_test_user_message(
            "Message 2",
        )))
        .unwrap();

        // Clear the chat
        chat.update(Action::ClearChat).unwrap();

        let buffer = draw_chat_component(&mut chat, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // Should not contain the previous messages
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());
        assert!(
            !all_content.contains("Message 1"),
            "Should not contain cleared message 1"
        );
        assert!(
            !all_content.contains("Message 2"),
            "Should not contain cleared message 2"
        );

        // Should still contain borders and title
        assert!(
            all_content.contains("Chat"),
            "Should still contain Chat title"
        );
    }

    #[test]
    fn test_draw_shows_streaming_completion() {
        let mut chat = Chat::new();

        // Start streaming
        chat.update(Action::StartStreaming).unwrap();
        chat.update(Action::StreamContent("Hello world".to_string()))
            .unwrap();

        // Complete streaming
        chat.update(Action::StreamComplete).unwrap();

        let buffer = draw_chat_component(&mut chat, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // Should find completed message without cursor
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());
        assert!(
            all_content.contains("Hello world"),
            "Should find completed message content"
        );
        assert!(
            !all_content.contains("▋"),
            "Should not contain cursor after completion"
        );
    }

    #[test]
    fn test_draw_scrolling_with_overflow_content() {
        let mut chat = Chat::new();

        // Add enough messages to overflow the view (accounting for borders)
        // With TEST_BUFFER_HEIGHT=24, we have ~22 lines of content area
        // Let's add 30 short messages to ensure overflow
        for i in 1..=30 {
            let message = create_test_user_message(&format!("Message {}", i));
            chat.update(Action::AddChatMessage(message)).unwrap();
        }

        // Initially, should see recent messages but not the first ones
        let buffer_initial = draw_chat_component(&mut chat, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let initial_content =
            extract_buffer_text(&buffer_initial, 0, buffer_initial.content().len());

        // Should see the last message (Message 30) since auto-scroll is enabled
        assert!(
            initial_content.contains("Message 30"),
            "Should see last message initially"
        );
        // Should NOT see the first message due to overflow
        assert!(
            !initial_content.contains("Message 1"),
            "Should not see first message initially due to overflow"
        );

        // Scroll up to see earlier content
        for _ in 0..10 {
            chat.update(Action::ScrollChat(ScrollDirection::Up))
                .unwrap();
        }

        let buffer_after_scroll_up =
            draw_chat_component(&mut chat, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let scroll_up_content = extract_buffer_text(
            &buffer_after_scroll_up,
            0,
            buffer_after_scroll_up.content().len(),
        );

        // After scrolling up, should see earlier messages
        assert!(
            scroll_up_content.contains("Message 1"),
            "Should see first message after scrolling up"
        );
        // Should NOT see the last message anymore
        assert!(
            !scroll_up_content.contains("Message 30"),
            "Should not see last message after scrolling up"
        );

        // Scroll back down to see later content
        for _ in 0..15 {
            chat.update(Action::ScrollChat(ScrollDirection::Down))
                .unwrap();
        }

        let buffer_after_scroll_down =
            draw_chat_component(&mut chat, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let scroll_down_content = extract_buffer_text(
            &buffer_after_scroll_down,
            0,
            buffer_after_scroll_down.content().len(),
        );

        // After scrolling down, should see the last message again
        assert!(
            scroll_down_content.contains("Message 30"),
            "Should see last message after scrolling back down"
        );
        // Should NOT see the first message anymore
        assert!(
            !scroll_down_content.contains("Message 1"),
            "Should not see first message after scrolling back down"
        );
    }

    #[test]
    fn test_draw_page_scrolling_with_overflow() {
        let mut chat = Chat::new();

        // Add messages with distinctive content for easy identification
        let messages = vec![
            "First message - should be at top",
            "Second message",
            "Third message",
            "Fourth message",
            "Fifth message",
            "Sixth message",
            "Seventh message",
            "Eighth message",
            "Ninth message",
            "Tenth message",
            "Eleventh message",
            "Twelfth message",
            "Thirteenth message",
            "Fourteenth message",
            "Fifteenth message",
            "Sixteenth message",
            "Seventeenth message",
            "Eighteenth message",
            "Nineteenth message",
            "Last message - should be at bottom",
        ];

        for msg in &messages {
            let message = create_test_user_message(msg);
            chat.update(Action::AddChatMessage(message)).unwrap();
        }

        // Initially at bottom due to auto-scroll
        let buffer_initial = draw_chat_component(&mut chat, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let initial_content =
            extract_buffer_text(&buffer_initial, 0, buffer_initial.content().len());

        assert!(
            initial_content.contains("Last message - should be at bottom"),
            "Should see last message initially"
        );
        assert!(
            !initial_content.contains("First message - should be at top"),
            "Should not see first message initially"
        );

        // Page up to see earlier content
        chat.update(Action::ScrollChat(ScrollDirection::PageUp))
            .unwrap();

        let buffer_page_up = draw_chat_component(&mut chat, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let page_up_content =
            extract_buffer_text(&buffer_page_up, 0, buffer_page_up.content().len());

        // After page up, should see earlier content
        assert!(
            !page_up_content.contains("Last message - should be at bottom"),
            "Should not see last message after page up"
        );

        // Multiple page ups to get to the top
        for _ in 0..3 {
            chat.update(Action::ScrollChat(ScrollDirection::PageUp))
                .unwrap();
        }

        let buffer_top = draw_chat_component(&mut chat, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let top_content = extract_buffer_text(&buffer_top, 0, buffer_top.content().len());

        assert!(
            top_content.contains("First message - should be at top"),
            "Should see first message after scrolling to top"
        );
        assert!(
            !top_content.contains("Last message - should be at bottom"),
            "Should not see last message when at top"
        );

        // Page down to see later content - need more page downs to get all the way to bottom
        for _ in 0..5 {
            chat.update(Action::ScrollChat(ScrollDirection::PageDown))
                .unwrap();
        }

        let buffer_bottom = draw_chat_component(&mut chat, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let bottom_content = extract_buffer_text(&buffer_bottom, 0, buffer_bottom.content().len());

        assert!(
            bottom_content.contains("Last message - should be at bottom"),
            "Should see last message after paging back down"
        );
        assert!(
            !bottom_content.contains("First message - should be at top"),
            "Should not see first message after paging back down"
        );
    }
}

impl Component for Chat {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        use crossterm::event::KeyModifiers;

        match (key.code, key.modifiers) {
            // Ctrl+Up/Down for chat scrolling
            (KeyCode::Up, KeyModifiers::CONTROL) => {
                Ok(Some(Action::ScrollChat(ScrollDirection::Up)))
            }
            (KeyCode::Down, KeyModifiers::CONTROL) => {
                Ok(Some(Action::ScrollChat(ScrollDirection::Down)))
            }
            // Page keys always work for chat scrolling
            (KeyCode::PageUp, _) => Ok(Some(Action::ScrollChat(ScrollDirection::PageUp))),
            (KeyCode::PageDown, _) => Ok(Some(Action::ScrollChat(ScrollDirection::PageDown))),
            _ => Ok(None),
        }
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent) -> Result<Option<Action>> {
        match mouse.kind {
            MouseEventKind::ScrollUp => Ok(Some(Action::ScrollChat(ScrollDirection::Up))),
            MouseEventKind::ScrollDown => Ok(Some(Action::ScrollChat(ScrollDirection::Down))),
            _ => Ok(None),
        }
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tick => {}
            Action::Render => {}
            Action::AddChatMessage(message) => {
                self.add_message(message);
            }
            Action::ClearChat => {
                self.clear_messages();
            }
            Action::ScrollChat(direction) => {
                // Disable auto-scroll when user manually scrolls
                self.auto_scroll = false;

                match direction {
                    ScrollDirection::Up => {
                        self.scroll_state.scroll_up();
                    }
                    ScrollDirection::Down => {
                        self.scroll_state.scroll_down();
                    }
                    ScrollDirection::PageUp => {
                        for _ in 0..5 {
                            self.scroll_state.scroll_up();
                        }
                    }
                    ScrollDirection::PageDown => {
                        for _ in 0..5 {
                            self.scroll_state.scroll_down();
                        }
                    }
                }
            }
            Action::StartStreaming => {
                // Add initial streaming message
                self.add_message(ChatMessage::AssistantStreaming {
                    content: String::new(),
                    timestamp: chrono::Utc::now(),
                });
            }
            Action::StreamContent(content) => {
                // Update the last streaming message
                if let Some(ChatMessage::AssistantStreaming {
                    content: current_content,
                    timestamp: _,
                }) = self.messages.last_mut()
                {
                    current_content.push_str(&content);
                    // Enable auto-scroll for streaming content
                    self.auto_scroll = true;
                    self.content_dirty = true;
                }
            }
            Action::StreamComplete => {
                // Convert streaming message to final message
                if let Some(ChatMessage::AssistantStreaming { content, timestamp }) =
                    self.messages.last().cloned()
                {
                    if let Some(last_msg) = self.messages.last_mut() {
                        *last_msg = ChatMessage::Assistant { content, timestamp };
                        self.content_dirty = true;
                    }
                }
            }
            Action::Error(error) => {
                self.add_message(ChatMessage::Error {
                    message: error,
                    timestamp: chrono::Utc::now(),
                });
            }
            Action::StreamToolCall {
                id,
                name,
                arguments,
            } => {
                // Check if we already have a streaming tool call with this ID
                let mut found_existing = false;
                for message in self.messages.iter_mut().rev() {
                    if let ChatMessage::ToolCall {
                        id: existing_id,
                        name: _existing_name,
                        params,
                        ..
                    } = message
                    {
                        if existing_id == &id {
                            // Update the existing tool call with new arguments
                            *params = arguments.clone();
                            found_existing = true;
                            break;
                        }
                    } else {
                        // Stop looking once we hit a non-tool-call message
                        break;
                    }
                }

                // If no existing tool call found, create a new one
                if !found_existing {
                    self.add_message(ChatMessage::ToolCall {
                        id,
                        name,
                        params: arguments,
                        timestamp: chrono::Utc::now(),
                    });
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // Calculate the inner area (accounting for borders)
        let inner_area = area.inner(ratatui::layout::Margin {
            horizontal: 1,
            vertical: 1,
        });
        let content_width = inner_area.width;

        // Store values we need before borrowing mutably
        let last_height = self.last_content_height;
        let auto_scroll = self.auto_scroll;

        // Get content and calculate height
        let content = self.create_message_content().clone();
        let content_height = content.lines.len() as u16;

        // Only recreate scroll view if content height changed
        let mut scroll_view = if content_height != last_height {
            self.last_content_height = content_height;
            ScrollView::new(Size::new(content_width, content_height))
        } else {
            // Reuse existing dimensions
            ScrollView::new(Size::new(content_width, last_height))
        };

        // Handle auto-scroll logic
        if auto_scroll {
            self.scroll_state.scroll_to_bottom();
            self.auto_scroll = false;
        }

        // Create the paragraph without borders (borders are handled by the outer block)
        let paragraph = Paragraph::new(content).wrap(ratatui::widgets::Wrap { trim: false });

        // Create the outer block with borders and title
        let block = Block::default().borders(Borders::ALL).title("Chat");
        frame.render_widget(block, area);

        // Render the scrollable content in the inner area
        let content_area = Rect::new(0, 0, content_width, content_height);
        scroll_view.render_widget(paragraph, content_area);

        scroll_view.render(inner_area, frame.buffer_mut(), &mut self.scroll_state);
        Ok(())
    }
}
