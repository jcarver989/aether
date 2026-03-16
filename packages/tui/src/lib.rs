//! A lightweight terminal UI rendering and widget library.
//!
//! `tui` provides composable primitives for building full-screen terminal
//! applications:
//!
//! - **[`Component`]** — Trait for reusable UI widgets with event handling
//! - **[`Frame`]** — A rendered frame of lines with cursor position
//! - **[`Renderer`]** — Efficient diff-based terminal renderer
//! - **[`TerminalSession`]** — Terminal lifecycle management

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

#[cfg(all(feature = "picker", feature = "testing"))]
pub mod test_picker;

#[cfg(feature = "picker")]
pub(crate) mod combobox;

#[cfg(feature = "picker")]
pub(crate) mod fuzzy_matcher;

pub(crate) mod runtime;

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
pub use diffs::diff_types::{DiffLine, DiffPreview, DiffTag, SplitDiffCell, SplitDiffRow};
pub use focus::{FocusOutcome, FocusRing};
pub use rendering::frame::Frame;
pub use rendering::line::Line;
pub use rendering::style::Style;
pub use theme::{Theme, ThemeBuildError};

// Rendering (always available - no runtime dependency)
pub use rendering::renderer::{Renderer, RendererCommand};

// Runtime
pub use crossterm::event::Event as CrosstermEvent;
pub use runtime::terminal::terminal_size;
pub use runtime::{MouseCapture, TerminalSession, spawn_terminal_event_task};

// &str text utilities
pub use rendering::soft_wrap::{
    display_width_text, pad_text_to_width, truncate_line, truncate_text,
};

// Span type
pub use rendering::span::Span;

// Markdown
#[cfg(feature = "syntax")]
pub use markdown::render_markdown;

// Feature-gated re-exports
#[cfg(feature = "syntax")]
pub use diffs::diff::highlight_diff;

#[cfg(feature = "syntax")]
pub use diffs::split_diff::render_diff;

#[cfg(feature = "syntax")]
pub use syntax_highlighting::SyntaxHighlighter;

#[cfg(feature = "picker")]
pub use combobox::{Combobox, PickerKey, classify_key};
#[cfg(feature = "picker")]
pub use fuzzy_matcher::Searchable;

#[cfg(feature = "picker")]
pub use fuzzy_matcher::FuzzyMatcher;

// Terminal event types (re-exported from crossterm)
pub use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseEvent, MouseEventKind,
};
pub use crossterm::style::Color;
