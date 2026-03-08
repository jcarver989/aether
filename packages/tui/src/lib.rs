//! A lightweight, composable terminal UI framework.
//!
//! `tui` provides both low-level rendering primitives and a higher-level runtime
//! entrypoint for rich CLI applications:
//!
//! - **[`Component`]** — Stateful widgets that render to `Vec<Line>`.
//! - **[`InteractiveComponent`]** — Keyboard input handling with typed actions via [`KeyEventResponse`].
//! - **[`FocusRing`]** — Reusable focus tracking with Tab/`BackTab` cycling.
//! - **[`TerminalScreen`]** / **[`Renderer`]** — Frame-diffing terminal output with cursor management.
//! - **[`runtime::run_app`]** — Terminal lifecycle, event loop, ticks, external events, effects, and cleanup.
//! - **[`Line`]**, **[`Span`](span::Span)**, **[`Style`]** — Styled text primitives.
//! - **[`Theme`](theme::Theme)** — Semantic color palettes.
//!
//! # Quick start
//!
//! ```rust
//! use tui::{Component, Line, RenderContext};
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
//! For a higher-level application bootstrap path, see [`runtime::run_app`].
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
pub mod component;
pub mod components;
pub mod diffs;
pub use diffs::diff_types;
pub mod focus;
pub mod rendering;
pub use rendering::frame;
pub use rendering::line;
pub use rendering::prepared_frame;
pub use rendering::render_context;
pub use rendering::renderer;
pub use rendering::size;
pub use rendering::soft_wrap;
pub use rendering::span;
pub use rendering::style;
pub use rendering::terminal_screen;
pub mod theme;

// Feature-gated modules
#[cfg(feature = "syntax")]
mod syntax;

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
pub use component::{
    Component, Cursor, InteractiveComponent, KeyEventResponse, RenderContext, RootComponent,
    TickableComponent,
};
pub use components::checkbox::Checkbox;
pub use components::form::{Form, FormAction, FormField, FormFieldKind};
pub use components::multi_select::MultiSelect;
pub use components::number_field::NumberField;
pub use components::radio_select::RadioSelect;
pub use components::select_option::SelectOption;
pub use components::spinner::{BRAILLE_FRAMES, Spinner};
pub use components::text_field::TextField;
pub use diffs::diff_types::{DiffLine, DiffPreview, DiffTag};
pub use focus::{FocusOutcome, FocusRing};
pub use rendering::frame::Frame;
pub use rendering::line::Line;
pub use rendering::prepared_frame::PreparedFrame;
pub use rendering::renderer::Renderer;
pub use rendering::size::Size;
pub use rendering::style::Style;
pub use rendering::terminal_screen::TerminalScreen;
pub use theme::{ColorPalette, Theme};

// Feature-gated re-exports
#[cfg(feature = "diff")]
pub use diffs::diff::highlight_diff;

#[cfg(feature = "picker")]
pub use combobox::{Combobox, PickerKey, Searchable, classify_key};

#[cfg(feature = "runtime")]
pub use runtime::{
    RuntimeAction, RuntimeApp, RuntimeEvent, RuntimeOptions, run_app, spawn_terminal_event_task,
};
