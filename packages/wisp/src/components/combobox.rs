use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config, Nucleo};
use std::sync::Arc;

const MAX_VISIBLE_MATCHES: u32 = 10;
const MATCH_TIMEOUT_MS: u64 = 10;
const MAX_TICKS_PER_QUERY: usize = 4;

pub trait Searchable: Clone {
    fn search_text(&self) -> String;
}

pub struct Combobox<T: Searchable + Send + Sync + 'static> {
    pub query: String,
    pub matches: Vec<T>,
    pub selected_index: usize,
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
            matcher,
        };
        combobox.matches = combobox.search(false);
        combobox
    }

    pub fn from_matches(matches: Vec<T>) -> Self {
        let matcher = Nucleo::new(Config::DEFAULT, Arc::new(|| {}), Some(1), 1);
        Self {
            query: String::new(),
            matches,
            selected_index: 0,
            matcher,
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
    }

    pub fn move_selection_down(&mut self) {
        match self.selected_index {
            _ if self.matches.is_empty() => {}
            i if i >= self.matches.len() - 1 => self.selected_index = 0,
            _ => self.selected_index += 1,
        }
    }

    pub fn selected(&self) -> Option<&T> {
        self.matches.get(self.selected_index)
    }

    fn refresh_matches(&mut self, append: bool) {
        self.matches = self.search(append);
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
        let limit = snapshot.matched_item_count().min(MAX_VISIBLE_MATCHES);
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
        let items = vec![
            FakeItem::new("a"),
            FakeItem::new("b"),
            FakeItem::new("c"),
        ];
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
        let items = vec![
            FakeItem::new("a"),
            FakeItem::new("b"),
            FakeItem::new("c"),
        ];
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
}
