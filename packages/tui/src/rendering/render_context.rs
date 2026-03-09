use std::sync::Arc;

use super::size::Size;
use crate::theme::Theme;

#[cfg(feature = "syntax")]
use crate::syntax_highlighting::SyntaxHighlighter;

/// Environment passed to [`Component::render`](crate::Component::render): terminal size, theme,
/// focus state, and optional height constraint.
#[derive(Clone)]
pub struct RenderContext {
    pub size: Size,
    pub theme: Arc<Theme>,
    pub focused: bool,
    pub max_height: Option<usize>,
    #[cfg(feature = "syntax")]
    pub(crate) highlighter: Arc<SyntaxHighlighter>,
}

impl RenderContext {
    pub fn new(size: impl Into<Size>) -> Self {
        Self::new_with_theme(size, Theme::default())
    }

    pub fn new_with_theme(size: impl Into<Size>, theme: Theme) -> Self {
        Self {
            size: size.into(),
            theme: Arc::new(theme),
            focused: true,
            max_height: None,
            #[cfg(feature = "syntax")]
            highlighter: Arc::new(SyntaxHighlighter::new()),
        }
    }

    #[cfg(feature = "syntax")]
    pub fn highlighter(&self) -> &SyntaxHighlighter {
        &self.highlighter
    }

    pub fn with_size(&self, size: impl Into<Size>) -> Self {
        Self {
            size: size.into(),
            theme: self.theme.clone(),
            focused: self.focused,
            max_height: self.max_height,
            #[cfg(feature = "syntax")]
            highlighter: self.highlighter.clone(),
        }
    }

    pub fn with_theme(&self, theme: Theme) -> Self {
        Self {
            size: self.size,
            theme: Arc::new(theme),
            focused: self.focused,
            max_height: self.max_height,
            #[cfg(feature = "syntax")]
            highlighter: self.highlighter.clone(),
        }
    }

    pub fn with_focused(&self, focused: bool) -> Self {
        Self {
            size: self.size,
            theme: self.theme.clone(),
            focused,
            max_height: self.max_height,
            #[cfg(feature = "syntax")]
            highlighter: self.highlighter.clone(),
        }
    }

    pub fn with_max_height(&self, max_height: usize) -> Self {
        Self {
            size: self.size,
            theme: self.theme.clone(),
            focused: self.focused,
            max_height: Some(max_height),
            #[cfg(feature = "syntax")]
            highlighter: self.highlighter.clone(),
        }
    }

    pub fn without_max_height(&self) -> Self {
        Self {
            size: self.size,
            theme: self.theme.clone(),
            focused: self.focused,
            max_height: None,
            #[cfg(feature = "syntax")]
            highlighter: self.highlighter.clone(),
        }
    }
}
