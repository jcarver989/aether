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

    /// Show the autocomplete with the given filter text and items.
    ///
    /// For async item loading, pass an empty vec and call `set_items()` when results arrive.
    pub fn show(&mut self, filter_text: String, items: Vec<T>) {
        self.filter.set(filter_text);
        self.items.set(items);
        self.selected_index.set(0);
        self.visible.set(true);
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
}
