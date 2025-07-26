use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

pub struct InputWidget<'a> {
    input: &'a str,
    cursor_pos: usize,
    block: Option<Block<'a>>,
}

impl<'a> InputWidget<'a> {
    pub fn new(input: &'a str, cursor_pos: usize) -> Self {
        Self {
            input,
            cursor_pos,
            block: None,
        }
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }
}

impl<'a> Widget for InputWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let content = Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Green)),
            Span::raw(self.input),
        ]);

        let mut paragraph = Paragraph::new(vec![content]);
        if let Some(block) = self.block {
            paragraph = paragraph.block(block);
        }

        paragraph.render(area, buf);
    }
}