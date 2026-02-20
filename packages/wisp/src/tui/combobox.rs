use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config, Nucleo};
use std::sync::Arc;

const MAX_VISIBLE: usize = 10;
const MAX_MATCHES: u32 = 200;
const MATCH_TIMEOUT_MS: u64 = 10;
const MAX_TICKS_PER_QUERY: usize = 4;

pub trait Searchable: Clone {
    fn search_text(&self) -> String;
}

pub struct Combobox<T: Searchable + Send + Sync + 'static> {
    pub query: String,
    pub matches: Vec<T>,
    pub selected_index: usize,
    scroll_offset: usize,
    matcher: Nucleo<T>,
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
            matcher,
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
            matcher: nucleo,
        }
    }

    pub fn update_query(&mut self, query: String) {
        let append = query.starts_with(&self.query);
        self.query = query;
        self.refresh_matches(append);
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

    pub fn move_selection_up(&mut self) {
        match self.selected_index {
            _ if self.matches.is_empty() => {}
            0 => self.selected_index = self.matches.len() - 1,
            i => self.selected_index = i - 1,
        }
        self.ensure_visible();
    }

    pub fn move_selection_down(&mut self) {
        match self.selected_index {
            _ if self.matches.is_empty() => {}
            i if i >= self.matches.len() - 1 => self.selected_index = 0,
            _ => self.selected_index += 1,
        }
        self.ensure_visible();
    }

    pub fn visible_matches(&self) -> &[T] {
        let end = (self.scroll_offset + MAX_VISIBLE).min(self.matches.len());
        &self.matches[self.scroll_offset..end]
    }

    pub fn visible_selected_index(&self) -> Option<usize> {
        self.selected_index.checked_sub(self.scroll_offset)
    }

    pub fn ensure_visible(&mut self) {
        if self.matches.is_empty() {
            self.scroll_offset = 0;
            return;
        }
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + MAX_VISIBLE {
            self.scroll_offset = self.selected_index + 1 - MAX_VISIBLE;
        }
    }

    pub fn selected(&self) -> Option<&T> {
        self.matches.get(self.selected_index)
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
        snapshot
            .matched_items(0..limit)
            .map(|item| item.data.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct FakeItem {
        text: String,
    }

    impl FakeItem {
        fn new(text: &str) -> Self {
            Self {
                text: text.to_string(),
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
    fn update_query_filters_matches() {
        let items = vec![
            FakeItem::new("apple"),
            FakeItem::new("banana"),
            FakeItem::new("avocado"),
        ];
        let mut combobox = Combobox::new(items);
        combobox.update_query("ban".to_string());
        assert_eq!(combobox.matches.len(), 1);
        assert_eq!(combobox.matches[0].text, "banana");
    }

    #[test]
    fn update_query_clamps_selected_index() {
        let items = vec![FakeItem::new("a"), FakeItem::new("b"), FakeItem::new("c")];
        let mut combobox = Combobox::new(items);
        combobox.selected_index = 2;
        combobox.update_query("a".to_string());
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

        combobox.move_selection_up();
        assert_eq!(combobox.selected_index, 2);

        combobox.move_selection_down();
        assert_eq!(combobox.selected_index, 0);
    }

    #[test]
    fn selected_returns_current_match() {
        let items = vec![FakeItem::new("x"), FakeItem::new("y")];
        let mut combobox = Combobox::new(items);
        assert_eq!(combobox.selected().unwrap().text, "x");

        combobox.move_selection_down();
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
        combobox.move_selection_up();
        combobox.move_selection_down();
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
        assert_eq!(combobox.visible_matches().len(), MAX_VISIBLE);
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
            combobox.move_selection_down();
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
            combobox.move_selection_down();
        }
        assert_eq!(combobox.selected_index, 15);
        // Now scroll back up
        for _ in 0..10 {
            combobox.move_selection_up();
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
            combobox.move_selection_down();
        }
        // Now wrap around from last to first
        combobox.selected_index = 24;
        combobox.ensure_visible();
        combobox.move_selection_down();
        assert_eq!(combobox.selected_index, 0);
        assert_eq!(combobox.scroll_offset, 0);
    }

    #[test]
    fn wrap_up_scrolls_to_end() {
        let mut combobox = Combobox::from_matches(many_items(25));
        combobox.move_selection_up();
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
}
