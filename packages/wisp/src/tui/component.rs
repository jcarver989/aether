use super::screen::Line;
use super::theme::Theme;
use crossterm::event::KeyEvent;

pub struct RenderContext {
    pub size: (u16, u16),
    pub theme: Theme,
}

impl RenderContext {
    pub fn new(size: (u16, u16)) -> Self {
        Self {
            size,
            theme: Theme::default(),
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

    fn handle_key(&mut self, key_event: KeyEvent, input: &mut String)
    -> InputOutcome<Self::Action>;
}
