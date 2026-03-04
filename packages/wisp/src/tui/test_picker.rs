use super::component::RenderContext;
use super::screen::Line;
use super::{Component, HandlesInput};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn type_query<P: HandlesInput>(picker: &mut P, text: &str) {
    for c in text.chars() {
        picker.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
}

pub fn rendered_lines<P: Component>(picker: &mut P) -> Vec<String> {
    rendered_lines_with_size(picker, (120, 40))
}

pub fn rendered_lines_with_size<P: Component>(picker: &mut P, size: (u16, u16)) -> Vec<String> {
    let context = RenderContext::new(size);
    picker
        .render(&context)
        .iter()
        .map(Line::plain_text)
        .collect()
}

pub fn rendered_raw_lines<P: Component>(picker: &mut P) -> Vec<Line> {
    rendered_raw_lines_with_size(picker, (120, 40))
}

pub fn rendered_raw_lines_with_size<P: Component>(picker: &mut P, size: (u16, u16)) -> Vec<Line> {
    let context = RenderContext::new(size);
    picker.render(&context)
}

pub fn selected_text<P: Component>(picker: &mut P) -> Option<String> {
    rendered_lines(picker)
        .into_iter()
        .find(|l| l.starts_with("▶"))
}
