use crate::line::Line;
pub use crate::rendering::frame::{Cursor, Frame};
pub use crate::rendering::render_context::RenderContext;
use crossterm::event::KeyEvent;
use std::time::Instant;

/// A component that renders a full frame with cursor information for the [`Renderer`](crate::Renderer).
pub trait RootComponent {
    fn render(&mut self, context: &RenderContext) -> Frame;
}

/// A stateful widget that can render itself as styled terminal lines.
pub trait Component {
    fn render(&self, context: &RenderContext) -> Vec<Line>;
}

/// A component with time-based animation state.
pub trait TickableComponent {
    /// Advance animation state by one tick.
    fn on_tick(&mut self, now: Instant);
}

/// A component that can process keyboard input and emit typed actions.
pub trait InteractiveComponent {
    type Action;

    fn on_key_event(&mut self, key_event: KeyEvent) -> KeyEventResponse<Self::Action>;
}

/// Result of handling a key event.
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
    /// `KeyEventResponse<A>` and `KeyEventResponse<B>`.
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
