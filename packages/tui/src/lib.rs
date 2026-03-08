//! A lightweight, composable terminal UI framework.
//!
//! `tui` provides a set of building blocks for rich CLI applications:
//!
//! - **[`Component`]** — Stateful widgets that render to `Vec<Line>`.
//! - **[`HandlesInput`]** — Keyboard input handling with typed actions via [`InputOutcome`].
//! - **[`FocusRing`]** — Reusable focus tracking with Tab/`BackTab` cycling.
//! - **[`Screen`](screen::Screen)** / **[`Renderer`]** — Frame-diffing terminal output with cursor management.
//! - **[`Line`]**, **[`Span`](screen::Span)**, **[`Style`]** — Styled text primitives.
//! - **[`Theme`](theme::Theme)** — Semantic color palettes derived from `.tmTheme` files.
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

#[cfg(feature = "picker")]
pub mod test_picker;

pub mod checkbox;
#[cfg(feature = "picker")]
pub mod combobox;
pub mod component;
pub mod diff;
pub mod diff_types;
pub mod focus;
pub mod form;
#[cfg(feature = "markdown")]
pub mod markdown;
pub mod multi_select;
pub mod number_field;
pub mod radio_select;
pub mod renderer;
#[cfg(feature = "runtime")]
pub mod runtime;
pub mod screen;
pub mod select_option;
pub mod size;
pub mod soft_wrap;
pub mod spinner;
mod syntax;
pub mod text_field;
pub mod theme;

pub use checkbox::Checkbox;
#[cfg(feature = "picker")]
pub use combobox::{Combobox, PickerKey, Searchable, classify_key};
pub use component::{Component, HandlesInput, InputOutcome, RenderContext, Tickable};
pub use diff_types::{DiffLine, DiffPreview, DiffTag};
pub use focus::{FocusOutcome, FocusRing};
pub use form::{Form, FormAction, FormField, FormFieldKind};
pub use multi_select::MultiSelect;
pub use number_field::NumberField;
pub use radio_select::RadioSelect;
pub use renderer::{Cursor, CursorComponent, RenderOutput, Renderer};
#[cfg(feature = "runtime")]
pub use runtime::spawn_terminal_event_task;
pub use screen::{Line, Style};
pub use select_option::SelectOption;
pub use size::Size;
pub use text_field::TextField;
