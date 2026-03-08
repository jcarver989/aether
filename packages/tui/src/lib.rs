//! A lightweight, composable terminal UI framework.
//!
//! `tui` provides a set of building blocks for rich CLI applications:
//!
//! - **[`Component`]** — Stateful widgets that render to `Vec<Line>`.
//! - **[`HandlesInput`]** — Keyboard input handling with typed actions via [`InputOutcome`].
//! - **[`FocusRing`]** — Reusable focus tracking with Tab/`BackTab` cycling.
//! - **[`Screen`]** / **[`Renderer`]** — Frame-diffing terminal output with cursor management.
//! - **[`Line`]**, **[`Span`](screen::Span)**, **[`Style`]** — Styled text primitives.
//! - **[`Theme`](theme::Theme)** — Semantic color palettes.
//!
//! # Quick start
//!
//! ```rust
//! use tui::{Component, RenderContext, Line};
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
//! - Core components (`Component`, `HandlesInput`, `RenderContext`)
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
pub use rendering::renderer;
pub use rendering::screen;
pub use rendering::size;
pub use rendering::soft_wrap;
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
pub use component::{Component, HandlesInput, InputOutcome, RenderContext, Tickable};
pub use components::checkbox::Checkbox;
pub use components::form::{Form, FormAction, FormField, FormFieldKind};
pub use components::multi_select::MultiSelect;
pub use components::number_field::NumberField;
pub use components::radio_select::RadioSelect;
pub use components::spinner::{Spinner, BRAILLE_FRAMES};
pub use components::text_field::TextField;
pub use diffs::diff_types::{DiffLine, DiffPreview, DiffTag};
pub use focus::{FocusOutcome, FocusRing};
pub use rendering::renderer::{Cursor, CursorComponent, RenderOutput, Renderer};
pub use rendering::screen::{Line, Style};
pub use components::select_option::SelectOption;
pub use rendering::size::Size;
pub use theme::Theme;

// Feature-gated re-exports
#[cfg(feature = "diff")]
pub use diffs::diff::highlight_diff;

#[cfg(feature = "picker")]
pub use combobox::{Combobox, PickerKey, Searchable, classify_key};

#[cfg(feature = "runtime")]
pub use runtime::spawn_terminal_event_task;
