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

/// Unified result type for event handling and effect processing.
///
/// Used by both [`Widget::on_event`] (widget-level events) and
/// [`App::update`](crate::runtime::app::App::update) (app-level effects).
#[derive(Debug, Default)]
pub enum Response<M> {
    /// Event not recognized, propagate to parent.
    Ignored,
    /// Event consumed / no effects.
    #[default]
    Ok,
    /// Request application exit.
    Exit,
    /// One message or effect.
    One(M),
    /// Multiple messages or effects.
    Many(Vec<M>),
}

impl<M> Response<M> {
    /// The event was not recognized and should propagate.
    pub fn ignored() -> Self {
        Self::Ignored
    }

    /// The event was consumed, no messages.
    pub fn ok() -> Self {
        Self::Ok
    }

    /// Request application exit.
    pub fn exit() -> Self {
        Self::Exit
    }

    /// Emit a single message.
    pub fn one(msg: M) -> Self {
        Self::One(msg)
    }

    /// Emit multiple messages.
    pub fn many(msgs: Vec<M>) -> Self {
        Self::Many(msgs)
    }

    /// Collapse a vector into the smallest matching representation.
    pub fn from_vec(mut items: Vec<M>) -> Self {
        match items.len() {
            0 => Self::Ok,
            1 => Self::One(items.pop().expect("one item")),
            _ => Self::Many(items),
        }
    }

    /// Whether the event was consumed (not ignored).
    pub fn is_handled(&self) -> bool {
        !matches!(self, Self::Ignored)
    }

    /// Check if this is an exit response.
    pub fn is_exit(&self) -> bool {
        matches!(self, Self::Exit)
    }

    /// Transform message types, preserving control state.
    pub fn map<U>(self, mut f: impl FnMut(M) -> U) -> Response<U> {
        match self {
            Self::Ignored => Response::Ignored,
            Self::Ok => Response::Ok,
            Self::Exit => Response::Exit,
            Self::One(m) => Response::One(f(m)),
            Self::Many(msgs) => Response::Many(msgs.into_iter().map(f).collect()),
        }
    }

    /// Discard messages, preserving control state.
    pub fn discard_messages<U>(self) -> Response<U> {
        match self {
            Self::Ignored => Response::Ignored,
            Self::Exit => Response::Exit,
            _ => Response::Ok,
        }
    }

    /// Merge two responses. Exit takes priority, then Ignored yields to the other.
    /// Messages are concatenated in order.
    pub fn merge(self, other: Self) -> Self {
        if self.is_exit() || other.is_exit() {
            return Self::Exit;
        }

        match (self, other) {
            (Self::Ignored, other) => other,
            (handled, Self::Ignored) => handled,
            (Self::Ok, Self::Ok) => Self::Ok,
            (Self::Ok, Self::One(m)) | (Self::One(m), Self::Ok) => Self::One(m),
            (Self::One(a), Self::One(b)) => Self::Many(vec![a, b]),
            (Self::Many(mut v), Self::One(m)) => {
                v.push(m);
                Self::Many(v)
            }
            (Self::One(m), Self::Many(mut v)) => {
                v.insert(0, m);
                Self::Many(v)
            }
            (Self::Many(mut a), Self::Many(b)) => {
                a.extend(b);
                Self::Many(a)
            }
            (Self::Ok, Self::Many(v)) | (Self::Many(v), Self::Ok) => Self::Many(v),
            // Exit cases handled above
            _ => unreachable!(),
        }
    }

    /// Add a single item to the end of this sequence.
    pub fn append(self, item: M) -> Self {
        self.merge(Self::One(item))
    }

    /// Collect messages into a Vec, consuming self.
    pub fn into_messages(self) -> Vec<M> {
        match self {
            Self::Ignored | Self::Ok | Self::Exit => Vec::new(),
            Self::One(m) => vec![m],
            Self::Many(v) => v,
        }
    }
}

impl<M> FromIterator<M> for Response<M> {
    fn from_iter<I: IntoIterator<Item = M>>(iter: I) -> Self {
        Self::from_vec(iter.into_iter().collect())
    }
}

/// A component that can process events and emit typed messages.
pub trait Widget {
    /// The message type emitted by this widget.
    type Message;

    /// Process an event and return the outcome.
    fn on_event(&mut self, event: &WidgetEvent) -> Response<Self::Message>;

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
