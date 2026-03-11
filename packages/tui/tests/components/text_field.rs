use super::*;
use crossterm::event::KeyCode;
use tui::TextField;
use tui::advanced::Renderer;

#[test]
fn empty_renders_cursor() {
    let tf = TextField::new(String::new());
    let term = render_component(|ctx| tf.render(ctx), 80, 24);
    assert_buffer_eq(&term, &["▏"]);
}

#[test]
fn with_value_renders_text_and_cursor() {
    let tf = TextField::new("hello".to_string());
    let term = render_component(|ctx| tf.render(ctx), 80, 24);
    assert_buffer_eq(&term, &["hello▏"]);
}

#[test]
fn typing_appends_to_render() {
    let mut tf = TextField::new(String::new());
    tf.on_event(&Event::Key(key(KeyCode::Char('a'))));
    tf.on_event(&Event::Key(key(KeyCode::Char('b'))));
    tf.on_event(&Event::Key(key(KeyCode::Char('c'))));
    let term = render_component(|ctx| tf.render(ctx), 80, 24);
    assert_buffer_eq(&term, &["abc▏"]);
}

#[test]
fn backspace_removes_from_render() {
    let mut tf = TextField::new("hi".to_string());
    tf.on_event(&Event::Key(key(KeyCode::Backspace)));
    let term = render_component(|ctx| tf.render(ctx), 80, 24);
    assert_buffer_eq(&term, &["h▏"]);
}

#[test]
fn backspace_on_empty_renders_cursor() {
    let mut tf = TextField::new(String::new());
    tf.on_event(&Event::Key(key(KeyCode::Backspace)));
    let term = render_component(|ctx| tf.render(ctx), 80, 24);
    assert_buffer_eq(&term, &["▏"]);
}

#[test]
fn terminal_state_diff_after_mutation() {
    let mut tf = TextField::new("ab".to_string());
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, tui::Theme::default());
    renderer.on_resize((80, 24));

    // Initial render
    render_component_with_renderer(|ctx| tf.render(ctx), &mut renderer, 80, 24);
    assert_buffer_eq(renderer.writer(), &["ab▏"]);

    // Mutate and re-render through same Renderer (exercises diff path)
    tf.on_event(&Event::Key(key(KeyCode::Char('c'))));
    render_component_with_renderer(|ctx| tf.render(ctx), &mut renderer, 80, 24);
    assert_buffer_eq(renderer.writer(), &["abc▏"]);
}

#[test]
fn unfocused_renders_without_cursor() {
    let tf = TextField::new("hello".to_string());
    let ctx = ViewContext::new((80, 24));
    let lines = tf.render_field(&ctx, false);
    let term = render_lines(&lines, 80, 24);
    assert_buffer_eq(&term, &["hello"]);
}

#[test]
fn unfocused_empty_renders_empty() {
    let tf = TextField::new(String::new());
    let ctx = ViewContext::new((80, 24));
    let lines = tf.render_field(&ctx, false);
    let term = render_lines(&lines, 80, 24);
    assert_buffer_eq(&term, &[""]);
}
