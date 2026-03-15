use tui::testing::render_lines;
use tui::{Combobox, Line, PickerKey, Searchable, ViewContext, classify_key};
use tui::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, PartialEq)]
struct FakeItem {
    text: String,
    disabled: bool,
}

impl FakeItem {
    fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            disabled: false,
        }
    }

    fn disabled(text: &str) -> Self {
        Self {
            text: text.to_string(),
            disabled: true,
        }
    }
}

impl Searchable for FakeItem {
    fn search_text(&self) -> String {
        self.text.clone()
    }
}

#[test]
fn new_returns_all_items_with_empty_query() {
    let items = vec![
        FakeItem::new("alpha"),
        FakeItem::new("beta"),
        FakeItem::new("gamma"),
    ];
    let combobox = Combobox::new(items);
    assert_eq!(combobox.matches().len(), 3);
    assert_eq!(combobox.query(), "");
    assert_eq!(combobox.selected_index(), 0);
}

#[test]
fn push_query_char_filters_matches() {
    let items = vec![
        FakeItem::new("apple"),
        FakeItem::new("banana"),
        FakeItem::new("avocado"),
    ];
    let mut combobox = Combobox::new(items);
    for c in "ban".chars() {
        combobox.push_query_char(c);
    }
    assert_eq!(combobox.matches().len(), 1);
    assert_eq!(combobox.matches()[0].text, "banana");
}

#[test]
fn push_query_char_clamps_selected_index() {
    let items = vec![FakeItem::new("a"), FakeItem::new("b"), FakeItem::new("c")];
    let mut combobox = Combobox::new(items);
    combobox.set_selected_index(2);
    combobox.push_query_char('a');
    assert_eq!(combobox.selected_index(), 0);
}

#[test]
fn push_and_pop_query_char() {
    let items = vec![
        FakeItem::new("cat"),
        FakeItem::new("car"),
        FakeItem::new("dog"),
    ];
    let mut combobox = Combobox::new(items);
    combobox.push_query_char('c');
    assert_eq!(combobox.query(), "c");
    combobox.push_query_char('a');
    assert_eq!(combobox.query(), "ca");

    combobox.pop_query_char();
    assert_eq!(combobox.query(), "c");
    combobox.pop_query_char();
    assert_eq!(combobox.query(), "");

    // pop on empty is no-op
    combobox.pop_query_char();
    assert_eq!(combobox.query(), "");
}

#[test]
fn selection_wraps_around() {
    let items = vec![FakeItem::new("a"), FakeItem::new("b"), FakeItem::new("c")];
    let mut combobox = Combobox::new(items);

    combobox.move_up();
    assert_eq!(combobox.selected_index(), 2);

    combobox.move_down();
    assert_eq!(combobox.selected_index(), 0);
}

#[test]
fn selected_returns_current_match() {
    let items = vec![FakeItem::new("x"), FakeItem::new("y")];
    let mut combobox = Combobox::new(items);
    assert_eq!(combobox.selected().unwrap().text, "x");

    combobox.move_down();
    assert_eq!(combobox.selected().unwrap().text, "y");
}

#[test]
fn from_matches_populates_directly() {
    let matches = vec![FakeItem::new("pre-populated")];
    let combobox = Combobox::from_matches(matches);
    assert_eq!(combobox.matches().len(), 1);
    assert_eq!(combobox.selected_index(), 0);
}

#[test]
fn empty_matches_selection_is_noop() {
    let mut combobox: Combobox<FakeItem> = Combobox::from_matches(vec![]);
    combobox.move_up();
    combobox.move_down();
    assert!(combobox.selected().is_none());
}

fn many_items(n: usize) -> Vec<FakeItem> {
    (0..n)
        .map(|i| FakeItem::new(&format!("item-{i}")))
        .collect()
}

#[test]
fn from_matches_stores_more_than_viewport() {
    let combobox = Combobox::from_matches(many_items(25));
    assert_eq!(combobox.matches().len(), 25);
}

#[test]
fn visible_matches_returns_viewport_window() {
    let combobox = Combobox::from_matches(many_items(25));
    let visible = combobox.visible_matches_with_selection();
    assert_eq!(visible.len(), 10); // DEFAULT_MAX_VISIBLE
    assert_eq!(visible[0].0.text, "item-0");
    assert_eq!(visible[9].0.text, "item-9");
}

#[test]
fn visible_matches_returns_all_when_fewer_than_viewport() {
    let combobox = Combobox::from_matches(many_items(3));
    let visible = combobox.visible_matches_with_selection();
    assert_eq!(visible.len(), 3);
}

#[test]
fn scroll_down_past_viewport_adjusts_offset() {
    let mut combobox = Combobox::from_matches(many_items(25));
    for _ in 0..12 {
        combobox.move_down();
    }
    assert_eq!(combobox.selected_index(), 12);
    let visible = combobox.visible_matches_with_selection();
    assert_eq!(visible[0].0.text, "item-3");
    let selected_visible_idx = visible.iter().position(|(_, sel)| *sel).unwrap();
    assert_eq!(selected_visible_idx, 9);
}

#[test]
fn scroll_up_adjusts_offset() {
    let mut combobox = Combobox::from_matches(many_items(25));
    // Scroll down past viewport
    for _ in 0..15 {
        combobox.move_down();
    }
    assert_eq!(combobox.selected_index(), 15);
    // Now scroll back up
    for _ in 0..10 {
        combobox.move_up();
    }
    assert_eq!(combobox.selected_index(), 5);
    let visible = combobox.visible_matches_with_selection();
    let selected_visible_idx = visible.iter().position(|(_, sel)| *sel).unwrap();
    assert_eq!(selected_visible_idx, 0);
}

#[test]
fn wrap_down_resets_scroll_offset() {
    let mut combobox = Combobox::from_matches(many_items(25));
    // Move to last item
    for _ in 0..24 {
        combobox.move_down();
    }
    assert_eq!(combobox.selected_index(), 24);
    // Now wrap around from last to first
    combobox.move_down();
    assert_eq!(combobox.selected_index(), 0);
    let visible = combobox.visible_matches_with_selection();
    assert_eq!(visible[0].0.text, "item-0");
}

#[test]
fn wrap_up_scrolls_to_end() {
    let mut combobox = Combobox::from_matches(many_items(25));
    combobox.move_up();
    assert_eq!(combobox.selected_index(), 24);
    let visible = combobox.visible_matches_with_selection();
    let selected_visible_idx = visible.iter().position(|(_, sel)| *sel).unwrap();
    assert_eq!(selected_visible_idx, 9);
}

#[test]
fn set_selected_index_clamps_and_ensures_visible() {
    let mut combobox = Combobox::from_matches(many_items(25));
    combobox.set_selected_index(100);
    assert_eq!(combobox.selected_index(), 24);

    combobox.set_selected_index(0);
    assert_eq!(combobox.selected_index(), 0);
}

#[test]
fn set_selected_index_noop_on_empty() {
    let mut combobox: Combobox<FakeItem> = Combobox::from_matches(vec![]);
    combobox.set_selected_index(5);
    assert_eq!(combobox.selected_index(), 0);
}

#[test]
fn classify_key_escape() {
    assert!(matches!(
        classify_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), true),
        PickerKey::Escape
    ));
}

#[test]
fn classify_key_arrows_and_ctrl() {
    assert!(matches!(
        classify_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), true),
        PickerKey::MoveUp
    ));
    assert!(matches!(
        classify_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), true),
        PickerKey::MoveDown
    ));
    assert!(matches!(
        classify_key(
            KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
            true
        ),
        PickerKey::MoveUp
    ));
    assert!(matches!(
        classify_key(
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
            true
        ),
        PickerKey::MoveDown
    ));
}

#[test]
fn classify_key_enter() {
    assert!(matches!(
        classify_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), true),
        PickerKey::Confirm
    ));
}

#[test]
fn classify_key_char() {
    assert!(matches!(
        classify_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE), true),
        PickerKey::Char('a')
    ));
}

#[test]
fn classify_key_backspace_empty_vs_nonempty() {
    // Empty query -> BackspaceOnEmpty
    assert!(matches!(
        classify_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE), true),
        PickerKey::BackspaceOnEmpty
    ));

    // Non-empty query -> Backspace
    assert!(matches!(
        classify_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE), false),
        PickerKey::Backspace
    ));
}

#[test]
fn classify_key_left_right() {
    assert!(matches!(
        classify_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), true),
        PickerKey::MoveLeft
    ));
    assert!(matches!(
        classify_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), true),
        PickerKey::MoveRight
    ));
}

#[test]
fn classify_key_other() {
    assert!(matches!(
        classify_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), true),
        PickerKey::Other
    ));
}

#[test]
fn render_items_empty_returns_empty() {
    let combobox: Combobox<FakeItem> = Combobox::from_matches(vec![]);
    let context = ViewContext::new((120, 40));
    let lines = combobox.render_items(&context, |_, _, _| Line::new("x".to_string()));
    assert!(lines.is_empty());
}

#[test]
fn render_items_calls_closure_for_each_visible() {
    let combobox = Combobox::from_matches(vec![
        FakeItem::new("a"),
        FakeItem::new("b"),
        FakeItem::new("c"),
    ]);
    let context = ViewContext::new((120, 40));
    let lines = combobox.render_items(&context, |item, selected, _ctx| {
        let prefix = if selected { "> " } else { "  " };
        Line::new(format!("{prefix}{}", item.text))
    });
    let term = render_lines(&lines, 120, 3);
    let output = term.get_lines();
    assert_eq!(output.len(), 3);
    assert!(output[0].contains("> a"));
    assert!(output[1].contains("b"));
    assert!(output[2].contains("c"));
}

#[test]
fn set_max_visible_changes_viewport_size() {
    let mut combobox = Combobox::from_matches(many_items(25));
    let initial_visible = combobox.visible_matches_with_selection().len();
    assert_eq!(initial_visible, 10); // DEFAULT_MAX_VISIBLE

    combobox.set_max_visible(5);
    assert_eq!(combobox.visible_matches_with_selection().len(), 5);

    combobox.set_max_visible(30);
    assert_eq!(combobox.visible_matches_with_selection().len(), 25); // clamped to total items
}

#[test]
fn move_down_where_skips_disabled() {
    let items = vec![
        FakeItem::new("a"),
        FakeItem::disabled("b"),
        FakeItem::new("c"),
    ];
    let mut combobox = Combobox::from_matches(items);
    combobox.move_down_where(|item| !item.disabled);
    assert_eq!(combobox.selected_index(), 2);
}

#[test]
fn move_up_where_skips_disabled() {
    let items = vec![
        FakeItem::new("a"),
        FakeItem::disabled("b"),
        FakeItem::new("c"),
    ];
    let mut combobox = Combobox::from_matches(items);
    combobox.set_selected_index(2);
    combobox.move_up_where(|item| !item.disabled);
    assert_eq!(combobox.selected_index(), 0);
}

#[test]
fn select_first_where_finds_first_enabled() {
    let items = vec![
        FakeItem::disabled("a"),
        FakeItem::disabled("b"),
        FakeItem::new("c"),
    ];
    let mut combobox = Combobox::from_matches(items);
    combobox.select_first_where(|item| !item.disabled);
    assert_eq!(combobox.selected_index(), 2);
}

#[test]
fn move_down_where_noop_when_all_filtered() {
    let items = vec![
        FakeItem::disabled("a"),
        FakeItem::disabled("b"),
        FakeItem::disabled("c"),
    ];
    let mut combobox = Combobox::from_matches(items);
    combobox.move_down_where(|item| !item.disabled);
    assert_eq!(combobox.selected_index(), 0);
}
