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
    let context = RenderContext::new((120, 40));
    picker
        .render(&context)
        .iter()
        .map(Line::plain_text)
        .collect()
}

pub fn selected_text<P: Component>(picker: &mut P) -> Option<String> {
    rendered_lines(picker)
        .into_iter()
        .find(|l| l.starts_with("▶"))
}
