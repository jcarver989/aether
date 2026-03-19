use crossterm::event::{KeyCode, MouseEventKind};

use crate::components::{Component, Event, ViewContext, wrap_selection};
use crate::line::Line;
use crate::rendering::frame::Frame;

pub trait SelectItem {
    fn render_item(&self, selected: bool, ctx: &ViewContext) -> Line;
}

#[derive(Debug)]
pub enum SelectListMessage {
    Close,
    Select(usize),
}

pub struct SelectList<T: SelectItem> {
    items: Vec<T>,
    selected_index: usize,
    placeholder: String,
}

impl<T: SelectItem> SelectList<T> {
    pub fn new(items: Vec<T>, placeholder: impl Into<String>) -> Self {
        Self {
            items,
            selected_index: 0,
            placeholder: placeholder.into(),
        }
    }

    pub fn items(&self) -> &[T] {
        &self.items
    }

    pub fn items_mut(&mut self) -> &mut [T] {
        &mut self.items
    }

    pub fn retain(&mut self, f: impl FnMut(&T) -> bool) {
        self.items.retain(f);
        self.clamp_index();
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn selected_item(&self) -> Option<&T> {
        self.items.get(self.selected_index)
    }

    pub fn set_items(&mut self, items: Vec<T>) {
        self.items = items;
        self.clamp_index();
    }

    pub fn set_selected(&mut self, index: usize) {
        if index < self.items.len() {
            self.selected_index = index;
        }
    }

    pub fn push(&mut self, item: T) {
        self.items.push(item);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn clamp_index(&mut self) {
        self.selected_index = self.selected_index.min(self.items.len().saturating_sub(1));
    }
}

impl<T: SelectItem> Component for SelectList<T> {
    type Message = SelectListMessage;

    async fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        if let Event::Mouse(mouse) = event {
            return match mouse.kind {
                MouseEventKind::ScrollUp => {
                    wrap_selection(&mut self.selected_index, self.items.len(), -1);
                    Some(vec![])
                }
                MouseEventKind::ScrollDown => {
                    wrap_selection(&mut self.selected_index, self.items.len(), 1);
                    Some(vec![])
                }
                _ => Some(vec![]),
            };
        }
        let Event::Key(key) = event else {
            return None;
        };
        match key.code {
            KeyCode::Esc => Some(vec![SelectListMessage::Close]),
            KeyCode::Up => {
                wrap_selection(&mut self.selected_index, self.items.len(), -1);
                Some(vec![])
            }
            KeyCode::Down => {
                wrap_selection(&mut self.selected_index, self.items.len(), 1);
                Some(vec![])
            }
            KeyCode::Enter => {
                if self.items.is_empty() {
                    Some(vec![])
                } else {
                    Some(vec![SelectListMessage::Select(self.selected_index)])
                }
            }
            _ => Some(vec![]),
        }
    }

    fn render(&mut self, ctx: &ViewContext) -> Frame {
        if self.items.is_empty() {
            return Frame::new(vec![Line::new(format!("  ({})", self.placeholder))]);
        }

        let inner = ctx.with_size((ctx.size.width.saturating_sub(2), ctx.size.height));
        Frame::new(
            self.items
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    item.render_item(i == self.selected_index, &inner)
                        .prepend("  ")
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};

    struct TestItem(String);

    impl SelectItem for TestItem {
        fn render_item(&self, _selected: bool, _ctx: &ViewContext) -> Line {
            Line::new(self.0.clone())
        }
    }

    fn items(names: &[&str]) -> Vec<TestItem> {
        names.iter().map(|n| TestItem(n.to_string())).collect()
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[tokio::test]
    async fn navigation_wraps_down() {
        let mut list = SelectList::new(items(&["a", "b", "c"]), "empty");
        assert_eq!(list.selected_index(), 0);

        list.on_event(&key(KeyCode::Down)).await;
        assert_eq!(list.selected_index(), 1);

        list.on_event(&key(KeyCode::Down)).await;
        list.on_event(&key(KeyCode::Down)).await;
        assert_eq!(list.selected_index(), 0);
    }

    #[tokio::test]
    async fn navigation_wraps_up() {
        let mut list = SelectList::new(items(&["a", "b", "c"]), "empty");
        list.on_event(&key(KeyCode::Up)).await;
        assert_eq!(list.selected_index(), 2);
    }

    #[tokio::test]
    async fn esc_emits_close() {
        let mut list = SelectList::new(items(&["a"]), "empty");
        let outcome = list.on_event(&key(KeyCode::Esc)).await;
        assert!(matches!(
            outcome.unwrap().as_slice(),
            [SelectListMessage::Close]
        ));
    }

    #[tokio::test]
    async fn enter_emits_select_with_index() {
        let mut list = SelectList::new(items(&["a", "b", "c"]), "empty");
        list.on_event(&key(KeyCode::Down)).await;
        let outcome = list.on_event(&key(KeyCode::Enter)).await;
        match outcome.unwrap().as_slice() {
            [SelectListMessage::Select(idx)] => assert_eq!(*idx, 1),
            other => panic!("expected Select(1), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn enter_on_empty_is_noop() {
        let mut list: SelectList<TestItem> = SelectList::new(vec![], "empty");
        let outcome = list.on_event(&key(KeyCode::Enter)).await;
        assert!(outcome.unwrap().is_empty());
    }

    #[test]
    fn empty_list_shows_placeholder() {
        let mut list: SelectList<TestItem> = SelectList::new(vec![], "no items");
        let ctx = ViewContext::new((80, 24));
        let frame = list.render(&ctx);
        assert_eq!(frame.lines().len(), 1);
        assert!(frame.lines()[0].plain_text().contains("no items"));
    }

    #[test]
    fn render_shows_selected_indicator() {
        let mut list = SelectList::new(items(&["alpha", "beta"]), "empty");
        let ctx = ViewContext::new((80, 24));
        let frame = list.render(&ctx);
        assert_eq!(frame.lines().len(), 2);
        assert!(frame.lines()[0].plain_text().starts_with("  "));
        assert!(frame.lines()[1].plain_text().starts_with("  "));
    }

    #[tokio::test]
    async fn set_items_clamps_index() {
        let mut list = SelectList::new(items(&["a", "b", "c"]), "empty");
        list.on_event(&key(KeyCode::Down)).await;
        list.on_event(&key(KeyCode::Down)).await;
        assert_eq!(list.selected_index(), 2);

        list.set_items(items(&["x"]));
        assert_eq!(list.selected_index(), 0);
    }

    #[tokio::test]
    async fn set_items_preserves_index_when_in_range() {
        let mut list = SelectList::new(items(&["a", "b", "c"]), "empty");
        list.on_event(&key(KeyCode::Down)).await;
        assert_eq!(list.selected_index(), 1);

        list.set_items(items(&["x", "y", "z"]));
        assert_eq!(list.selected_index(), 1);
    }

    #[test]
    fn push_adds_item() {
        let mut list = SelectList::new(items(&["a"]), "empty");
        list.push(TestItem("b".to_string()));
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn tick_events_are_ignored() {
        let mut list = SelectList::new(items(&["a"]), "empty");
        let outcome = list.on_event(&Event::Tick).await;
        assert!(outcome.is_none());
    }

    #[tokio::test]
    async fn mouse_scroll_moves_selection() {
        use crossterm::event::{MouseEvent, MouseEventKind};
        let mut list = SelectList::new(items(&["a", "b", "c"]), "empty");
        assert_eq!(list.selected_index(), 0);

        let scroll_down = Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
        list.on_event(&scroll_down).await;
        assert_eq!(list.selected_index(), 1);

        let scroll_up = Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
        list.on_event(&scroll_up).await;
        assert_eq!(list.selected_index(), 0);
    }

    #[tokio::test]
    async fn retain_removes_items_and_clamps_index() {
        let mut list = SelectList::new(items(&["a", "b", "c"]), "empty");
        list.on_event(&key(KeyCode::Down)).await;
        list.on_event(&key(KeyCode::Down)).await;
        assert_eq!(list.selected_index(), 2);

        list.retain(|item| item.0 != "c");
        assert_eq!(list.len(), 2);
        assert_eq!(list.selected_index(), 1);
    }

    #[test]
    fn retain_to_empty_clamps_to_zero() {
        let mut list = SelectList::new(items(&["a"]), "empty");
        list.retain(|_| false);
        assert!(list.is_empty());
        assert_eq!(list.selected_index(), 0);
    }

    #[test]
    fn items_mut_allows_mutation_but_not_length_change() {
        let mut list = SelectList::new(items(&["a", "b"]), "empty");
        list.items_mut()[0] = TestItem("x".to_string());
        assert_eq!(list.items()[0].0, "x");
        assert_eq!(list.len(), 2);
    }
}
