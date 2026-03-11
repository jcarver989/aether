use super::*;
use crossterm::event::KeyCode;
use tui::RadioSelect;

#[test]
fn renders_all_options_first_selected() {
    let rs = RadioSelect::new(sample_options(), 0);
    let term = render_component(|ctx| rs.render(ctx), 80, 24);
    assert_buffer_eq(&term, &["● Alpha", "○ Beta", "○ Gamma"]);
}

#[test]
fn renders_second_selected() {
    let rs = RadioSelect::new(sample_options(), 1);
    let term = render_component(|ctx| rs.render(ctx), 80, 24);
    assert_buffer_eq(&term, &["○ Alpha", "● Beta", "○ Gamma"]);
}

#[test]
fn down_arrow_changes_selection() {
    let mut rs = RadioSelect::new(sample_options(), 0);
    rs.on_event(&WidgetEvent::Key(key(KeyCode::Down)));
    let term = render_component(|ctx| rs.render(ctx), 80, 24);
    assert_buffer_eq(&term, &["○ Alpha", "● Beta", "○ Gamma"]);
}

#[test]
fn up_from_first_wraps_to_last() {
    let mut rs = RadioSelect::new(sample_options(), 0);
    rs.on_event(&WidgetEvent::Key(key(KeyCode::Up)));
    let term = render_component(|ctx| rs.render(ctx), 80, 24);
    assert_buffer_eq(&term, &["○ Alpha", "○ Beta", "● Gamma"]);
}

#[test]
fn right_from_last_wraps_to_first() {
    let mut rs = RadioSelect::new(sample_options(), 2);
    rs.on_event(&WidgetEvent::Key(key(KeyCode::Right)));
    let term = render_component(|ctx| rs.render(ctx), 80, 24);
    assert_buffer_eq(&term, &["● Alpha", "○ Beta", "○ Gamma"]);
}

#[test]
fn unfocused_renders_selected_title_inline() {
    let rs = RadioSelect::new(sample_options(), 1);
    let ctx = ViewContext::new((80, 24)).with_focused(false);
    let lines = rs.render(&ctx);
    assert_eq!(
        lines.len(),
        1,
        "Unfocused should render a single inline line"
    );
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    assert!(
        output[0].contains("Beta"),
        "Expected 'Beta' in unfocused render, got: '{}'",
        output[0]
    );
}
