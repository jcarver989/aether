use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    action::{Action, ScrollDirection},
    config::Config,
};

use super::Component;

/// A trait for items that can be rendered in the virtual scroll component
pub trait VirtualScrollItem {
    /// Get the height this item needs when rendered at the given width
    fn height(&self, width: u16) -> u16;
    
    /// Render this item to the buffer at the given area
    fn render(&self, area: Rect, buf: &mut Buffer);
}

/// A simple, efficient virtual scrolling component
pub struct VirtualScroll<T: VirtualScrollItem> {
    items: Vec<T>,
    scroll_offset: u16,
    viewport_height: u16,
    total_content_height: u16,
    item_heights_cache: Vec<u16>,
    cache_valid: bool,
    command_tx: Option<UnboundedSender<Action>>,
}

impl<T: VirtualScrollItem> VirtualScroll<T> {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            scroll_offset: 0,
            viewport_height: 0,
            total_content_height: 0,
            item_heights_cache: Vec::new(),
            cache_valid: false,
            command_tx: None,
        }
    }

    pub fn with_items(items: Vec<T>) -> Self {
        let mut scroll = Self::new();
        scroll.set_items(items);
        scroll
    }

    pub fn set_items(&mut self, items: Vec<T>) {
        self.items = items;
        self.cache_valid = false;
        self.item_heights_cache.clear();
    }

    pub fn add_item(&mut self, item: T) {
        self.items.push(item);
        self.cache_valid = false;
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.item_heights_cache.clear();
        self.cache_valid = false;
        self.scroll_offset = 0;
        self.total_content_height = 0;
    }

    pub fn items(&self) -> &[T] {
        &self.items
    }

    pub fn items_mut(&mut self) -> &mut Vec<T> {
        self.cache_valid = false;
        &mut self.items
    }

    fn update_cache(&mut self, viewport_width: u16) {
        if self.cache_valid {
            return;
        }

        self.item_heights_cache.clear();
        self.total_content_height = 0;

        for item in &self.items {
            let height = item.height(viewport_width);
            self.item_heights_cache.push(height);
            self.total_content_height += height;
        }

        self.cache_valid = true;
    }

    fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset = self.scroll_offset.saturating_sub(1);
        }
    }

    fn scroll_down(&mut self) {
        let max_scroll = self.total_content_height.saturating_sub(self.viewport_height);
        if self.scroll_offset < max_scroll {
            self.scroll_offset += 1;
        }
    }

    fn page_up(&mut self) {
        let page_size = self.viewport_height.saturating_sub(1).max(1);
        self.scroll_offset = self.scroll_offset.saturating_sub(page_size);
    }

    fn page_down(&mut self) {
        let page_size = self.viewport_height.saturating_sub(1).max(1);
        let max_scroll = self.total_content_height.saturating_sub(self.viewport_height);
        self.scroll_offset = (self.scroll_offset + page_size).min(max_scroll);
    }

    pub fn scroll_to_bottom(&mut self) {
        let max_scroll = self.total_content_height.saturating_sub(self.viewport_height);
        self.scroll_offset = max_scroll;
    }

    /// Get the range of items that are visible in the current viewport
    fn get_visible_items(&self) -> (usize, usize, u16) {
        if self.items.is_empty() || !self.cache_valid {
            return (0, 0, 0);
        }

        let viewport_start = self.scroll_offset;
        let viewport_end = viewport_start + self.viewport_height;

        let mut current_y = 0u16;
        let mut start_idx = 0;
        let mut end_idx;
        let mut render_offset = 0u16;

        // Find the first visible item
        for (idx, &height) in self.item_heights_cache.iter().enumerate() {
            if current_y + height > viewport_start {
                start_idx = idx;
                render_offset = viewport_start.saturating_sub(current_y);
                break;
            }
            current_y += height;
        }

        // Find the last visible item
        current_y = self.item_heights_cache[..start_idx].iter().sum::<u16>();
        end_idx = start_idx;
        
        for idx in start_idx..self.items.len() {
            if current_y >= viewport_end {
                break;
            }
            current_y += self.item_heights_cache[idx];
            end_idx = idx + 1;
        }

        (start_idx, end_idx, render_offset)
    }
}

impl<T: VirtualScrollItem> Default for VirtualScroll<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: VirtualScrollItem> Component for VirtualScroll<T> {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, _config: Config) -> Result<()> {
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        match key.code {
            KeyCode::Up => Ok(Some(Action::ScrollChat(ScrollDirection::Up))),
            KeyCode::Down => Ok(Some(Action::ScrollChat(ScrollDirection::Down))),
            KeyCode::PageUp => Ok(Some(Action::ScrollChat(ScrollDirection::PageUp))),
            KeyCode::PageDown => Ok(Some(Action::ScrollChat(ScrollDirection::PageDown))),
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
            Action::ScrollChat(direction) => {
                match direction {
                    ScrollDirection::Up => self.scroll_up(),
                    ScrollDirection::Down => self.scroll_down(),
                    ScrollDirection::PageUp => self.page_up(),
                    ScrollDirection::PageDown => self.page_down(),
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        self.viewport_height = area.height;
        self.update_cache(area.width);

        let (start_idx, end_idx, _render_offset) = self.get_visible_items();

        // Calculate the virtual Y position where the first visible item should start
        let first_item_virtual_y: u16 = self.item_heights_cache[..start_idx].iter().sum();
        let viewport_start_y = self.scroll_offset;
        
        let mut current_virtual_y = first_item_virtual_y;

        // Only render visible items
        for idx in start_idx..end_idx {
            if let Some(item) = self.items.get(idx) {
                let item_height = self.item_heights_cache[idx];
                
                // Calculate where this item should appear in the viewport
                let item_start_in_viewport = current_virtual_y.saturating_sub(viewport_start_y);
                let item_end_in_viewport = (current_virtual_y + item_height).saturating_sub(viewport_start_y);
                
                // Skip items that are completely outside the viewport
                if item_start_in_viewport < area.height && item_end_in_viewport > 0 {
                    let render_area = Rect {
                        x: area.x,
                        y: area.y + item_start_in_viewport,
                        width: area.width,
                        height: item_height.min(area.height - item_start_in_viewport),
                    };

                    // Render the item directly to the frame buffer
                    item.render(render_area, frame.buffer_mut());
                }

                current_virtual_y += item_height;

                // Early exit if we're beyond the viewport
                if current_virtual_y > viewport_start_y + area.height {
                    break;
                }
            }
        }

        Ok(())
    }
}