use crate::theme::Theme;
use aether_core::types::ChatMessage;
use ratatui::{
    Frame,
    layout::{Offset, Rect},
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Widget},
};

#[derive(Debug, Clone)]
pub struct ChatWindow {
    theme: Theme,
    pub messages: Vec<ChatMessageBlock>,
    pub scroll_offset: usize,
}

impl ChatWindow {
    pub fn new() -> Self {
        Self {
            theme: Theme::default(),
            messages: vec![],
            scroll_offset: 0,
        }
    }

    pub fn add_message(&mut self, message: ChatMessageBlock) -> () {
        self.messages.push(message);
    }
}

impl ChatWindow {
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default().title("Chat");
        frame.render_widget(block, area);

        let mut y = 2;

        for message in &mut self.messages {
            let (paragraph, height) = message.draw(&area, &self.theme);

            let msg_y_signed = (area.y as i32) + (y as i32) - (self.scroll_offset as i32);

            // Skip rendering if message is completely above visible area
            if msg_y_signed + (height as i32) <= (area.y as i32) + 5 {
                y += height;
                continue;
            }

            // Skip rendering if message is completely below visible area
            if msg_y_signed >= (area.y as i32) + (area.height as i32) - 5 {
                break;
            }

            // Skip rendering if message starts above visible area (this should catch remaining edge cases)
            if msg_y_signed < (area.y as i32) {
                y += height;
                continue;
            }

            let msg_y = msg_y_signed as u16;

            let msg_rect = Rect {
                x: area.x + 1,
                y: msg_y,
                width: area.width - 2,
                height: height as u16,
            };

            frame.render_widget(paragraph, msg_rect);
            y += height;
        }
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }
}

#[derive(Debug, Clone)]
pub struct ChatMessageBlock {
    message: ChatMessage,
    height: Option<usize>,
}

impl ChatMessageBlock {
    pub fn new(message: ChatMessage) -> Self {
        Self {
            message,
            height: None,
        }
    }

    fn draw(&mut self, area: &Rect, theme: &Theme) -> (Paragraph, usize) {
        let paragraph = match &self.message {
            ChatMessage::System {
                content,
                timestamp: _,
            } => {
                let block = create_block("System", theme.system);
                Paragraph::new(content.clone()).block(block)
            }

            ChatMessage::User {
                content,
                timestamp: _,
            } => {
                let block = create_block("User", theme.user);
                Paragraph::new(content.clone()).block(block)
            }

            ChatMessage::Assistant {
                content,
                timestamp: _,
            } => {
                let block = create_block("Assistant", theme.assistant);
                Paragraph::new(content.clone()).block(block)
            }

            _ => {
                let block = create_block("Unknown", theme.system);
                Paragraph::new("Unknown").block(block)
            }
        };

        if let None = self.height {
            let height = paragraph.line_count(area.width);
            self.height = Some(height);
        }

        (paragraph, self.height.unwrap_or(0))
    }
}

fn create_block(title: &str, color: Color) -> Block {
    Block::default()
        .title(title)
        .style(Style::default().fg(color))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color))
        .border_type(BorderType::Rounded)
        .padding(Padding::new(1, 1, 1, 1))
}
