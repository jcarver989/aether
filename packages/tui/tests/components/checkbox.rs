use super::*;
use crossterm::event::KeyCode;
use tui::Checkbox;

#[test]
fn unchecked_renders_bracket_space() {
    let cb = Checkbox::new(false);
    let term = render_component(|ctx| cb.render(ctx), 80, 24);
    assert_buffer_eq(&term, &["[ ]"]);
}

#[test]
fn checked_renders_bracket_x() {
    let cb = Checkbox::new(true);
    let term = render_component(|ctx| cb.render(ctx), 80, 24);
    assert_buffer_eq(&term, &["[x]"]);
}

#[test]
fn space_toggle_updates_render() {
    let mut cb = Checkbox::new(false);
    cb.on_event(&Event::Key(key(KeyCode::Char(' '))));
    let term = render_component(|ctx| cb.render(ctx), 80, 24);
    assert_buffer_eq(&term, &["[x]"]);
}

#[test]
fn double_toggle_returns_to_unchecked() {
    let mut cb = Checkbox::new(false);
    cb.on_event(&Event::Key(key(KeyCode::Char(' '))));
    cb.on_event(&Event::Key(key(KeyCode::Char(' '))));
    let term = render_component(|ctx| cb.render(ctx), 80, 24);
    assert_buffer_eq(&term, &["[ ]"]);
}

#[test]
fn non_space_key_does_not_change_render() {
    let mut cb = Checkbox::new(false);
    cb.on_event(&Event::Key(key(KeyCode::Enter)));
    let term = render_component(|ctx| cb.render(ctx), 80, 24);
    assert_buffer_eq(&term, &["[ ]"]);
}

#[test]
fn unfocused_renders_without_highlight() {
    let cb = Checkbox::new(true);
    let ctx = ViewContext::new((80, 24));
    let lines = cb.render_field(&ctx, false);
    let term = render_lines(&lines, 80, 24);
    assert_buffer_eq(&term, &["[x]"]);
}
