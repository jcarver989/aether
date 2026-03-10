//! A lightweight, composable terminal UI framework.
//!
//! `tui` provides a simple, opinionated app model for building full-screen terminal
//! applications:
//!
//! - **[`App`]** — Single trait combining event handling, effects, and rendering
//! - **[`AppEvent`]** — Unified event type for terminal, external, and tick events
//! - **[`Effects`]** — Effect result type for commands and exit
//! - **[`Runner`]** — Builder-style runner that owns terminal lifecycle
//!
//! # Quick start
//!
//! Build an app by implementing the [`App`] trait with `update` and `view` methods:
//!
//! ```rust
//! use tui::{App, AppEvent, Effects, Frame, Line, RenderContext, Runner, Cursor};
//! use tui::{KeyEvent, KeyCode};
//!
//! struct Counter { count: i32 }
//!
//! impl App for Counter {
//!     type Event = ();
//!     type Effect = ();
//!     type Error = std::io::Error;
//!
//!     fn update(&mut self, event: AppEvent<()>, _ctx: &RenderContext) -> Effects<()> {
//!         match event {
//!             AppEvent::Key(key) if key.code == KeyCode::Char('q') => Effects::exit(),
//!             AppEvent::Key(key) if key.code == KeyCode::Char('j') => {
//!                 self.count += 1;
//!                 Effects::none()
//!             }
//!             AppEvent::Key(key) if key.code == KeyCode::Char('k') => {
//!                 self.count -= 1;
//!                 Effects::none()
//!             }
//!             _ => Effects::none(),
//!         }
//!     }
//!
//!     fn view(&self, _ctx: &RenderContext) -> Frame {
//!         Frame::new(
//!             vec![Line::new(format!("Count: {} (j/k to change, q to quit)", self.count))],
//!             Cursor { row: 0, col: 0, is_visible: false },
//!         )
//!     }
//! }
//! ```
//!
//! Run with default settings:
//!
//! ```rust,no_run
//! # use tui::{App, AppEvent, Effects, Frame, Line, RenderContext, Runner, Cursor};
//! # use tui::{KeyEvent, KeyCode};
//! # struct Counter { count: i32 }
//! # impl App for Counter {
//! #     type Event = ();
//! #     type Effect = ();
//! #     type Error = std::io::Error;
//! #     fn update(&mut self, event: AppEvent<()>, _ctx: &RenderContext) -> Effects<()> { Effects::none() }
//! #     fn view(&self, _ctx: &RenderContext) -> Frame { Frame::new(vec![], Cursor { row: 0, col: 0, is_visible: false }) }
//! # }
//! # async fn example() -> Result<(), std::io::Error> {
//! Runner::new(Counter { count: 0 }).run().await
//! # }
//! ```
//!
//! # Building reusable widgets
//!
//! For child components and reusable widgets, use [`Component`] and [`InteractiveComponent`]:
//!
//! ```rust
//! use tui::{Component, InteractiveComponent, MessageResult, Line, RenderContext};
//!
//! struct Greeting { name: String }
//!
//! impl Component for Greeting {
//!     fn render(&self, _ctx: &RenderContext) -> Vec<Line> {
//!         vec![Line::new(format!("Hello, {}!", self.name))]
//!     }
//! }
//! ```
//!
//! # Other exports
//!
//! - **[`Container`]** — Bordered panel for stacking content blocks with title/footer
//! - **[`FocusRing`]** / **[`FocusGroup`]** — Focus tracking with Tab/BackTab cycling
//! - **[`wrap_selection`]** — Helper for navigating selection indices with wrap-around
//! - **[`Line`]**, **[`Span`](span::Span)**, **[`Style`]** — Styled text primitives
//! - **[`Theme`](theme::Theme)** — Semantic color palettes
//! - **[`advanced::Renderer`]** — Frame-diffing terminal output with manual control
//!
//! Supporting widgets (available but not core):
//! - **[`Dialog`]** — Confirmation dialog with focusable buttons
//! - **[`StatusBar`]** — Status line with left/right sections
//!
//! # Advanced APIs
//!
//! If you need manual renderer control or terminal session management,
//! use items from [`advanced`].
//!
//! - **[`advanced::Renderer`]** — Manual frame-diffing renderer control
//! - **[`advanced::TerminalSession`]** / **[`advanced::MouseCapture`]** — Terminal lifecycle helpers
//!
//! # Feature Flags
//!
//! The crate uses feature flags to reduce compile time and binary size:
//!
//! | Feature | Description | Default |
//! |---------|-------------|---------|
//! | `syntax` | Syntax highlighting via syntect | ✅ |
//! | `markdown` | Markdown rendering (implies syntax) | ✅ |
//! | `diff` | Diff preview rendering (implies syntax) | ✅ |
//! | `serde` | `to_json()` methods for form widgets | ✅ |
//! | `runtime` | Async terminal event handling via tokio | ✅ |
//! | `picker` | Fuzzy search/picker in combobox | ✅ |
//!
//! ## Minimal Installation
//!
//! For a minimal footprint, disable default features:
//!
//! ```toml
//! [dependencies]
//! tui = { version = "0.1", default-features = false }
//! ```
//!
//! This gives you:
//! - Core components (`Component`, `InteractiveComponent`, `RenderContext`)
//! - Form widgets (`TextField`, `Checkbox`, `NumberField`, etc.)
//! - Focus management (`FocusRing`)
//! - Rendering primitives (`Line`, `Span`, `Style`, `Theme`)
//!
//! ## Adding Features
//!
//! ```toml
//! # Just form serialization
//! tui = { version = "0.1", default-features = false, features = ["serde"] }
//!
//! # Everything except async runtime
//! tui = { version = "0.1", default-features = false, features = ["markdown", "diff", "serde", "picker"] }
//! ```

// Core modules - always available
pub mod components;
pub mod diffs;
pub use diffs::diff_types;
pub mod focus;
pub mod rendering;
pub(crate) use rendering::line;
pub(crate) use rendering::size;
pub(crate) use rendering::span;
pub(crate) use rendering::style;
pub mod theme;

// Feature-gated modules
#[cfg(feature = "syntax")]
mod syntax_highlighting;

#[cfg(feature = "markdown")]
pub mod markdown;

#[cfg(feature = "diff")]
pub use diffs::diff;

#[cfg(feature = "picker")]
pub mod test_picker;

#[cfg(feature = "picker")]
pub mod combobox;

#[cfg(feature = "runtime")]
pub mod runtime;

#[cfg(feature = "testing")]
pub mod testing;

// Core re-exports - always available
pub use components::checkbox::Checkbox;
pub use components::container::{BORDER_H_PAD, Container};
pub use components::form::{Form, FormField, FormFieldKind, FormMessage};
pub use components::multi_select::MultiSelect;
pub use components::number_field::NumberField;
pub use components::radio_select::RadioSelect;
pub use components::select_option::SelectOption;
pub use components::spinner::{BRAILLE_FRAMES, Spinner};
pub use components::text_field::TextField;

// Supporting widgets - available but not part of the core happy path.
// Access via `tui::Dialog`, `tui::StatusBar`, or `tui::components::dialog` / `tui::components::status_bar`.
pub use components::dialog::{Dialog, DialogMessage};
pub use components::status_bar::StatusBar;
pub use components::{
    Component, Cursor, InteractiveComponent, MessageResult, RenderContext, UiEvent, wrap_selection,
};
pub use diffs::diff_types::{DiffLine, DiffPreview, DiffTag};
pub use focus::{FocusGroup, FocusOutcome, FocusRing, NavigationResult};
pub use rendering::frame::Frame;
pub use rendering::line::Line;
pub use rendering::size::Size;
pub use rendering::style::Style;
pub use theme::{ColorPalette, Theme};

/// Advanced APIs for users who need low-level control.
///
/// Most applications should use the [`App`] trait with [`Runner`] instead.
#[cfg(feature = "runtime")]
pub mod advanced {
    /// Low-level renderer for manual frame control.
    pub use crate::rendering::renderer::Renderer;

    /// Prepared frame representation used by low-level rendering tests and internals.
    pub use crate::rendering::prepared_frame::PreparedFrame;

    /// Terminal frame-diffing screen implementation.
    pub use crate::rendering::terminal_screen::TerminalScreen;

    /// Terminal session management for manual runtime control.
    pub use crate::runtime::{MouseCapture, TerminalSession, spawn_terminal_event_task};

    /// Direct terminal size query helper.
    pub use crate::runtime::terminal::terminal_size;
}

// Feature-gated re-exports
#[cfg(feature = "diff")]
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
pub use runtime::{App, AppEvent, Effects, Runner, run};
