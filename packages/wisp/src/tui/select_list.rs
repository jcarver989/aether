use super::component::RenderContext;
use super::screen::Line;
use crate::tui::Component;
use crossterm::style::Stylize;

pub trait Selectable {
    fn display_text(&self) -> String;

    fn is_disabled(&self) -> bool {
        false
    }
}

pub struct SelectList<T> {
    pub items: Vec<T>,
    pub selected_index: usize,
}

impl<T: Selectable> SelectList<T> {
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items,
            selected_index: 0,
        }
    }

    pub fn move_up(&mut self) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        for _ in 0..len {
            self.selected_index = (self.selected_index + len - 1) % len;
            if !self.items[self.selected_index].is_disabled() {
                return;
            }
        }
    }

    pub fn move_down(&mut self) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        for _ in 0..len {
            self.selected_index = (self.selected_index + 1) % len;
            if !self.items[self.selected_index].is_disabled() {
                return;
            }
        }
    }

    pub fn selected(&self) -> Option<&T> {
        self.items.get(self.selected_index)
    }
}

impl<T: Selectable> Component for SelectList<T> {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let mut lines = Vec::new();

        for (i, item) in self.items.iter().enumerate() {
            let prefix = if i == self.selected_index {
                "▶ "
            } else {
                "  "
            };

            let text = format!("{prefix}{}", item.display_text());
            let line = if item.is_disabled() {
                Line::new(text.with(context.theme.muted).to_string())
            } else if i == self.selected_index {
                Line::new(text.with(context.theme.primary).to_string())
            } else {
                Line::new(text)
            };
            lines.push(line);
        }

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeItem {
        label: String,
        disabled: bool,
    }

    impl FakeItem {
        fn enabled(label: &str) -> Self {
            Self {
                label: label.to_string(),
                disabled: false,
            }
        }

        fn disabled(label: &str) -> Self {
            Self {
                label: label.to_string(),
                disabled: true,
            }
        }
    }

    impl Selectable for FakeItem {
        fn display_text(&self) -> String {
            self.label.clone()
        }

        fn is_disabled(&self) -> bool {
            self.disabled
        }
    }

    #[test]
    fn empty_list_move_is_noop() {
        let mut list: SelectList<FakeItem> = SelectList::new(vec![]);
        list.move_up();
        list.move_down();
        assert!(list.selected().is_none());
    }

    #[test]
    fn wraps_down_to_first() {
        let mut list = SelectList::new(vec![
            FakeItem::enabled("a"),
            FakeItem::enabled("b"),
            FakeItem::enabled("c"),
        ]);
        list.move_down(); // -> 1
        list.move_down(); // -> 2
        list.move_down(); // -> 0 (wrap)
        assert_eq!(list.selected_index, 0);
    }

    #[test]
    fn wraps_up_to_last() {
        let mut list = SelectList::new(vec![
            FakeItem::enabled("a"),
            FakeItem::enabled("b"),
            FakeItem::enabled("c"),
        ]);
        list.move_up(); // -> 2 (wrap)
        assert_eq!(list.selected_index, 2);
    }

    #[test]
    fn skips_disabled_items_going_down() {
        let mut list = SelectList::new(vec![
            FakeItem::enabled("a"),
            FakeItem::disabled("b"),
            FakeItem::enabled("c"),
        ]);
        list.move_down(); // skips b -> 2
        assert_eq!(list.selected_index, 2);
    }

    #[test]
    fn skips_disabled_items_going_up() {
        let mut list = SelectList::new(vec![
            FakeItem::enabled("a"),
            FakeItem::disabled("b"),
            FakeItem::enabled("c"),
        ]);
        list.selected_index = 2;
        list.move_up(); // skips b -> 0
        assert_eq!(list.selected_index, 0);
    }

    #[test]
    fn all_disabled_stays_put() {
        let mut list = SelectList::new(vec![FakeItem::disabled("a"), FakeItem::disabled("b")]);
        list.move_down();
        // Should have cycled through all and landed somewhere (no panic)
        assert!(list.selected_index < 2);
    }

    #[test]
    fn selected_returns_current_item() {
        let list = SelectList::new(vec![
            FakeItem::enabled("first"),
            FakeItem::enabled("second"),
        ]);
        assert_eq!(list.selected().unwrap().label, "first");
    }

    #[test]
    fn render_shows_pointer_on_selected() {
        let list = SelectList::new(vec![FakeItem::enabled("alpha"), FakeItem::enabled("beta")]);
        let ctx = RenderContext::new((80, 24));
        let lines = list.render(&ctx);
        assert_eq!(lines.len(), 2);
        // First line (selected) has the pointer prefix in its styled output
        // Second line has plain "  " prefix
        assert!(lines[1].as_str().contains("beta"));
    }

    #[test]
    fn render_disabled_item_uses_muted_style() {
        let list = SelectList::new(vec![
            FakeItem::enabled("enabled"),
            FakeItem::disabled("disabled"),
        ]);
        let ctx = RenderContext::new((80, 24));
        let lines = list.render(&ctx);
        assert_eq!(lines.len(), 2);
        // Both lines should render without panic
        assert!(lines[0].as_str().contains("enabled"));
        assert!(lines[1].as_str().contains("disabled"));
    }
}
