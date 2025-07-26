use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::llm::ToolCall;

pub struct ToolCallWidget<'a> {
    tool_call: &'a ToolCall,
    block: Option<Block<'a>>,
}

impl<'a> ToolCallWidget<'a> {
    pub fn new(tool_call: &'a ToolCall) -> Self {
        Self {
            tool_call,
            block: None,
        }
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }
}

impl<'a> Widget for ToolCallWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let content = vec![
            Line::from(vec![
                Span::styled("Tool: ", Style::default().fg(Color::Yellow)),
                Span::raw(&self.tool_call.name),
            ]),
            Line::from(vec![
                Span::styled("Args: ", Style::default().fg(Color::Yellow)),
                Span::raw(serde_json::to_string_pretty(&self.tool_call.arguments).unwrap_or_default()),
            ]),
        ];

        let mut paragraph = Paragraph::new(content);
        if let Some(block) = self.block {
            paragraph = paragraph.block(block);
        }

        paragraph.render(area, buf);
    }
}