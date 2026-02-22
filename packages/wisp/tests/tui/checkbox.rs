use super::*;
use wisp::tui::{Checkbox, Component};

#[test]
fn unchecked_renders_bracket_space() {
    let mut cb = Checkbox::new(false);
    let term = render_component(&mut cb, 80, 24);
    assert_buffer_eq(&term, &["[ ]"]);
}

#[test]
fn checked_renders_bracket_x() {
    let mut cb = Checkbox::new(true);
    let term = render_component(&mut cb, 80, 24);
    assert_buffer_eq(&term, &["[x]"]);
}

#[test]
fn space_toggle_updates_render() {
    let mut cb = Checkbox::new(false);
    cb.handle_key(key(KeyCode::Char(' ')));
    let term = render_component(&mut cb, 80, 24);
    assert_buffer_eq(&term, &["[x]"]);
}

#[test]
fn double_toggle_returns_to_unchecked() {
    let mut cb = Checkbox::new(false);
    cb.handle_key(key(KeyCode::Char(' ')));
    cb.handle_key(key(KeyCode::Char(' ')));
    let term = render_component(&mut cb, 80, 24);
    assert_buffer_eq(&term, &["[ ]"]);
}

#[test]
fn non_space_key_does_not_change_render() {
    let mut cb = Checkbox::new(false);
    cb.handle_key(key(KeyCode::Enter));
    let term = render_component(&mut cb, 80, 24);
    assert_buffer_eq(&term, &["[ ]"]);
}

#[test]
fn unfocused_renders_without_highlight() {
    let mut cb = Checkbox::new(true);
    let ctx = RenderContext::new((80, 24)).with_focused(false);
    let lines = cb.render(&ctx);
    let term = render_lines(&lines, 80, 24);
    assert_buffer_eq(&term, &["[x]"]);
}
