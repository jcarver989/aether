use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders},
};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, block_layout::BlockLayoutManager, content_blocks::BlockRenderer, content_block::ContentBlock};
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
        if self.scroll_offset > 0 {
            self.scroll_offset = self.scroll_offset.saturating_sub(3);
            self.auto_scroll = false;
        }
    }

    fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(3);
        // Check if we've scrolled to bottom
        let total_height = self.layout_manager.get_total_height();
        if self.scroll_offset >= total_height {
            self.auto_scroll = true;
        }
    }

    fn page_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(15);
        self.auto_scroll = false;
    }

    fn page_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(15);
        let total_height = self.layout_manager.get_total_height();
        if self.scroll_offset >= total_height {
            self.auto_scroll = true;
        }
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

        // Calculate layouts for all blocks
        self.layout_manager.calculate_layouts(
            &self.content_blocks,
            inner_area,
            &self.block_renderer,
            self.scroll_offset,
        );

        // Handle auto-scroll
        if self.auto_scroll {
            let total_height = self.layout_manager.get_total_height();
            if total_height > inner_area.height {
                self.scroll_offset = total_height - inner_area.height;
                self.layout_manager.scroll_to_offset(self.scroll_offset);
            }
            self.auto_scroll = false;
        }

        // Render visible blocks
        for layout in self.layout_manager.get_visible_layouts() {
            if let Some(block) = self.content_blocks.get(layout.block_id) {
                // Adjust the area for scrolling
                let render_area = Rect {
                    x: layout.area.x,
                    y: layout.area.y.saturating_sub(self.scroll_offset) + inner_area.y,
                    width: layout.area.width,
                    height: layout.area.height,
                };

                // Only render if the block is visible in the viewport
                if render_area.y < inner_area.y + inner_area.height 
                    && render_area.y + render_area.height > inner_area.y {
                    let is_selected = self.selected_block == Some(layout.block_id);
                    self.block_renderer.render_block(frame, render_area, block, is_selected)?;
                }
            }
        }

        Ok(())
    }
}