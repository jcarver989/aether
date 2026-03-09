use crate::Component;
use crate::component::{InteractiveComponent, RenderContext, UiEvent};
use crate::line::Line;
use crate::size::Size;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn type_query<P: InteractiveComponent>(picker: &mut P, text: &str) {
    for c in text.chars() {
        let _ = picker.on_event(UiEvent::Key(KeyEvent::new(
            KeyCode::Char(c),
            KeyModifiers::NONE,
        )));
    }
}

pub fn rendered_lines<P: Component>(picker: &P) -> Vec<String> {
    rendered_lines_with_size(picker, (120, 40))
}

pub fn rendered_lines_with_size<P: Component>(picker: &P, size: impl Into<Size>) -> Vec<String> {
    let context = RenderContext::new(size);
    picker
        .render(&context)
        .iter()
        .map(Line::plain_text)
        .collect()
}

pub fn rendered_raw_lines<P: Component>(picker: &P) -> Vec<Line> {
    rendered_raw_lines_with_size(picker, (120, 40))
}

pub fn rendered_raw_lines_with_size<P: Component>(picker: &P, size: impl Into<Size>) -> Vec<Line> {
    let context = RenderContext::new(size);
    picker.render(&context)
}

pub fn selected_text<P: Component>(picker: &P) -> Option<String> {
    rendered_lines(picker)
        .into_iter()
        .find(|l| l.starts_with("▶"))
}
