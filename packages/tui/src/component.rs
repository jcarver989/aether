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

/// Events that can be processed by an [`InteractiveComponent`].
#[derive(Debug, Clone)]
pub enum UiEvent {
    /// A keyboard event.
    Key(KeyEvent),
    /// Pasted text from bracketed paste mode.
    Paste(String),
    /// A tick event for time-based updates.
    Tick(Instant),
}

/// Result of handling a [`UiEvent`] in an [`InteractiveComponent`].
///
/// - `handled` — whether the event was consumed (prevents further propagation).
/// - `render` — whether a render should be enqueued even without messages.
/// - `messages` — typed messages emitted upward to the parent.
pub struct MessageResult<T> {
    pub handled: bool,
    pub render: bool,
    pub messages: Vec<T>,
}

impl<T> MessageResult<T> {
    /// The event was not recognized and should propagate.
    pub fn ignored() -> Self {
        Self {
            handled: false,
            render: false,
            messages: Vec::new(),
        }
    }

    /// The event was consumed, no messages, no render needed.
    pub fn consumed() -> Self {
        Self {
            handled: true,
            render: false,
            messages: Vec::new(),
        }
    }

    /// Request a render without emitting messages.
    pub fn render() -> Self {
        Self {
            handled: true,
            render: true,
            messages: Vec::new(),
        }
    }

    /// Emit a single message.
    pub fn message(message: T) -> Self {
        Self {
            handled: true,
            render: false,
            messages: vec![message],
        }
    }

    /// Emit multiple messages.
    pub fn messages(messages: Vec<T>) -> Self {
        Self {
            handled: true,
            render: false,
            messages,
        }
    }

    /// Request a render in addition to current state.
    pub fn with_render(mut self) -> Self {
        self.render = true;
        self
    }

    /// Transform message types, preserving `handled` and `render`.
    pub fn map<U>(self, f: impl FnMut(T) -> U) -> MessageResult<U> {
        MessageResult {
            handled: self.handled,
            render: self.render,
            messages: self.messages.into_iter().map(f).collect(),
        }
    }

    /// Discard messages, preserving `handled` and `render`.
    ///
    /// The output type is inferred from context, so this can convert between
    /// `MessageResult<M>` and `MessageResult<N>`.
    pub fn discard_messages<U>(self) -> MessageResult<U> {
        MessageResult {
            handled: self.handled,
            render: self.render,
            messages: Vec::new(),
        }
    }

    /// Merge two results.
    ///
    /// - `handled = self.handled || other.handled`
    /// - `render = self.render || other.render`
    /// - messages are appended in order (`self.messages` before `other.messages`)
    pub fn merge(mut self, other: Self) -> Self {
        self.handled = self.handled || other.handled;
        self.render = self.render || other.render;
        self.messages.extend(other.messages);
        self
    }
}

/// A component that can process [`UiEvent`]s and emit typed messages.
pub trait InteractiveComponent: Component {
    /// The message type emitted by this component.
    type Message;

    /// Process an event and return the result.
    fn on_event(&mut self, event: UiEvent) -> MessageResult<Self::Message>;
}
