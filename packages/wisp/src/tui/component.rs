use super::screen::Line;
use super::theme::Theme;
use crossterm::event::KeyEvent;
use std::time::Instant;

#[derive(Clone)]
pub struct RenderContext {
    pub size: (u16, u16),
    pub theme: Theme,
    pub focused: bool,
    pub max_height: Option<usize>,
}

impl RenderContext {
    pub fn new(size: (u16, u16)) -> Self {
        Self::new_with_theme(size, Theme::default())
    }

    pub fn new_with_theme(size: (u16, u16), theme: Theme) -> Self {
        Self {
            size,
            theme,
            focused: true,
            max_height: None,
        }
    }

    pub fn with_size(&self, size: (u16, u16)) -> Self {
        Self {
            size,
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

pub trait Component {
    fn render(&mut self, context: &RenderContext) -> Vec<Line>;
}

pub struct InputOutcome<A> {
    pub consumed: bool,
    pub needs_render: bool,
    pub action: Option<A>,
}

impl<A> InputOutcome<A> {
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

pub trait HandlesInput {
    type Action;

    fn handle_key(&mut self, key_event: KeyEvent) -> InputOutcome<Self::Action>;
}

pub trait Tickable {
    /// Advance animation state by one tick.
    fn on_tick(&mut self, now: Instant);
}
