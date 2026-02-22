use super::*;
use wisp::tui::MultiSelect;

#[test]
fn renders_all_unchecked() {
    let mut ms = MultiSelect::new(sample_options(), vec![false, false, false]);
    let term = render_component(&mut ms, 80, 24);
    assert_buffer_eq(&term, &["[ ] Alpha", "[ ] Beta", "[ ] Gamma"]);
}

#[test]
fn renders_some_checked() {
    let mut ms = MultiSelect::new(sample_options(), vec![true, false, true]);
    let term = render_component(&mut ms, 80, 24);
    assert_buffer_eq(&term, &["[x] Alpha", "[ ] Beta", "[x] Gamma"]);
}

#[test]
fn space_toggles_at_cursor() {
    let mut ms = MultiSelect::new(sample_options(), vec![false, false, false]);
    ms.handle_key(key(KeyCode::Char(' ')));
    let term = render_component(&mut ms, 80, 24);
    assert_buffer_eq(&term, &["[x] Alpha", "[ ] Beta", "[ ] Gamma"]);
}

#[test]
fn navigate_and_toggle_multiple() {
    let mut ms = MultiSelect::new(sample_options(), vec![false, false, false]);
    // Toggle Alpha (cursor=0)
    ms.handle_key(key(KeyCode::Char(' ')));
    // Move to Beta, then Gamma
    ms.handle_key(key(KeyCode::Down));
    ms.handle_key(key(KeyCode::Down));
    // Toggle Gamma (cursor=2)
    ms.handle_key(key(KeyCode::Char(' ')));
    let term = render_component(&mut ms, 80, 24);
    assert_buffer_eq(&term, &["[x] Alpha", "[ ] Beta", "[x] Gamma"]);
}

#[test]
fn cursor_wraps_up_from_first() {
    let mut ms = MultiSelect::new(sample_options(), vec![false, false, false]);
    // Press Up from cursor=0, should wrap to cursor=2 (Gamma)
    ms.handle_key(key(KeyCode::Up));
    assert_eq!(ms.cursor, 2);
    // Toggle at the new cursor position to verify it's on Gamma
    ms.handle_key(key(KeyCode::Char(' ')));
    let term = render_component(&mut ms, 80, 24);
    assert_buffer_eq(&term, &["[ ] Alpha", "[ ] Beta", "[x] Gamma"]);
}

#[test]
fn render_inline_none_selected() {
    let ms = MultiSelect::new(sample_options(), vec![false, false, false]);
    let ctx = RenderContext::new((80, 24));
    let line = ms.render_inline(&ctx);
    let term = render_lines(&[line], 80, 24);
    let lines = term.get_lines();
    assert!(
        lines[0].contains("(none)"),
        "Expected '(none)' in inline render, got: '{}'",
        lines[0]
    );
}

#[test]
fn render_inline_with_selections() {
    let ms = MultiSelect::new(sample_options(), vec![true, false, true]);
    let ctx = RenderContext::new((80, 24));
    let line = ms.render_inline(&ctx);
    let term = render_lines(&[line], 80, 24);
    let lines = term.get_lines();
    assert!(
        lines[0].contains("Alpha, Gamma"),
        "Expected 'Alpha, Gamma' in inline render, got: '{}'",
        lines[0]
    );
}
