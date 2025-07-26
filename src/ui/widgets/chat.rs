use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Widget},
};

pub struct ChatWidget<'a> {
    messages: &'a [String],
    block: Option<Block<'a>>,
}

impl<'a> ChatWidget<'a> {
    pub fn new(messages: &'a [String]) -> Self {
        Self {
            messages,
            block: None,
        }
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }
}

impl<'a> Widget for ChatWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let items: Vec<ListItem> = self.messages
            .iter()
            .map(|msg| ListItem::new(Line::from(msg.as_str())))
            .collect();

        let mut list = List::new(items);
        if let Some(block) = self.block {
            list = list.block(block);
        }

        list.render(area, buf);
    }
}