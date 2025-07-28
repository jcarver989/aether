use color_eyre::Result;
use ratatui::{Frame, layout::Rect};

use super::content_block::ContentBlock;
use crate::theme::Theme;

pub struct BlockRenderer {
    #[allow(dead_code)]
    theme: Theme,
}

impl BlockRenderer {
    pub fn new(theme: Theme) -> Self {
        Self { theme }
    }

    #[allow(dead_code)]
    pub fn render_block(
        &self,
        frame: &mut Frame,
        area: Rect,
        block: &ContentBlock,
        selected: bool,
    ) -> Result<()> {
        block.render(frame, area, &self.theme, selected)
    }

    pub fn calculate_block_height(&self, block: &ContentBlock, width: u16) -> u16 {
        block.calculate_height(width)
    }
}
