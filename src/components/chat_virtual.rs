use color_eyre::Result;
use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap, Widget},
};
use tokio::sync::mpsc::UnboundedSender;

use super::{
    Component,
    content_block::ContentBlock,
    virtual_scroll::{VirtualScroll, VirtualScrollItem},
};
use crate::{
    action::Action,
    config::Config,
    types::ChatMessage,
};

/// Adapter to make ContentBlock work with VirtualScrollItem
pub struct ContentBlockItem {
    pub block: ContentBlock,
    pub selected: bool,
}

impl VirtualScrollItem for ContentBlockItem {
    fn height(&self, width: u16) -> u16 {
        // Calculate the height needed for this content block
        let content_lines = match &self.block {
            ContentBlock::SystemMessage { content, .. } => content.lines().count(),
            ContentBlock::UserMessage { content, .. } => content.lines().count(),
            ContentBlock::AssistantMessage { display_text, .. } => {
                display_text.lines().count() + if matches!(self.block, ContentBlock::AssistantMessage { streaming: true, .. }) { 1 } else { 0 }
            },
            ContentBlock::ToolCallBlock { name, params, .. } => {
                1 + params.lines().count() // name + params
            },
            ContentBlock::ToolResultBlock { content, .. } => content.lines().count(),
            ContentBlock::ErrorBlock { message, .. } => message.lines().count(),
        };

        // Add title line + content lines + separator + borders
        (content_lines + 3).min(u16::MAX as usize) as u16
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        let is_selected = self.selected;
        
        // Create text with styled title and content
        let mut lines = Vec::new();
        
        let (title, title_style) = match &self.block {
            ContentBlock::SystemMessage { .. } => {
                ("System", Style::default().fg(Color::Cyan))
            }
            ContentBlock::UserMessage { .. } => {
                ("User", Style::default().fg(Color::Green))
            }
            ContentBlock::AssistantMessage { .. } => {
                ("Assistant", Style::default().fg(Color::Blue))
            }
            ContentBlock::ToolCallBlock { .. } => {
                ("Tool Call", Style::default().fg(Color::Yellow))
            }
            ContentBlock::ToolResultBlock { .. } => {
                ("Tool Result", Style::default().fg(Color::Magenta))
            }
            ContentBlock::ErrorBlock { .. } => {
                ("Error", Style::default().fg(Color::Red))
            }
        };

        // Add title line
        lines.push(Line::from(Span::styled(
            format!("▶ {}", title),
            if is_selected {
                title_style
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::REVERSED)
            } else {
                title_style.add_modifier(Modifier::BOLD)
            },
        )));

        // Add content lines based on block type
        match &self.block {
            ContentBlock::SystemMessage { content, .. } => {
                for line in content.lines() {
                    lines.push(Line::from(Span::raw(format!("  {}", line))));
                }
            }
            ContentBlock::UserMessage { content, .. } => {
                for line in content.lines() {
                    lines.push(Line::from(Span::raw(format!("  {}", line))));
                }
            }
            ContentBlock::AssistantMessage {
                display_text,
                streaming,
                ..
            } => {
                for line in display_text.lines() {
                    lines.push(Line::from(Span::raw(format!("  {}", line))));
                }
                if *streaming {
                    lines.push(Line::from(Span::raw("  ⟨streaming⟩")));
                }
            }
            ContentBlock::ToolCallBlock { name, params, .. } => {
                lines.push(Line::from(Span::raw(format!("  {}: {}", name, params))));
            }
            ContentBlock::ToolResultBlock { content, .. } => {
                for line in content.lines() {
                    lines.push(Line::from(Span::raw(format!("  {}", line))));
                }
            }
            ContentBlock::ErrorBlock { message, .. } => {
                for line in message.lines() {
                    lines.push(Line::from(Span::raw(format!("  {}", line))));
                }
            }
        }

        // Add a separator line
        lines.push(Line::from(""));

        let text = Text::from(lines);

        // Create a bordered paragraph for better visual separation
        let block_widget = ratatui::widgets::Block::default()
            .borders(ratatui::widgets::Borders::LEFT)
            .border_style(if is_selected {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::DarkGray)
            });

        let paragraph = Paragraph::new(text)
            .block(block_widget)
            .wrap(Wrap { trim: false });

        paragraph.render(area, buf);
    }
}

/// A virtual scrolling chat component that uses the VirtualScroll component
pub struct ChatVirtual {
    scroll: VirtualScroll<ContentBlockItem>,
    messages: Vec<ChatMessage>,
    content_dirty: bool,
    selected_block: Option<usize>,
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
}

impl Default for ChatVirtual {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatVirtual {
    pub fn new() -> Self {
        Self {
            scroll: VirtualScroll::new(),
            messages: Vec::new(),
            content_dirty: true,
            selected_block: None,
            command_tx: None,
            config: Config::default(),
        }
    }

    fn rebuild_content_blocks(&mut self) {
        let blocks: Vec<ContentBlockItem> = self.messages
            .iter()
            .enumerate()
            .map(|(idx, message)| ContentBlockItem {
                block: ContentBlock::from(message),
                selected: self.selected_block == Some(idx),
            })
            .collect();

        // Update the virtual scroll with new items
        *self.scroll.items_mut() = blocks;
        self.content_dirty = false;
    }

    /// Optimized rebuild that only updates the last message (for streaming)
    fn rebuild_last_content_block(&mut self) {
        if let Some(last_message) = self.messages.last() {
            let last_idx = self.messages.len() - 1;
            let new_block = ContentBlockItem {
                block: ContentBlock::from(last_message),
                selected: self.selected_block == Some(last_idx),
            };

            // Only update the last item in the virtual scroll
            let items = self.scroll.items_mut();
            let len = items.len();
            if len > 0 {
                items[len - 1] = new_block;
            } else {
                items.push(new_block);
            }
        }
        self.content_dirty = false;
    }

    fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
        self.content_dirty = true;
        // Auto-scroll to bottom when new messages arrive
        self.scroll.scroll_to_bottom();
    }

    fn clear_messages(&mut self) {
        self.messages.clear();
        self.scroll.clear();
        self.content_dirty = false;
        self.selected_block = None;
    }
}

impl Component for ChatVirtual {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx.clone());
        self.scroll.register_action_handler(tx)
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config.clone();
        self.scroll.register_config_handler(config)
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        // Delegate scrolling to the virtual scroll component
        self.scroll.handle_key_event(key)
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent) -> Result<Option<Action>> {
        // Delegate mouse events to the virtual scroll component
        self.scroll.handle_mouse_event(mouse)
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::AddChatMessage(message) => {
                self.add_message(message);
            }
            Action::ClearChat => {
                self.clear_messages();
            }
            Action::StartStreaming => {
                self.add_message(ChatMessage::AssistantStreaming {
                    content: String::new(),
                    timestamp: chrono::Utc::now(),
                });
            }
            Action::StreamContent(content) => {
                if let Some(ChatMessage::AssistantStreaming {
                    content: current_content,
                    ..
                }) = self.messages.last_mut()
                {
                    current_content.push_str(&content);
                    self.content_dirty = true;
                    // Use optimized rebuild for streaming content to avoid performance issues
                    self.rebuild_last_content_block();
                    self.scroll.scroll_to_bottom();
                }
            }
            Action::StreamComplete => {
                if let Some(ChatMessage::AssistantStreaming { content, timestamp }) =
                    self.messages.last().cloned()
                {
                    if let Some(last_msg) = self.messages.last_mut() {
                        *last_msg = ChatMessage::Assistant { content, timestamp };
                        self.content_dirty = true;
                        // Use optimized rebuild to update the final message state
                        self.rebuild_last_content_block();
                    }
                }
            }
            Action::Error(error) => {
                self.add_message(ChatMessage::Error {
                    message: error,
                    timestamp: chrono::Utc::now(),
                });
            }
            Action::StreamToolCall { id, name, arguments } => {
                // Find existing tool call or create new one
                let mut found = false;
                for message in &mut self.messages {
                    if let ChatMessage::ToolCall {
                        id: existing_id,
                        name: existing_name,
                        params,
                        ..
                    } = message
                    {
                        if existing_id == &id {
                            *existing_name = name.clone();
                            *params = arguments.clone();
                            self.content_dirty = true;
                            // Force rebuild for tool call updates
                            self.rebuild_content_blocks();
                            found = true;
                            break;
                        }
                    }
                }
                
                if !found {
                    self.add_message(ChatMessage::ToolCall {
                        id,
                        name,
                        params: arguments,
                        timestamp: chrono::Utc::now(),
                    });
                }
            }
            _ => {
                // Forward other actions to the virtual scroll component
                return self.scroll.update(action);
            }
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // Create the outer block with borders and title
        let block = Block::default().borders(Borders::ALL).title("Chat (Virtual)");
        frame.render_widget(block, area);

        // Calculate the inner area (accounting for borders)
        let inner_area = area.inner(ratatui::layout::Margin {
            horizontal: 1,
            vertical: 1,
        });

        // Rebuild content blocks if needed
        if self.content_dirty {
            self.rebuild_content_blocks();
        }

        // Delegate rendering to the virtual scroll component
        self.scroll.draw(frame, inner_area)
    }
}