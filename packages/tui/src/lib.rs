//! A lightweight, composable terminal UI framework.
//!
//! `tui` provides a simple, opinionated app model for building full-screen terminal
//! applications:
//!
//! - **[`App`]** — Single trait combining event handling, effects, and rendering
//! - **[`AppEvent`]** — Unified event type for terminal, external, and tick events
//! - **[`Response`]** — Unified result type for event handling and effects
//! - **[`Runner`]** — Builder-style runner that owns terminal lifecycle

// Core modules - always available
pub(crate) mod components;
pub(crate) mod diffs;
pub(crate) mod focus;
pub(crate) mod rendering;
pub(crate) use rendering::line;
pub(crate) use rendering::size;
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

#[cfg(feature = "runtime")]
pub mod runtime;

#[cfg(feature = "testing")]
pub mod testing;

// Core re-exports - always available
pub use components::checkbox::Checkbox;
pub use components::layout::Layout;
pub use components::panel::{BORDER_H_PAD, Panel};
pub use components::form::{Form, FormField, FormFieldKind, FormMessage};
pub use components::multi_select::MultiSelect;
pub use components::number_field::NumberField;
pub use components::radio_select::RadioSelect;
pub use components::select_option::SelectOption;
pub use components::spinner::{BRAILLE_FRAMES, Spinner};
pub use components::text_field::TextField;

pub use components::{
    Cursor, PickerMessage, Response, ViewContext, Widget, WidgetEvent, wrap_selection,
};
pub use diffs::diff_types::{DiffLine, DiffPreview, DiffTag};
pub use focus::{FocusOutcome, FocusRing};
pub use rendering::frame::Frame;
pub use rendering::line::Line;
pub use rendering::size::Size;
pub use rendering::style::Style;
pub use theme::Theme;

/// Advanced APIs for users who need low-level control.
///
/// Most applications should use the [`App`] trait with [`Runner`] instead.
#[cfg(feature = "runtime")]
pub mod advanced {
    /// Low-level renderer for manual frame control.
    pub use crate::rendering::renderer::Renderer;

    /// Narrowed handle for terminal operations during effect execution.
    pub use crate::rendering::renderer::Terminal;

    /// Prepared frame representation used by low-level rendering tests and internals.
    pub use crate::rendering::prepared_frame::PreparedFrame;

    /// Terminal frame-diffing screen implementation.
    pub use crate::rendering::terminal_screen::TerminalScreen;

    /// Terminal session management for manual runtime control.
    pub use crate::runtime::{MouseCapture, TerminalSession, spawn_terminal_event_task};

    /// Direct terminal size query helper.
    pub use crate::runtime::terminal::terminal_size;
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

// Terminal event types (re-exported from crossterm)
pub use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseEvent, MouseEventKind,
};
pub use crossterm::style::Color;

#[cfg(feature = "runtime")]
pub use runtime::{App, AppEvent, Runner, run};
