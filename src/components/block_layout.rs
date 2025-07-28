use ratatui::layout::Rect;
use super::content_block::ContentBlock;
use crate::components::content_blocks::BlockRenderer;

#[derive(Debug, Clone)]
pub struct BlockLayout {
    pub block_id: usize,
    pub area: Rect,
    pub visible: bool,
}

pub struct BlockLayoutManager {
    layouts: Vec<BlockLayout>,
    viewport_height: u16,
    scroll_offset: u16,
}

impl BlockLayoutManager {
    pub fn new() -> Self {
        Self {
            layouts: Vec::new(),
            viewport_height: 0,
            scroll_offset: 0,
        }
    }

    pub fn calculate_layouts(
        &mut self,
        blocks: &[ContentBlock],
        viewport_area: Rect,
        renderer: &BlockRenderer,
        scroll_offset: u16,
    ) {
        self.layouts.clear();
        self.viewport_height = viewport_area.height;
        self.scroll_offset = scroll_offset;

        let mut current_y = viewport_area.y;
        let width = viewport_area.width;

        for (i, block) in blocks.iter().enumerate() {
            let block_height = renderer.calculate_block_height(block, width);
            
            // Add spacing between blocks
            let spacing = if i > 0 { 1 } else { 0 };
            current_y += spacing;

            let block_area = Rect {
                x: viewport_area.x,
                y: current_y,
                width,
                height: block_height,
            };

            // Determine visibility based on scroll offset and viewport
            let visible_start = viewport_area.y + scroll_offset;
            let visible_end = visible_start + viewport_area.height;
            let block_start = current_y;
            let block_end = current_y + block_height;

            let visible = !(block_end < visible_start || block_start > visible_end);

            self.layouts.push(BlockLayout {
                block_id: i,
                area: block_area,
                visible,
            });

            current_y += block_height;
        }
    }

    pub fn get_visible_layouts(&self) -> impl Iterator<Item = &BlockLayout> {
        self.layouts.iter().filter(|layout| layout.visible)
    }

    pub fn get_total_height(&self) -> u16 {
        self.layouts.last()
            .map(|layout| layout.area.y + layout.area.height)
            .unwrap_or(0)
    }

    pub fn scroll_to_offset(&mut self, offset: u16) {
        self.scroll_offset = offset;
        
        // Recalculate visibility for existing layouts
        let visible_start = self.scroll_offset;
        let visible_end = visible_start + self.viewport_height;
        
        for layout in &mut self.layouts {
            let block_start = layout.area.y;
            let block_end = layout.area.y + layout.area.height;
            layout.visible = !(block_end < visible_start || block_start > visible_end);
        }
    }

    pub fn get_block_at_position(&self, y: u16) -> Option<usize> {
        for layout in &self.layouts {
            if y >= layout.area.y && y < layout.area.y + layout.area.height {
                return Some(layout.block_id);
            }
        }
        None
    }

    pub fn get_layout_for_block(&self, block_id: usize) -> Option<&BlockLayout> {
        self.layouts.iter().find(|layout| layout.block_id == block_id)
    }
}

impl Default for BlockLayoutManager {
    fn default() -> Self {
        Self::new()
    }
}