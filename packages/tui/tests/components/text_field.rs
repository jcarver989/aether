use super::*;
use crossterm::event::KeyCode;
use tui::TextField;
use tui::rendering::screen::Screen;

#[test]
fn empty_renders_cursor() {
    let tf = TextField::new(String::new());
    let term = render_component(&tf, 80, 24);
    assert_buffer_eq(&term, &["▏"]);
}

#[test]
fn with_value_renders_text_and_cursor() {
    let tf = TextField::new("hello".to_string());
    let term = render_component(&tf, 80, 24);
    assert_buffer_eq(&term, &["hello▏"]);
}

#[test]
fn typing_appends_to_render() {
    let mut tf = TextField::new(String::new());
    tf.handle_key(key(KeyCode::Char('a')));
    tf.handle_key(key(KeyCode::Char('b')));
    tf.handle_key(key(KeyCode::Char('c')));
    let term = render_component(&tf, 80, 24);
    assert_buffer_eq(&term, &["abc▏"]);
}

#[test]
fn backspace_removes_from_render() {
    let mut tf = TextField::new("hi".to_string());
    tf.handle_key(key(KeyCode::Backspace));
    let term = render_component(&tf, 80, 24);
    assert_buffer_eq(&term, &["h▏"]);
}

#[test]
fn backspace_on_empty_renders_cursor() {
    let mut tf = TextField::new(String::new());
    tf.handle_key(key(KeyCode::Backspace));
    let term = render_component(&tf, 80, 24);
    assert_buffer_eq(&term, &["▏"]);
}

#[test]
fn screen_diff_after_mutation() {
    let mut tf = TextField::new("ab".to_string());
    let mut screen = Screen::new();
    let mut terminal = TestTerminal::new(80, 24);

    // Initial render
    render_component_with_screen(&tf, &mut screen, &mut terminal, 80, 24);
    assert_buffer_eq(&terminal, &["ab▏"]);

    // Mutate and re-render through same Screen (exercises diff path)
    tf.handle_key(key(KeyCode::Char('c')));
    render_component_with_screen(&tf, &mut screen, &mut terminal, 80, 24);
    assert_buffer_eq(&terminal, &["abc▏"]);
}

#[test]
fn unfocused_renders_without_cursor() {
    let tf = TextField::new("hello".to_string());
    let ctx = RenderContext::new((80, 24)).with_focused(false);
    let lines = tf.render(&ctx);
    let term = render_lines(&lines, 80, 24);
    assert_buffer_eq(&term, &["hello"]);
}

#[test]
fn unfocused_empty_renders_empty() {
    let tf = TextField::new(String::new());
    let ctx = RenderContext::new((80, 24)).with_focused(false);
    let lines = tf.render(&ctx);
    let term = render_lines(&lines, 80, 24);
    assert_buffer_eq(&term, &[""]);
}
