use super::size::Size;
use crate::theme::Theme;

/// Environment passed to [`Component::render`](crate::Component::render): terminal size, theme,
/// focus state, and optional height constraint.
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
