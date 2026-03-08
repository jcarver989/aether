use crate::line::Line;
use crate::size::Size;
use crate::theme::Theme;
use crossterm::event::KeyEvent;
use std::time::Instant;

/// Environment passed to [`Component::render`]: terminal size, theme, focus state,
/// and optional height constraint.
#[derive(Clone)]
pub struct RenderContext {
    pub size: Size,
    pub theme: Theme,
    pub focused: bool,
    pub max_height: Option<usize>,
}

impl RenderContext {
    pub fn new(size: impl Into<Size>) -> Self {
        Self::new_with_theme(size, Theme::default())
    }

    pub fn new_with_theme(size: impl Into<Size>, theme: Theme) -> Self {
        Self {
            size: size.into(),
            theme,
            focused: true,
            max_height: None,
        }
    }

    pub fn with_size(&self, size: impl Into<Size>) -> Self {
        Self {
            size: size.into(),
            theme: self.theme.clone(),
            focused: self.focused,
            max_height: self.max_height,
        }
    }

    pub fn with_theme(&self, theme: Theme) -> Self {
        Self {
            size: self.size,
            theme,
            focused: self.focused,
            max_height: self.max_height,
        }
    }

    pub fn with_focused(&self, focused: bool) -> Self {
        Self {
            size: self.size,
            theme: self.theme.clone(),
            focused,
            max_height: self.max_height,
        }
    }

    pub fn with_max_height(&self, max_height: usize) -> Self {
        Self {
            size: self.size,
            theme: self.theme.clone(),
            focused: self.focused,
            max_height: Some(max_height),
        }
    }

    pub fn without_max_height(&self) -> Self {
        Self {
            size: self.size,
            theme: self.theme.clone(),
            focused: self.focused,
            max_height: None,
        }
    }
}

/// A stateful widget that can render itself as styled terminal lines.
pub trait Component {
    fn render(&self, context: &RenderContext) -> Vec<Line>;
}

/// Result of handling a key event via [`HandlesInput`].
///
/// - `consumed` — whether the key was handled (prevents further propagation).
/// - `needs_render` — whether the UI should re-render.
/// - `action` — an optional typed action emitted to the parent.
pub struct InputOutcome<A> {
    pub consumed: bool,
    pub needs_render: bool,
    pub action: Option<A>,
}

impl<A> InputOutcome<A> {
    /// Transform the action type, preserving `consumed` and `needs_render`.
    pub fn map<B>(self, f: impl FnOnce(A) -> B) -> InputOutcome<B> {
        InputOutcome {
            consumed: self.consumed,
            needs_render: self.needs_render,
            action: self.action.map(f),
        }
    }

    /// Discard the action, preserving `consumed` and `needs_render`.
    ///
    /// The output type is inferred from context, so this can convert between
    /// `InputOutcome<A>` and `InputOutcome<B>`.
    pub fn discard_action<B>(self) -> InputOutcome<B> {
        InputOutcome {
            consumed: self.consumed,
            needs_render: self.needs_render,
            action: None,
        }
    }

    pub fn ignored() -> Self {
        Self {
            consumed: false,
            needs_render: false,
            action: None,
        }
    }

    pub fn consumed() -> Self {
        Self {
            consumed: true,
            needs_render: false,
            action: None,
        }
    }

    pub fn consumed_and_render() -> Self {
        Self {
            consumed: true,
            needs_render: true,
            action: None,
        }
    }

    pub fn action(action: A) -> Self {
        Self {
            consumed: true,
            needs_render: false,
            action: Some(action),
        }
    }

    pub fn action_and_render(action: A) -> Self {
        Self {
            consumed: true,
            needs_render: true,
            action: Some(action),
        }
    }
}

/// A component that can process keyboard input and emit typed actions.
pub trait HandlesInput {
    type Action;

    fn handle_key(&mut self, key_event: KeyEvent) -> InputOutcome<Self::Action>;
}

/// A component with time-based animation state.
pub trait Tickable {
    /// Advance animation state by one tick.
    fn on_tick(&mut self, now: Instant);
}
