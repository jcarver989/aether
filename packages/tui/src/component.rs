use crate::line::Line;
pub use crate::rendering::render_context::RenderContext;
use crossterm::event::KeyEvent;
use std::time::Instant;

/// A stateful widget that can render itself as styled terminal lines.
pub trait Component {
    fn render(&self, context: &RenderContext) -> Vec<Line>;
}

/// A component with time-based animation state.
pub trait TickableComponent {
    /// Advance animation state by one tick.
    fn on_tick(&mut self, now: Instant);
}

/// Logical cursor position within a component's rendered output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub logical_row: usize,
    pub col: usize,
}

/// The output of [`CursorComponent::render`]: rendered lines plus cursor state.
pub struct RenderOutput {
    pub lines: Vec<Line>,
    pub cursor: Cursor,
    pub cursor_visible: bool,
}

/// A component that renders with cursor position information for the [`Renderer`](crate::Renderer).
pub trait CursorComponent {
    fn render(&mut self, context: &RenderContext) -> RenderOutput;
}

/// A component that can process keyboard input and emit typed actions.
pub trait InteractiveComponent {
    type Action;

    fn on_key_event(&mut self, key_event: KeyEvent) -> KeyEventResponse<Self::Action>;
}

/// Result of handling a key event via [`HandlesInput`].
///
/// - `consumed` — whether the key was handled (prevents further propagation).
/// - `action` — an optional typed action emitted to the parent.
pub struct KeyEventResponse<A> {
    pub consumed: bool,
    pub action: Option<A>,
}

impl<A> KeyEventResponse<A> {
    /// Transform the action type, preserving `consumed`.
    pub fn map<B>(self, f: impl FnOnce(A) -> B) -> KeyEventResponse<B> {
        KeyEventResponse {
            consumed: self.consumed,
            action: self.action.map(f),
        }
    }

    /// Discard the action, preserving `consumed`.
    ///
    /// The output type is inferred from context, so this can convert between
    /// `InputOutcome<A>` and `InputOutcome<B>`.
    pub fn discard_action<B>(self) -> KeyEventResponse<B> {
        KeyEventResponse {
            consumed: self.consumed,
            action: None,
        }
    }

    pub fn ignored() -> Self {
        Self {
            consumed: false,
            action: None,
        }
    }

    pub fn consumed() -> Self {
        Self {
            consumed: true,
            action: None,
        }
    }

    pub fn action(action: A) -> Self {
        Self {
            consumed: true,
            action: Some(action),
        }
    }
}
