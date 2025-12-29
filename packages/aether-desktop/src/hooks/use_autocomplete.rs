//! Generic autocomplete hook for dropdown/typeahead UI.
//!
//! Handles keyboard navigation, selection state, and visibility.
//! Can be used for slash commands, file pickers, search results, etc.

use dioxus::prelude::*;

/// Controller for autocomplete dropdown state and behavior.
///
/// Generic over the item type `T` (e.g., `SlashCommand`, `FileMatch`).
/// Manages visibility, selection index, and keyboard navigation.
///
/// This struct only contains `Signal` handles which are cheap to clone/copy.
/// We manually implement `Copy` because the derive macro requires `T: Copy`,
/// but `Signal<T>` is always `Copy` regardless of `T`.
pub struct AutocompleteController<T: Clone + PartialEq + 'static> {
    /// Whether the dropdown is visible
    visible: Signal<bool>,
    /// Current filter/search text
    filter: Signal<String>,
    /// Currently selected index in the items list
    selected_index: Signal<usize>,
    /// Items to display (typically filtered externally)
    items: Signal<Vec<T>>,
}

// Manual impls because derive requires T: Copy/Clone/PartialEq, but Signal<T> has these unconditionally

impl<T: Clone + PartialEq + 'static> Clone for AutocompleteController<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Clone + PartialEq + 'static> Copy for AutocompleteController<T> {}

impl<T: Clone + PartialEq + 'static> PartialEq for AutocompleteController<T> {
    fn eq(&self, other: &Self) -> bool {
        self.visible == other.visible
            && self.filter == other.filter
            && self.selected_index == other.selected_index
            && self.items == other.items
    }
}

impl<T: Clone + PartialEq + 'static> std::fmt::Debug for AutocompleteController<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutocompleteController")
            .field("visible", &self.is_visible())
            .field("filter", &self.filter())
            .field("selected_index", &self.selected_index())
            .field("items_count", &self.items.read().len())
            .finish()
    }
}

impl<T: Clone + PartialEq + 'static> AutocompleteController<T> {
    /// Create a new autocomplete controller with default state.
    pub fn new() -> Self {
        Self {
            visible: Signal::new(false),
            filter: Signal::new(String::new()),
            selected_index: Signal::new(0),
            items: Signal::new(Vec::new()),
        }
    }

    /// Whether the autocomplete dropdown is visible.
    pub fn is_visible(&self) -> bool {
        *self.visible.read()
    }

    /// Current filter text.
    pub fn filter(&self) -> String {
        self.filter.read().clone()
    }

    /// Current selected index.
    pub fn selected_index(&self) -> usize {
        *self.selected_index.read()
    }

    /// Get a clone of the current items.
    pub fn items(&self) -> Vec<T> {
        self.items.read().clone()
    }

    /// Get the currently selected item, if any.
    pub fn selected_item(&self) -> Option<T> {
        let items = self.items.read();
        let idx = *self.selected_index.read();
        items.get(idx).cloned()
    }

    /// Show the autocomplete with the given filter text and items.
    ///
    /// For async item loading, pass an empty vec and call `set_items()` when results arrive.
    pub fn show(&mut self, filter_text: String, items: Vec<T>) {
        self.filter.set(filter_text);
        self.items.set(items);
        self.selected_index.set(0);
        self.visible.set(true);
    }

    /// Hide the autocomplete and reset state.
    pub fn hide(&mut self) {
        self.visible.set(false);
        self.filter.set(String::new());
        self.selected_index.set(0);
    }

    /// Update the items list (e.g., after filtering or async search).
    pub fn set_items(&mut self, new_items: Vec<T>) {
        self.items.set(new_items);
        // Clamp selection to valid range
        let len = self.items.read().len();
        if *self.selected_index.read() >= len && len > 0 {
            self.selected_index.set(len - 1);
        }
    }

    /// Move selection to the next item.
    pub fn select_next(&mut self) {
        let len = self.items.read().len();
        if len == 0 {
            return;
        }
        let current = *self.selected_index.read();
        self.selected_index.set((current + 1).min(len - 1));
    }

    /// Move selection to the previous item.
    pub fn select_previous(&mut self) {
        let current = *self.selected_index.read();
        self.selected_index.set(current.saturating_sub(1));
    }

    /// Confirm the current selection and hide.
    ///
    /// Returns the selected item if one exists.
    pub fn confirm(&mut self) -> Option<T> {
        let item = self.selected_item();
        self.hide();
        item
    }

    /// Handle a keyboard event.
    ///
    /// Returns `Some(item)` if selection was confirmed, `None` otherwise.
    /// Returns `KeyAction` indicating what happened.
    pub fn handle_key(&mut self, key: &dioxus::prelude::Key) -> KeyAction<T> {
        if !self.is_visible() {
            return KeyAction::Ignored;
        }

        match key {
            Key::ArrowDown => {
                self.select_next();
                KeyAction::Consumed
            }
            Key::ArrowUp => {
                self.select_previous();
                KeyAction::Consumed
            }
            Key::Enter | Key::Tab => {
                if let Some(item) = self.confirm() {
                    KeyAction::Selected(item)
                } else {
                    KeyAction::Consumed
                }
            }
            Key::Escape => {
                self.hide();
                KeyAction::Consumed
            }
            _ => KeyAction::Ignored,
        }
    }
}

/// Result of handling a keyboard event.
#[derive(Clone, Debug, PartialEq)]
pub enum KeyAction<T> {
    /// Key was not handled by autocomplete
    Ignored,
    /// Key was consumed (navigation, escape)
    Consumed,
    /// An item was selected
    Selected(T),
}

impl<T> KeyAction<T> {
    /// Whether the key event was handled.
    #[allow(dead_code)]
    pub fn was_handled(&self) -> bool {
        !matches!(self, KeyAction::Ignored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_action_was_handled() {
        assert!(!KeyAction::<String>::Ignored.was_handled());
        assert!(KeyAction::<String>::Consumed.was_handled());
        assert!(KeyAction::Selected("test".to_string()).was_handled());
    }

    #[test]
    fn test_key_action_equality() {
        assert_eq!(KeyAction::<String>::Ignored, KeyAction::Ignored);
        assert_eq!(KeyAction::<String>::Consumed, KeyAction::Consumed);
        assert_eq!(
            KeyAction::Selected("a".to_string()),
            KeyAction::Selected("a".to_string())
        );
        assert_ne!(
            KeyAction::Selected("a".to_string()),
            KeyAction::Selected("b".to_string())
        );
        assert_ne!(KeyAction::<String>::Ignored, KeyAction::Consumed);
    }

    #[test]
    fn test_key_action_clone() {
        let action = KeyAction::Selected("test".to_string());
        let cloned = action.clone();
        assert_eq!(action, cloned);
    }

    #[test]
    fn test_key_action_debug() {
        let ignored = KeyAction::<String>::Ignored;
        let consumed = KeyAction::<String>::Consumed;
        let selected = KeyAction::Selected("item".to_string());

        assert!(format!("{:?}", ignored).contains("Ignored"));
        assert!(format!("{:?}", consumed).contains("Consumed"));
        assert!(format!("{:?}", selected).contains("Selected"));
        assert!(format!("{:?}", selected).contains("item"));
    }
}
