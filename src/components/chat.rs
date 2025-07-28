use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    prelude::Size,
    widgets::{Block, Borders, Widget},
};
use tokio::sync::mpsc::UnboundedSender;
use tui_scrollview::{ScrollView, ScrollViewState};

use super::{
    Component, block_layout::BlockLayoutManager, content_block::ContentBlock,
    content_blocks::BlockRenderer,
};

// Custom widget that renders chat blocks to a buffer for ScrollView
struct ChatContentWidget<'a> {
    content_blocks: &'a [ContentBlock],
    layout_manager: &'a BlockLayoutManager,
    #[allow(dead_code)]
    block_renderer: &'a BlockRenderer,
    selected_block: Option<usize>,
    #[allow(dead_code)]
    content_height: u16,
}

impl<'a> Widget for ChatContentWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Virtual rendering: only render blocks that intersect with the visible area
        // Get the scroll position to determine what's visible
        let visible_start = area.y;
        let visible_end = area.y + area.height;

        // Only iterate through layouts that could be visible
        for layout in self.layout_manager.get_visible_layouts(visible_start, visible_end) {
            if let Some(block) = self.content_blocks.get(layout.block_id) {
                let render_area = Rect {
                    x: area.x,
                    y: layout.area.y, // Use absolute coordinates from layout
                    width: layout.area.width.min(area.width),
                    height: layout.area.height,
                };

                // Double-check intersection (layout manager should handle this)
                if render_area.y < visible_end && render_area.y + render_area.height > visible_start {
                    self.render_block_to_buffer(block, render_area, buf);
                }
            }
        }
    }
}

impl<'a> ChatContentWidget<'a> {
    fn render_block_to_buffer(&self, block: &ContentBlock, area: Rect, buf: &mut Buffer) {
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::text::{Line, Span, Text};
        use ratatui::widgets::{Paragraph, Wrap};

        let is_selected = self.selected_block == Some(area.y as usize); // Approximation

        // Create text with styled title and content - build lines directly to avoid cloning
        let mut lines = Vec::new();
        
        let (title, title_style) = match block {
            crate::components::content_block::ContentBlock::SystemMessage { .. } => {
                ("System", Style::default().fg(Color::Cyan))
            }
            crate::components::content_block::ContentBlock::UserMessage { .. } => {
                ("User", Style::default().fg(Color::Green))
            }
            crate::components::content_block::ContentBlock::AssistantMessage { .. } => {
                ("Assistant", Style::default().fg(Color::Blue))
            }
            crate::components::content_block::ContentBlock::ToolCallBlock { .. } => {
                ("Tool Call", Style::default().fg(Color::Yellow))
            }
            crate::components::content_block::ContentBlock::ToolResultBlock { .. } => {
                ("Tool Result", Style::default().fg(Color::Magenta))
            }
            crate::components::content_block::ContentBlock::ErrorBlock { .. } => {
                ("Error", Style::default().fg(Color::Red))
            }
        };

        // Add title line
        lines.push(Line::from(Span::styled(
            format!("▶ {title}"),
            if is_selected {
                title_style
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::REVERSED)
            } else {
                title_style.add_modifier(Modifier::BOLD)
            },
        )));

        // Add content lines based on block type
        match block {
            crate::components::content_block::ContentBlock::SystemMessage { content, .. } => {
                for line in content.lines() {
                    lines.push(Line::from(Span::raw(format!("  {line}"))));
                }
            }
            crate::components::content_block::ContentBlock::UserMessage { content, .. } => {
                for line in content.lines() {
                    lines.push(Line::from(Span::raw(format!("  {line}"))));
                }
            }
            crate::components::content_block::ContentBlock::AssistantMessage {
                display_text,
                streaming,
                ..
            } => {
                if *streaming {
                    for line in display_text.lines() {
                        lines.push(Line::from(Span::raw(format!("  {line}"))));
                    }
                    lines.push(Line::from(Span::raw("  ⟨streaming⟩")));
                } else {
                    for line in display_text.lines() {
                        lines.push(Line::from(Span::raw(format!("  {line}"))));
                    }
                }
            }
            crate::components::content_block::ContentBlock::ToolCallBlock {
                name, params, ..
            } => {
                lines.push(Line::from(Span::raw(format!("  {name}: {params}"))));
            }
            crate::components::content_block::ContentBlock::ToolResultBlock { content, .. } => {
                for line in content.lines() {
                    lines.push(Line::from(Span::raw(format!("  {line}"))));
                }
            }
            crate::components::content_block::ContentBlock::ErrorBlock { message, .. } => {
                for line in message.lines() {
                    lines.push(Line::from(Span::raw(format!("  {line}"))));
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
use std::sync::Arc;
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
    config: Arc<Config>,
    #[allow(dead_code)]
    theme: Theme,
    auto_scroll: bool,
    scroll_offset: u16,
    content_dirty: bool,
    selected_block: Option<usize>,
    scroll_view_state: ScrollViewState,
    // Layout caching
    layout_dirty: bool,
    cached_area: Option<Rect>,
    cached_total_height: u16,
    // ScrollView caching
    cached_scroll_view: Option<ScrollView>,
    cached_scroll_size: Option<Size>,
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
            config: Arc::new(Config::default()),
            theme,
            auto_scroll: true,
            scroll_offset: 0,
            content_dirty: true,
            selected_block: None,
            scroll_view_state: ScrollViewState::default(),
            // Layout caching
            layout_dirty: true,
            cached_area: None,
            cached_total_height: 0,
            // ScrollView caching
            cached_scroll_view: None,
            cached_scroll_size: None,
        }
    }

    #[allow(dead_code)]
    pub fn get_messages(&self) -> &Vec<ChatMessage> {
        &self.messages
    }

    #[allow(dead_code)]
    pub fn get_content_blocks(&mut self) -> &Vec<ContentBlock> {
        // Ensure content blocks are up to date
        if self.content_dirty {
            if self.messages.len() > self.content_blocks.len() + 5 {
                self.rebuild_content_blocks();
            } else {
                self.rebuild_content_blocks_incremental();
            }
            self.content_dirty = false;
        }
        &self.content_blocks
    }

    fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
        self.auto_scroll = true;
        self.content_dirty = true;
        self.layout_dirty = true;
        // Don't rebuild immediately - let draw() handle it when needed
    }

    #[allow(dead_code)]
    fn add_message_optimized(&mut self, message: ChatMessage) {
        // For streaming content, avoid marking everything dirty if we're just appending
        let is_streaming_update = matches!(message, ChatMessage::AssistantStreaming { .. });
        
        self.messages.push(message);
        self.auto_scroll = true;
        
        if is_streaming_update && !self.content_blocks.is_empty() {
            // Just add the new content block without full rebuild
            if let Some(last_message) = self.messages.last() {
                self.content_blocks.push(ContentBlock::from(last_message));
                // Only mark layout dirty for the new block
                self.layout_dirty = true;
            }
        } else {
            self.content_dirty = true;
            self.layout_dirty = true;
        }
    }

    fn clear_messages(&mut self) {
        self.messages.clear();
        self.content_blocks.clear();
        self.auto_scroll = true;
        self.content_dirty = true;
        self.layout_dirty = true;
        self.scroll_offset = 0;
        self.selected_block = None;
    }

    fn rebuild_content_blocks(&mut self) {
        self.content_blocks.clear();
        for message in &self.messages {
            self.content_blocks.push(ContentBlock::from(message));
        }
        // Mark layout as dirty since content changed
        self.layout_dirty = true;
        // Don't set content_dirty here - it should already be set by the caller
        // This prevents infinite rebuilding loops
    }

    fn rebuild_content_blocks_incremental(&mut self) {
        let messages_len = self.messages.len();
        let blocks_len = self.content_blocks.len();
        
        if blocks_len < messages_len {
            // Add missing content blocks
            for message in &self.messages[blocks_len..] {
                self.content_blocks.push(ContentBlock::from(message));
            }
            self.layout_dirty = true;
        } else if blocks_len > messages_len {
            // Remove excess content blocks (shouldn't happen but be safe)
            self.content_blocks.truncate(messages_len);
            self.layout_dirty = true;
        } else {
            // Same length - need to check if any existing blocks need updating
            // This is critical for tool calls that get updated in place
            let mut needs_update = false;
            for (message, block) in self.messages.iter().zip(self.content_blocks.iter_mut()) {
                let new_block = ContentBlock::from(message);
                
                // Compare actual content to detect updates (critical for streaming and tool calls)
                let block_changed = match (&*block, &new_block) {
                    (
                        crate::components::content_block::ContentBlock::ToolCallBlock { name: old_name, params: old_params, .. },
                        crate::components::content_block::ContentBlock::ToolCallBlock { name: new_name, params: new_params, .. }
                    ) => old_name != new_name || old_params != new_params,
                    (
                        crate::components::content_block::ContentBlock::AssistantMessage { display_text: old_text, streaming: old_streaming, .. },
                        crate::components::content_block::ContentBlock::AssistantMessage { display_text: new_text, streaming: new_streaming, .. }
                    ) => old_text != new_text || old_streaming != new_streaming,
                    _ => std::mem::discriminant(&*block) != std::mem::discriminant(&new_block)
                };
                
                if block_changed {
                    *block = new_block;
                    needs_update = true;
                }
            }
            
            if needs_update {
                self.layout_dirty = true;
            }
        }
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
            } = message
            {
                if existing_id == id {
                    *existing_name = name.to_string();
                    *params = arguments.to_string();
                    self.content_dirty = true;
                    self.layout_dirty = true;
                    // Let draw() handle rebuild when needed
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

    fn register_config_handler(&mut self, config: Arc<Config>) -> Result<()> {
        self.config = config;
        // Theme is already set in new(), keep using the default theme for now
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        use crossterm::event::KeyModifiers;

        match (key.code, key.modifiers) {
            // Regular arrow keys for chat scrolling
            (KeyCode::Up, KeyModifiers::NONE) => Ok(Some(Action::ScrollChat(ScrollDirection::Up))),
            (KeyCode::Down, KeyModifiers::NONE) => {
                Ok(Some(Action::ScrollChat(ScrollDirection::Down)))
            }
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
            Action::ScrollChat(direction) => match direction {
                ScrollDirection::Up => self.scroll_up(),
                ScrollDirection::Down => self.scroll_down(),
                ScrollDirection::PageUp => self.page_up(),
                ScrollDirection::PageDown => self.page_down(),
            },
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
                    self.layout_dirty = true;
                    // Let draw() handle rebuild when needed
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
                        self.layout_dirty = true;
                        // Let draw() handle rebuild when needed
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
                self.update_tool_call(&id, &name, &arguments);
            }
            Action::ToggleBlockExpansion(block_id) => {
                if let Some(block) = self.content_blocks.get_mut(block_id) {
                    block.toggle_expansion();
                    self.content_dirty = true;
                    self.layout_dirty = true;
                }
            }
            Action::SelectBlock(block_id) => {
                self.selected_block = Some(block_id);
            }
            Action::ToggleCodeBlockExpansion {
                block_id,
                element_id,
            } => {
                if let Some(block) = self.content_blocks.get_mut(block_id) {
                    block.toggle_code_block_expansion(element_id);
                    self.content_dirty = true;
                    self.layout_dirty = true;
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

        // Rebuild content blocks if dirty - use incremental approach when possible
        if self.content_dirty {
            if self.messages.len() > self.content_blocks.len() + 5 {
                // Full rebuild if we're way out of sync
                self.rebuild_content_blocks();
            } else {
                // Incremental rebuild for better performance
                self.rebuild_content_blocks_incremental();
            }
            self.content_dirty = false;
        }

        // Calculate layouts for all blocks with a virtual area to get total content size
        let virtual_area = Rect {
            x: 0, // Use relative coordinates for ScrollView
            y: 0,
            width: inner_area.width.saturating_sub(1), // Account for scrollbar
            height: u16::MAX,                          // Allow unlimited height for content
        };

        // Only recalculate layout if content changed or area changed
        let area_changed = self.cached_area != Some(virtual_area);
        if self.layout_dirty || area_changed {
            self.layout_manager.calculate_layouts(
                &self.content_blocks,
                virtual_area,
                &self.block_renderer,
                0, // No scroll offset for layout calculation
            );
            
            self.cached_total_height = self.layout_manager.get_total_height();
            self.cached_area = Some(virtual_area);
            self.layout_dirty = false;
        }

        let total_height = self.cached_total_height;

        // Handle auto-scroll
        if self.auto_scroll {
            self.scroll_view_state.scroll_to_bottom();
            self.auto_scroll = false;
        }

        // Create or reuse ScrollView with the content size
        let scroll_size = Size::new(virtual_area.width, total_height.max(inner_area.height));
        let mut scroll_view = if let (Some(cached_view), Some(cached_size)) = 
            (&self.cached_scroll_view, &self.cached_scroll_size) 
        {
            if *cached_size == scroll_size {
                // Reuse existing ScrollView
                cached_view.clone()
            } else {
                // Size changed, create new one
                ScrollView::new(scroll_size)
            }
        } else {
            // First time, create new one
            ScrollView::new(scroll_size)
        };

        // Create the custom chat content widget
        let chat_content = ChatContentWidget {
            content_blocks: &self.content_blocks,
            layout_manager: &self.layout_manager,
            block_renderer: &self.block_renderer,
            selected_block: self.selected_block,
            content_height: total_height,
        };

        // Cache the ScrollView for reuse
        self.cached_scroll_view = Some(scroll_view.clone());
        self.cached_scroll_size = Some(scroll_size);

        // Render the content into the scroll view
        scroll_view.render_widget(chat_content, scroll_view.area());

        // Render the scroll view as a stateful widget
        frame.render_stateful_widget(scroll_view, inner_area, &mut self.scroll_view_state);

        Ok(())
    }
}
