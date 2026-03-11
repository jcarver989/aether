use crossterm::event::{KeyEvent, MouseEvent};

use crate::line::Line;
use crate::rendering::render_context::ViewContext;

/// Events that a [`Widget`] can handle.
pub enum Event {
    Key(KeyEvent),
    Paste(String),
    Mouse(MouseEvent),
    Tick,
}

/// A component that can process events and emit typed messages.
pub trait Component {
    /// The message type emitted by this widget.
    type Message;

    /// Process an event and return the outcome.
    ///
    /// - `None` — event not recognized, propagate to parent
    /// - `Some(vec![])` — event consumed, no messages
    /// - `Some(vec![msg, ...])` — event consumed, emit messages
    fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>>;

    /// Render the current state to lines.
    fn render(&self, ctx: &ViewContext) -> Vec<Line>;
}

/// Merge two event outcomes. `None` (ignored) yields to the other.
/// Messages are concatenated in order.
pub fn merge<M>(a: Option<Vec<M>>, b: Option<Vec<M>>) -> Option<Vec<M>> {
    match (a, b) {
        (None, other) | (other, None) => other,
        (Some(mut a), Some(b)) => {
            a.extend(b);
            Some(a)
        }
    }
}

/// Generic message type for picker components.
pub enum PickerMessage<T> {
    Close,
    CloseAndPopChar,
    CloseWithChar(char),
    Confirm(T),
    CharTyped(char),
    PopChar,
}
