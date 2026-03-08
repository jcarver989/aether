use crate::component::RenderContext;
use crate::line::Line;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config, Nucleo};
use std::cmp::Ordering;
use std::sync::Arc;

const DEFAULT_MAX_VISIBLE: usize = 10;
const MAX_MATCHES: u32 = 200;
const MATCH_TIMEOUT_MS: u64 = 10;
const MAX_TICKS_PER_QUERY: usize = 4;

pub enum PickerKey {
    Escape,
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    Confirm,
    Char(char),
    Backspace,
    BackspaceOnEmpty,
    ControlChar,
    Other,
}

pub trait Searchable: Clone {
    fn search_text(&self) -> String;
}

pub struct Combobox<T: Searchable + Send + Sync + 'static> {
    query: String,
    matches: Vec<T>,
    selected_index: usize,
    scroll_offset: usize,
    max_visible: usize,
    matcher: Nucleo<T>,
    match_sort: Option<fn(&T, &T) -> Ordering>,
}

impl<T: Searchable + Send + Sync + 'static> Combobox<T> {
    pub fn new(items: Vec<T>) -> Self {
        let mut matcher = Nucleo::new(Config::DEFAULT, Arc::new(|| {}), Some(1), 1);
        let injector = matcher.injector();
        for item in items {
            injector.push(item, |item, columns| {
                let text = item.search_text();
                columns[0] = text.as_str().into();
            });
        }
        let _ = matcher.tick(0);

        let mut combobox = Self {
            query: String::new(),
            matches: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            max_visible: DEFAULT_MAX_VISIBLE,
            matcher,
            match_sort: None,
        };
        combobox.matches = combobox.search(false);
        combobox
    }

    pub fn from_matches(matches: Vec<T>) -> Self {
        let nucleo = Nucleo::new(Config::DEFAULT, Arc::new(|| {}), Some(1), 1);
        Self {
            query: String::new(),
            matches,
            selected_index: 0,
            scroll_offset: 0,
            max_visible: DEFAULT_MAX_VISIBLE,
            matcher: nucleo,
            match_sort: None,
        }
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn matches(&self) -> &[T] {
        &self.matches
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn set_max_visible(&mut self, max: usize) {
        self.max_visible = max;
        self.ensure_visible();
    }

    pub fn set_match_sort(&mut self, sort: fn(&T, &T) -> Ordering) {
        self.match_sort = Some(sort);
        self.matches = self.search(false);
        self.scroll_offset = 0;
        if self.selected_index >= self.matches.len() {
            self.selected_index = 0;
        }
        self.ensure_visible();
    }

    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }

    pub fn selected(&self) -> Option<&T> {
        self.matches.get(self.selected_index)
    }

    pub fn push_query_char(&mut self, c: char) {
        self.query.push(c);
        self.refresh_matches(true);
    }

    pub fn pop_query_char(&mut self) {
        if self.query.pop().is_none() {
            return;
        }
        self.refresh_matches(false);
    }

    pub fn set_selected_index(&mut self, index: usize) {
        let len = self.matches.len();
        if len == 0 {
            return;
        }
        self.selected_index = index.min(len - 1);
        self.ensure_visible();
    }

    pub fn move_up(&mut self) {
        self.move_up_where(|_| true);
    }

    pub fn move_down(&mut self) {
        self.move_down_where(|_| true);
    }

    pub fn move_up_where(&mut self, predicate: impl Fn(&T) -> bool) {
        let len = self.matches.len();
        if len == 0 {
            return;
        }
        let mut idx = self.selected_index;
        for _ in 0..len {
            idx = if idx == 0 { len - 1 } else { idx - 1 };
            if predicate(&self.matches[idx]) {
                self.selected_index = idx;
                self.ensure_visible();
                return;
            }
        }
    }

    pub fn move_down_where(&mut self, predicate: impl Fn(&T) -> bool) {
        let len = self.matches.len();
        if len == 0 {
            return;
        }
        let mut idx = self.selected_index;
        for _ in 0..len {
            idx = (idx + 1) % len;
            if predicate(&self.matches[idx]) {
                self.selected_index = idx;
                self.ensure_visible();
                return;
            }
        }
    }

    pub fn select_first_where(&mut self, predicate: impl Fn(&T) -> bool) {
        if let Some(idx) = self.matches.iter().position(&predicate) {
            self.selected_index = idx;
            self.ensure_visible();
        }
    }

    pub fn render_items(
        &self,
        context: &RenderContext,
        render_item: impl Fn(&T, bool, &RenderContext) -> Line,
    ) -> Vec<Line> {
        self.visible_matches_with_selection()
            .into_iter()
            .map(|(item, is_selected)| render_item(item, is_selected, context))
            .collect()
    }

    pub fn visible_matches_with_selection(&self) -> Vec<(&T, bool)> {
        let visible_selected_index = self.visible_selected_index();
        self.visible_matches()
            .iter()
            .enumerate()
            .map(|(i, item)| (item, Some(i) == visible_selected_index))
            .collect()
    }

    fn visible_matches(&self) -> &[T] {
        let end = (self.scroll_offset + self.max_visible).min(self.matches.len());
        &self.matches[self.scroll_offset..end]
    }

    fn visible_selected_index(&self) -> Option<usize> {
        self.selected_index.checked_sub(self.scroll_offset)
    }

    fn ensure_visible(&mut self) {
        if self.matches.is_empty() {
            self.scroll_offset = 0;
            return;
        }
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + self.max_visible {
            self.scroll_offset = self.selected_index + 1 - self.max_visible;
        }
    }

    fn refresh_matches(&mut self, append: bool) {
        self.matches = self.search(append);
        self.scroll_offset = 0;
        if self.selected_index >= self.matches.len() {
            self.selected_index = 0;
        }
    }

    fn search(&mut self, append: bool) -> Vec<T> {
        self.matcher.pattern.reparse(
            0,
            &self.query,
            CaseMatching::Smart,
            Normalization::Smart,
            append,
        );
        let mut status = self.matcher.tick(MATCH_TIMEOUT_MS);
        let mut ticks = 0;
        while status.running && ticks < MAX_TICKS_PER_QUERY {
            status = self.matcher.tick(MATCH_TIMEOUT_MS);
            ticks += 1;
        }

        let snapshot = self.matcher.snapshot();
        let limit = snapshot.matched_item_count().min(MAX_MATCHES);
        let mut matches: Vec<T> = snapshot
            .matched_items(0..limit)
            .map(|item| item.data.clone())
            .collect();
        if let Some(sort) = self.match_sort {
            matches.sort_by(sort);
        }
        matches
    }
}

pub fn classify_key(key: KeyEvent, query_is_empty: bool) -> PickerKey {
    match key.code {
        KeyCode::Esc => PickerKey::Escape,
        KeyCode::Up => PickerKey::MoveUp,
        KeyCode::Down => PickerKey::MoveDown,
        KeyCode::Left => PickerKey::MoveLeft,
        KeyCode::Right => PickerKey::MoveRight,
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => PickerKey::MoveUp,
        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => PickerKey::MoveDown,
        KeyCode::Enter => PickerKey::Confirm,
        KeyCode::Char(c) if c.is_control() => PickerKey::ControlChar,
        KeyCode::Char(c) => PickerKey::Char(c),
        KeyCode::Backspace if query_is_empty => PickerKey::BackspaceOnEmpty,
        KeyCode::Backspace => PickerKey::Backspace,
        _ => PickerKey::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(combobox.matches.len(), 3);
        assert_eq!(combobox.query, "");
        assert_eq!(combobox.selected_index, 0);
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
        assert_eq!(combobox.matches.len(), 1);
        assert_eq!(combobox.matches[0].text, "banana");
    }

    #[test]
    fn push_query_char_clamps_selected_index() {
        let items = vec![FakeItem::new("a"), FakeItem::new("b"), FakeItem::new("c")];
        let mut combobox = Combobox::new(items);
        combobox.selected_index = 2;
        combobox.push_query_char('a');
        assert_eq!(combobox.selected_index, 0);
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
        assert_eq!(combobox.query, "c");
        combobox.push_query_char('a');
        assert_eq!(combobox.query, "ca");

        combobox.pop_query_char();
        assert_eq!(combobox.query, "c");
        combobox.pop_query_char();
        assert_eq!(combobox.query, "");

        // pop on empty is no-op
        combobox.pop_query_char();
        assert_eq!(combobox.query, "");
    }

    #[test]
    fn selection_wraps_around() {
        let items = vec![FakeItem::new("a"), FakeItem::new("b"), FakeItem::new("c")];
        let mut combobox = Combobox::new(items);

        combobox.move_up();
        assert_eq!(combobox.selected_index, 2);

        combobox.move_down();
        assert_eq!(combobox.selected_index, 0);
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
        assert_eq!(combobox.matches.len(), 1);
        assert_eq!(combobox.selected_index, 0);
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
        assert_eq!(combobox.matches.len(), 25);
    }

    #[test]
    fn visible_matches_returns_viewport_window() {
        let combobox = Combobox::from_matches(many_items(25));
        assert_eq!(combobox.visible_matches().len(), DEFAULT_MAX_VISIBLE);
        assert_eq!(combobox.visible_matches()[0].text, "item-0");
        assert_eq!(combobox.visible_matches()[9].text, "item-9");
    }

    #[test]
    fn visible_matches_returns_all_when_fewer_than_viewport() {
        let combobox = Combobox::from_matches(many_items(3));
        assert_eq!(combobox.visible_matches().len(), 3);
    }

    #[test]
    fn scroll_down_past_viewport_adjusts_offset() {
        let mut combobox = Combobox::from_matches(many_items(25));
        for _ in 0..12 {
            combobox.move_down();
        }
        assert_eq!(combobox.selected_index, 12);
        assert_eq!(combobox.scroll_offset, 3);
        assert_eq!(combobox.visible_matches()[0].text, "item-3");
        assert_eq!(combobox.visible_selected_index(), Some(9));
    }

    #[test]
    fn scroll_up_adjusts_offset() {
        let mut combobox = Combobox::from_matches(many_items(25));
        // Scroll down past viewport
        for _ in 0..15 {
            combobox.move_down();
        }
        assert_eq!(combobox.selected_index, 15);
        // Now scroll back up
        for _ in 0..10 {
            combobox.move_up();
        }
        assert_eq!(combobox.selected_index, 5);
        assert_eq!(combobox.scroll_offset, 5);
        assert_eq!(combobox.visible_selected_index(), Some(0));
    }

    #[test]
    fn wrap_down_resets_scroll_offset() {
        let mut combobox = Combobox::from_matches(many_items(25));
        // Move to last item
        for _ in 0..15 {
            combobox.move_down();
        }
        // Now wrap around from last to first
        combobox.selected_index = 24;
        combobox.ensure_visible();
        combobox.move_down();
        assert_eq!(combobox.selected_index, 0);
        assert_eq!(combobox.scroll_offset, 0);
    }

    #[test]
    fn wrap_up_scrolls_to_end() {
        let mut combobox = Combobox::from_matches(many_items(25));
        combobox.move_up();
        assert_eq!(combobox.selected_index, 24);
        assert_eq!(combobox.scroll_offset, 15);
        assert_eq!(combobox.visible_selected_index(), Some(9));
    }

    #[test]
    fn refresh_matches_resets_scroll_offset() {
        let items = many_items(25);
        let mut combobox = Combobox::from_matches(items);
        combobox.scroll_offset = 10;
        combobox.selected_index = 15;
        // Simulating what refresh_matches does via update_query
        combobox.matches = many_items(5);
        combobox.scroll_offset = 0;
        combobox.selected_index = 0;
        assert_eq!(combobox.scroll_offset, 0);
        assert_eq!(combobox.selected_index, 0);
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
        let context = RenderContext::new((120, 40));
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
        let context = RenderContext::new((120, 40));
        let lines = combobox.render_items(&context, |item, selected, _ctx| {
            let prefix = if selected { "> " } else { "  " };
            Line::new(format!("{prefix}{}", item.text))
        });
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].plain_text(), "> a");
        assert_eq!(lines[1].plain_text(), "  b");
        assert_eq!(lines[2].plain_text(), "  c");
    }

    #[test]
    fn set_max_visible_changes_viewport_size() {
        let mut combobox = Combobox::from_matches(many_items(25));
        assert_eq!(combobox.visible_matches().len(), DEFAULT_MAX_VISIBLE);

        combobox.set_max_visible(5);
        assert_eq!(combobox.visible_matches().len(), 5);

        combobox.set_max_visible(30);
        assert_eq!(combobox.visible_matches().len(), 25); // clamped to total items
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
        assert_eq!(combobox.selected_index, 2);
    }

    #[test]
    fn move_up_where_skips_disabled() {
        let items = vec![
            FakeItem::new("a"),
            FakeItem::disabled("b"),
            FakeItem::new("c"),
        ];
        let mut combobox = Combobox::from_matches(items);
        combobox.selected_index = 2;
        combobox.move_up_where(|item| !item.disabled);
        assert_eq!(combobox.selected_index, 0);
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
        assert_eq!(combobox.selected_index, 2);
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
        assert_eq!(combobox.selected_index, 0);
    }
}
