use crate::Component;
use crate::components::{Event, ViewContext};
use crate::line::Line;
use crate::size::Size;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn type_query<P: Component>(picker: &mut P, text: &str) {
    for c in text.chars() {
        let _ = picker.on_event(&Event::Key(KeyEvent::new(
            KeyCode::Char(c),
            KeyModifiers::NONE,
        )));
    }
}

pub fn rendered_lines_from(lines: &[Line]) -> Vec<String> {
    lines.iter().map(Line::plain_text).collect()
}

pub fn rendered_lines_with_context(
    render: impl FnOnce(&ViewContext) -> Vec<Line>,
    size: impl Into<Size>,
) -> Vec<String> {
    let context = ViewContext::new(size);
    render(&context).iter().map(Line::plain_text).collect()
}

pub fn rendered_raw_lines_with_context(
    render: impl FnOnce(&ViewContext) -> Vec<Line>,
    size: impl Into<Size>,
) -> Vec<Line> {
    let context = ViewContext::new(size);
    render(&context)
}
