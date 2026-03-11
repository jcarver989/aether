use crossterm::event::{KeyEvent, MouseEvent};

use crate::line::Line;
use crate::rendering::render_context::ViewContext;

/// Events that a [`Widget`] can handle.
pub enum WidgetEvent {
    Key(KeyEvent),
    Paste(String),
    Mouse(MouseEvent),
    Tick,
}

/// Result of handling an event in a [`Widget`].
pub enum Outcome<M> {
    /// Event not recognized, propagate to parent.
    Ignored,
    /// Event consumed, no messages.
    Consumed,
    /// Event consumed, emit one message.
    Message(M),
    /// Event consumed, emit multiple messages.
    Messages(Vec<M>),
}

impl<M> Outcome<M> {
    /// The event was not recognized and should propagate.
    pub fn ignored() -> Self {
        Self::Ignored
    }

    /// The event was consumed, no messages.
    pub fn consumed() -> Self {
        Self::Consumed
    }

    /// Emit a single message.
    pub fn message(msg: M) -> Self {
        Self::Message(msg)
    }

    /// Emit multiple messages.
    pub fn messages(msgs: Vec<M>) -> Self {
        Self::Messages(msgs)
    }

    /// Whether the event was consumed (not ignored).
    pub fn is_handled(&self) -> bool {
        !matches!(self, Self::Ignored)
    }

    /// Transform message types, preserving handled state.
    pub fn map<U>(self, mut f: impl FnMut(M) -> U) -> Outcome<U> {
        match self {
            Self::Ignored => Outcome::Ignored,
            Self::Consumed => Outcome::Consumed,
            Self::Message(m) => Outcome::Message(f(m)),
            Self::Messages(msgs) => {
                Outcome::Messages(msgs.into_iter().map(f).collect())
            }
        }
    }

    /// Discard messages, preserving handled state.
    pub fn discard_messages<U>(self) -> Outcome<U> {
        match self {
            Self::Ignored => Outcome::Ignored,
            _ => Outcome::Consumed,
        }
    }

    /// Merge two outcomes. If either is handled, result is handled.
    /// Messages are concatenated in order.
    pub fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Ignored, other) => other,
            (handled, Self::Ignored) => handled,
            (Self::Consumed, Self::Consumed) => Self::Consumed,
            (Self::Consumed, Self::Message(m)) | (Self::Message(m), Self::Consumed) => {
                Self::Message(m)
            }
            (Self::Message(a), Self::Message(b)) => Self::Messages(vec![a, b]),
            (Self::Messages(mut v), Self::Message(m)) => {
                v.push(m);
                Self::Messages(v)
            }
            (Self::Message(m), Self::Messages(mut v)) => {
                v.insert(0, m);
                Self::Messages(v)
            }
            (Self::Messages(mut a), Self::Messages(b)) => {
                a.extend(b);
                Self::Messages(a)
            }
            (Self::Consumed, Self::Messages(v)) | (Self::Messages(v), Self::Consumed) => {
                Self::Messages(v)
            }
        }
    }

    /// Collect messages into a Vec, consuming self.
    pub fn into_messages(self) -> Vec<M> {
        match self {
            Self::Ignored | Self::Consumed => Vec::new(),
            Self::Message(m) => vec![m],
            Self::Messages(v) => v,
        }
    }
}

/// A component that can process events and emit typed messages.
pub trait Widget {
    /// The message type emitted by this widget.
    type Message;

    /// Process an event and return the outcome.
    fn on_event(&mut self, event: &WidgetEvent) -> Outcome<Self::Message>;

    /// Render the current state to lines.
    fn render(&self, ctx: &ViewContext) -> Vec<Line>;
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
