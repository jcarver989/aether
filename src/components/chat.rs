use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::Rect,
    prelude::Size,
    widgets::{Block, Borders, Widget},
    buffer::Buffer,
};
use tokio::sync::mpsc::UnboundedSender;
use tui_scrollview::{ScrollView, ScrollViewState};

use super::{Component, block_layout::BlockLayoutManager, content_blocks::BlockRenderer, content_block::ContentBlock};

// Custom widget that renders chat blocks to a buffer for ScrollView
struct ChatContentWidget<'a> {
    content_blocks: &'a [ContentBlock],
    layout_manager: &'a BlockLayoutManager,
    block_renderer: &'a BlockRenderer,
    selected_block: Option<usize>,
    content_height: u16,
}

impl<'a> Widget for ChatContentWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Create a mock Frame that writes to our buffer
        // This is a workaround since BlockRenderer expects Frame but we need Buffer
        
        // For each layout, render the block content
        for layout in self.layout_manager.get_all_layouts() {
            if let Some(block) = self.content_blocks.get(layout.block_id) {
                let render_area = Rect {
                    x: area.x,
                    y: area.y + layout.area.y,
                    width: layout.area.width.min(area.width),
                    height: layout.area.height,
                };

                // Only render if the block is within the area
                if render_area.y < area.y + area.height && 
                   render_area.y + render_area.height > area.y {
                    
                    // We need to adapt the block rendering to work with Buffer
                    // For now, render a simplified version
                    self.render_block_to_buffer(block, render_area, buf);
                }
            }
        }
    }
}

impl<'a> ChatContentWidget<'a> {
    fn render_block_to_buffer(&self, block: &ContentBlock, area: Rect, buf: &mut Buffer) {
        use ratatui::widgets::{Paragraph, Wrap};
        use ratatui::text::{Text, Line, Span};
        use ratatui::style::{Style, Color, Modifier};

        let is_selected = self.selected_block == Some(area.y as usize); // Approximation
        
        // Create styled content based on block type
        let (title, content_text, title_style) = match block {
            crate::components::content_block::ContentBlock::SystemMessage { content, .. } => {
                ("System", content.clone(), Style::default().fg(Color::Cyan))
            }
            crate::components::content_block::ContentBlock::UserMessage { content, .. } => {
                ("User", content.clone(), Style::default().fg(Color::Green))
            }
            crate::components::content_block::ContentBlock::AssistantMessage { content, streaming, .. } => {
                let text = content.iter().map(|elem| {
                    match elem {
                        crate::components::content_block::ContentElement::Text(t) => t.clone(),
                        crate::components::content_block::ContentElement::CodeBlock { code, language, .. } => {
                            format!("```{}\n{}\n```", language, code)
                        }
                        crate::components::content_block::ContentElement::InlineCode(c) => format!("`{}`", c),
                        crate::components::content_block::ContentElement::Bold(b) => format!("**{}**", b),
                        crate::components::content_block::ContentElement::Italic(i) => format!("*{}*", i),
                        crate::components::content_block::ContentElement::Link { text, url } => format!("[{}]({})", text, url),
                    }
                }).collect::<Vec<_>>().join("");
                
                let display_text = if *streaming { 
                    format!("{} ⟨streaming⟩", text) 
                } else { 
                    text 
                };
                ("Assistant", display_text, Style::default().fg(Color::Blue))
            }
            crate::components::content_block::ContentBlock::ToolCallBlock { name, params, .. } => {
                ("Tool Call", format!("{}: {}", name, params), Style::default().fg(Color::Yellow))
            }
            crate::components::content_block::ContentBlock::ToolResultBlock { content, .. } => {
                ("Tool Result", content.clone(), Style::default().fg(Color::Magenta))
            }
            crate::components::content_block::ContentBlock::ErrorBlock { message, .. } => {
                ("Error", message.clone(), Style::default().fg(Color::Red))
            }
        };

        // Create text with styled title and content
        let mut lines = vec![
            Line::from(Span::styled(
                format!("▶ {}", title),
                if is_selected { 
                    title_style.add_modifier(Modifier::BOLD).add_modifier(Modifier::REVERSED)
                } else {
                    title_style.add_modifier(Modifier::BOLD)
                }
            ))
        ];

        // Add content lines
        for line in content_text.lines() {
            lines.push(Line::from(Span::raw(format!("  {}", line))));
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
use crate::{
    action::{Action, ScrollDirection},
    config::Config,
    theme::Theme,
    types::ChatMessage,
};

pub struct Chat {
    messages: Vec<ChatMessage>,
    content_blocks: Vec<ContentBlock>,
    layout_manager: BlockLayoutManager,
    block_renderer: BlockRenderer,
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    theme: Theme,
    auto_scroll: bool,
    scroll_offset: u16,
    content_dirty: bool,
    selected_block: Option<usize>,
    scroll_view_state: ScrollViewState,
}

impl Default for Chat {
    fn default() -> Self {
        Self::new()
    }
}

impl Chat {
    pub fn new() -> Self {
        let theme = Theme::default();
        Self {
            messages: Vec::new(),
            content_blocks: Vec::new(),
            layout_manager: BlockLayoutManager::new(),
            block_renderer: BlockRenderer::new(theme.clone()),
            command_tx: None,
            config: Config::default(),
            theme,
            auto_scroll: true,
            scroll_offset: 0,
            content_dirty: true,
            selected_block: None,
            scroll_view_state: ScrollViewState::default(),
        }
    }

    pub fn get_messages(&self) -> &Vec<ChatMessage> {
        &self.messages
    }

    pub fn get_content_blocks(&self) -> &Vec<ContentBlock> {
        &self.content_blocks
    }

    fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
        self.auto_scroll = true;
        self.content_dirty = true;
        self.rebuild_content_blocks();
    }

    fn clear_messages(&mut self) {
        self.messages.clear();
        self.content_blocks.clear();
        self.auto_scroll = true;
        self.content_dirty = true;
        self.scroll_offset = 0;
        self.selected_block = None;
    }

    fn rebuild_content_blocks(&mut self) {
        self.content_blocks.clear();
        for message in &self.messages {
            self.content_blocks.push(ContentBlock::from(message));
        }
        self.content_dirty = true;
    }

    fn scroll_up(&mut self) {
        self.scroll_view_state.scroll_up();
        self.auto_scroll = false;
    }

    fn scroll_down(&mut self) {
        self.scroll_view_state.scroll_down();
        self.auto_scroll = false;
    }

    fn page_up(&mut self) {
        self.scroll_view_state.scroll_page_up();
        self.auto_scroll = false;
    }

    fn page_down(&mut self) {
        self.scroll_view_state.scroll_page_down();
        self.auto_scroll = false;
    }

    fn update_tool_call(&mut self, id: &str, name: &str, arguments: &str) {
        // Find existing tool call message and update it
        for message in &mut self.messages {
            if let ChatMessage::ToolCall { 
                id: existing_id, 
                name: existing_name, 
                params, 
                .. 
            } = message {
                if existing_id == id {
                    *existing_name = name.to_string();
                    *params = arguments.to_string();
                    self.content_dirty = true;
                    self.rebuild_content_blocks();
                    return;
                }
            }
        }
        
        // If not found, add a new tool call message
        let tool_call = ChatMessage::ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            params: arguments.to_string(),
            timestamp: chrono::Utc::now(),
        };
        self.add_message(tool_call);
    }
}

impl Component for Chat {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        // Theme is already set in new(), keep using the default theme for now
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
            // Enter to toggle expansion of selected block
            (KeyCode::Enter, _) => {
                if let Some(selected) = self.selected_block {
                    Ok(Some(Action::ToggleBlockExpansion(selected)))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent) -> Result<Option<Action>> {
        match mouse.kind {
            MouseEventKind::ScrollUp => Ok(Some(Action::ScrollChat(ScrollDirection::Up))),
            MouseEventKind::ScrollDown => Ok(Some(Action::ScrollChat(ScrollDirection::Down))),
            MouseEventKind::Down(_) => {
                // Select block at mouse position
                let block_id = self.layout_manager.get_block_at_position(mouse.row);
                self.selected_block = block_id;
                Ok(None)
            }
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
                match direction {
                    ScrollDirection::Up => self.scroll_up(),
                    ScrollDirection::Down => self.scroll_down(),
                    ScrollDirection::PageUp => self.page_up(),
                    ScrollDirection::PageDown => self.page_down(),
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
                    self.rebuild_content_blocks();
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
                        self.rebuild_content_blocks();
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
                self.update_tool_call(&id, &name, &arguments);
            }
            Action::ToggleBlockExpansion(block_id) => {
                if let Some(block) = self.content_blocks.get_mut(block_id) {
                    block.toggle_expansion();
                    self.content_dirty = true;
                }
            }
            Action::SelectBlock(block_id) => {
                self.selected_block = Some(block_id);
            }
            Action::ToggleCodeBlockExpansion { block_id, element_id } => {
                if let Some(block) = self.content_blocks.get_mut(block_id) {
                    block.toggle_code_block_expansion(element_id);
                    self.content_dirty = true;
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // Create the outer block with borders and title
        let block = Block::default().borders(Borders::ALL).title("Chat");
        frame.render_widget(block, area);

        // Calculate the inner area (accounting for borders)
        let inner_area = area.inner(ratatui::layout::Margin {
            horizontal: 1,
            vertical: 1,
        });

        // Rebuild content blocks if dirty
        if self.content_dirty {
            self.rebuild_content_blocks();
            self.content_dirty = false;
        }

        // Calculate layouts for all blocks with a virtual area to get total content size
        let virtual_area = Rect {
            x: 0, // Use relative coordinates for ScrollView
            y: 0,
            width: inner_area.width.saturating_sub(1), // Account for scrollbar
            height: u16::MAX, // Allow unlimited height for content
        };
        
        self.layout_manager.calculate_layouts(
            &self.content_blocks,
            virtual_area,
            &self.block_renderer,
            0, // No scroll offset for layout calculation
        );

        let total_height = self.layout_manager.get_total_height();

        // Handle auto-scroll
        if self.auto_scroll {
            self.scroll_view_state.scroll_to_bottom();
            self.auto_scroll = false;
        }

        // Create ScrollView with the content size
        let scroll_size = Size::new(virtual_area.width, total_height.max(inner_area.height));
        let mut scroll_view = ScrollView::new(scroll_size);

        // Create the custom chat content widget
        let chat_content = ChatContentWidget {
            content_blocks: &self.content_blocks,
            layout_manager: &self.layout_manager,
            block_renderer: &self.block_renderer,
            selected_block: self.selected_block,
            content_height: total_height,
        };

        // Render the content into the scroll view
        scroll_view.render_widget(chat_content, scroll_view.area());

        // Render the scroll view as a stateful widget
        frame.render_stateful_widget(scroll_view, inner_area, &mut self.scroll_view_state);

        Ok(())
    }
}