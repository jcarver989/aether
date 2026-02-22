use super::*;
use wisp::tui::{Component, NumberField};

#[test]
fn empty_renders_cursor() {
    let mut nf = NumberField::new(String::new(), false);
    let term = render_component(&mut nf, 80, 24);
    assert_buffer_eq(&term, &["▏"]);
}

#[test]
fn integer_input_renders() {
    let mut nf = NumberField::new(String::new(), true);
    nf.handle_key(key(KeyCode::Char('-')));
    nf.handle_key(key(KeyCode::Char('4')));
    nf.handle_key(key(KeyCode::Char('2')));
    let term = render_component(&mut nf, 80, 24);
    assert_buffer_eq(&term, &["-42▏"]);
}

#[test]
fn float_input_renders() {
    let mut nf = NumberField::new(String::new(), false);
    nf.handle_key(key(KeyCode::Char('3')));
    nf.handle_key(key(KeyCode::Char('.')));
    nf.handle_key(key(KeyCode::Char('1')));
    nf.handle_key(key(KeyCode::Char('4')));
    let term = render_component(&mut nf, 80, 24);
    assert_buffer_eq(&term, &["3.14▏"]);
}

#[test]
fn integer_rejects_dot() {
    let mut nf = NumberField::new(String::new(), true);
    nf.handle_key(key(KeyCode::Char('1')));
    nf.handle_key(key(KeyCode::Char('.')));
    nf.handle_key(key(KeyCode::Char('2')));
    let term = render_component(&mut nf, 80, 24);
    assert_buffer_eq(&term, &["12▏"]);
}

#[test]
fn rejects_alpha() {
    let mut nf = NumberField::new(String::new(), false);
    nf.handle_key(key(KeyCode::Char('1')));
    nf.handle_key(key(KeyCode::Char('a')));
    nf.handle_key(key(KeyCode::Char('2')));
    let term = render_component(&mut nf, 80, 24);
    assert_buffer_eq(&term, &["12▏"]);
}

#[test]
fn rejects_second_dot() {
    let mut nf = NumberField::new(String::new(), false);
    nf.handle_key(key(KeyCode::Char('1')));
    nf.handle_key(key(KeyCode::Char('.')));
    nf.handle_key(key(KeyCode::Char('2')));
    nf.handle_key(key(KeyCode::Char('.')));
    nf.handle_key(key(KeyCode::Char('3')));
    let term = render_component(&mut nf, 80, 24);
    assert_buffer_eq(&term, &["1.23▏"]);
}

#[test]
fn backspace_renders() {
    let mut nf = NumberField::new(String::new(), true);
    nf.handle_key(key(KeyCode::Char('9')));
    nf.handle_key(key(KeyCode::Char('9')));
    nf.handle_key(key(KeyCode::Backspace));
    let term = render_component(&mut nf, 80, 24);
    assert_buffer_eq(&term, &["9▏"]);
}

#[test]
fn unfocused_renders_without_cursor() {
    let mut nf = NumberField::new("42".to_string(), true);
    let ctx = RenderContext::new((80, 24)).with_focused(false);
    let lines = nf.render(&ctx);
    let term = render_lines(&lines, 80, 24);
    assert_buffer_eq(&term, &["42"]);
}
