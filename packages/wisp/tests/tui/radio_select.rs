use super::*;
use wisp::tui::RadioSelect;

#[test]
fn renders_all_options_first_selected() {
    let mut rs = RadioSelect::new(sample_options(), 0);
    let term = render_component(&mut rs, 80, 24);
    assert_buffer_eq(&term, &["● Alpha", "○ Beta", "○ Gamma"]);
}

#[test]
fn renders_second_selected() {
    let mut rs = RadioSelect::new(sample_options(), 1);
    let term = render_component(&mut rs, 80, 24);
    assert_buffer_eq(&term, &["○ Alpha", "● Beta", "○ Gamma"]);
}

#[test]
fn down_arrow_changes_selection() {
    let mut rs = RadioSelect::new(sample_options(), 0);
    rs.handle_key(key(KeyCode::Down));
    let term = render_component(&mut rs, 80, 24);
    assert_buffer_eq(&term, &["○ Alpha", "● Beta", "○ Gamma"]);
}

#[test]
fn up_from_first_wraps_to_last() {
    let mut rs = RadioSelect::new(sample_options(), 0);
    rs.handle_key(key(KeyCode::Up));
    let term = render_component(&mut rs, 80, 24);
    assert_buffer_eq(&term, &["○ Alpha", "○ Beta", "● Gamma"]);
}

#[test]
fn right_from_last_wraps_to_first() {
    let mut rs = RadioSelect::new(sample_options(), 2);
    rs.handle_key(key(KeyCode::Right));
    let term = render_component(&mut rs, 80, 24);
    assert_buffer_eq(&term, &["● Alpha", "○ Beta", "○ Gamma"]);
}

#[test]
fn render_inline_shows_selected_title() {
    let rs = RadioSelect::new(sample_options(), 1);
    let ctx = RenderContext::new((80, 24));
    let line = rs.render_inline(&ctx);
    let term = render_lines(&[line], 80, 24);
    let lines = term.get_lines();
    assert!(
        lines[0].contains("Beta"),
        "Expected 'Beta' in inline render, got: '{}'",
        lines[0]
    );
}
