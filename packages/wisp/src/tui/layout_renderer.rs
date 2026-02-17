use super::RenderContext;
use super::frame_renderer::FrameRenderer;
use super::screen::{Line, Screen};
use super::soft_wrap::soft_wrap_lines_with_map;
use std::io::{self, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutCursor {
    pub logical_row: usize,
    pub col: usize,
}

pub struct ScreenLayout {
    pub logical_lines: Vec<Line>,
    pub cursor: LayoutCursor,
}

pub struct LayoutRenderer<T: Write> {
    tui: FrameRenderer<T>,
}

impl<T: Write> LayoutRenderer<T> {
    pub fn new(writer: T) -> Self {
        Self {
            tui: FrameRenderer::new(writer),
        }
    }

    pub fn render(&mut self, layout: &ScreenLayout) -> io::Result<()> {
        let context = self.tui.context();
        let (visual_lines, logical_to_visual) =
            soft_wrap_lines_with_map(&layout.logical_lines, context.size.0);

        let mut cursor_row = logical_to_visual
            .get(layout.cursor.logical_row)
            .copied()
            .unwrap_or_else(|| visual_lines.len().saturating_sub(1));
        let width = context.size.0 as usize;
        let mut cursor_col = layout.cursor.col;
        if width > 0 {
            cursor_row += cursor_col / width;
            cursor_col %= width;
        } else {
            cursor_col = 0;
        }
        if cursor_row >= visual_lines.len() {
            cursor_row = visual_lines.len().saturating_sub(1);
        }

        self.tui.render_lines(&visual_lines)?;
        let rows_up = visual_lines
            .len()
            .saturating_sub(1)
            .saturating_sub(cursor_row) as u16;
        self.tui.reposition_cursor(rows_up, cursor_col as u16)?;
        Ok(())
    }

    pub fn push_to_scrollback(&mut self, lines: &[Line]) -> io::Result<()> {
        self.tui.push_to_scrollback(lines)
    }

    pub fn context(&self) -> &RenderContext {
        self.tui.context()
    }

    pub fn update_render_context(&mut self) {
        self.tui.update_context_from_terminal();
    }

    pub fn update_render_context_with(&mut self, size: (u16, u16)) {
        self.tui.update_context(size);
    }

    #[allow(dead_code)]
    pub fn writer(&self) -> &T {
        self.tui.writer()
    }

    #[allow(dead_code)]
    pub fn writer_mut(&mut self) -> &mut T {
        self.tui.writer_mut()
    }

    #[allow(dead_code)]
    pub fn screen(&self) -> &Screen {
        self.tui.screen()
    }
}
