//! A lightweight terminal UI rendering and widget library.
//!
//! `tui` provides composable primitives for building full-screen terminal
//! applications:
//!
//! - **[`Component`]** — Trait for reusable UI widgets with event handling
//! - **[`Frame`]** — A rendered frame of lines with cursor position
//! - **[`Renderer`](advanced::Renderer)** — Efficient diff-based terminal renderer
//! - **[`TerminalSession`](advanced::TerminalSession)** — Terminal lifecycle management

// Core modules - always available
pub(crate) mod components;
pub(crate) mod diffs;
pub(crate) mod focus;
pub(crate) mod rendering;
pub(crate) use rendering::line;
pub(crate) use rendering::span;
pub(crate) use rendering::style;
pub(crate) mod theme;

// Feature-gated modules
#[cfg(feature = "syntax")]
mod syntax_highlighting;

#[cfg(feature = "syntax")]
pub(crate) mod markdown;

#[cfg(feature = "picker")]
pub mod test_picker;

#[cfg(feature = "picker")]
pub(crate) mod combobox;

#[cfg(feature = "picker")]
pub(crate) mod fuzzy_matcher;

#[cfg(feature = "runtime")]
pub mod runtime;

#[cfg(feature = "testing")]
pub mod testing;

// Core re-exports - always available
pub use components::checkbox::Checkbox;
pub use components::form::{Form, FormField, FormFieldKind, FormMessage};
pub use components::layout::Layout;
pub use components::multi_select::MultiSelect;
pub use components::number_field::NumberField;
pub use components::panel::{BORDER_H_PAD, Panel};
pub use components::radio_select::RadioSelect;
pub use components::select_list::{SelectItem, SelectList, SelectListMessage};
pub use components::select_option::SelectOption;
pub use components::spinner::{BRAILLE_FRAMES, Spinner};
pub use components::text_field::TextField;

pub use components::{Component, Cursor, Event, PickerMessage, ViewContext, merge, wrap_selection};
pub use diffs::diff_types::{DiffLine, DiffPreview, DiffTag};
pub use focus::{FocusOutcome, FocusRing};
pub use rendering::frame::Frame;
pub use rendering::line::Line;
pub use rendering::style::Style;
pub use theme::Theme;

/// Advanced APIs for low-level terminal control.
#[cfg(feature = "runtime")]
pub mod advanced {
    /// Low-level renderer for manual frame control.
    pub use crate::rendering::renderer::{Renderer, RendererCommand};

    /// Terminal session management for manual runtime control.
    pub use crate::runtime::{MouseCapture, TerminalSession, spawn_terminal_event_task};

    /// Direct terminal size query helper.
    pub use crate::runtime::terminal::terminal_size;

    /// Raw crossterm event type returned by [`spawn_terminal_event_task`].
    pub use crossterm::event::Event as CrosstermEvent;
}

// &str text utilities
pub use rendering::soft_wrap::{display_width_text, pad_text_to_width, truncate_text};

// Span type
pub use rendering::span::Span;

// Markdown
#[cfg(feature = "syntax")]
pub use markdown::render_markdown;

// Feature-gated re-exports
#[cfg(feature = "syntax")]
pub use diffs::diff::highlight_diff;

#[cfg(feature = "syntax")]
pub use syntax_highlighting::SyntaxHighlighter;

#[cfg(feature = "picker")]
pub use combobox::{Combobox, PickerKey, Searchable, classify_key};

#[cfg(feature = "picker")]
pub use fuzzy_matcher::FuzzyMatcher;

// Terminal event types (re-exported from crossterm)
pub use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseEvent, MouseEventKind,
};
pub use crossterm::style::Color;
