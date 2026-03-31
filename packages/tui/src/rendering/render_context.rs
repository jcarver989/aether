use std::sync::Arc;

use crate::theme::Theme;

#[cfg(feature = "syntax")]
use crate::syntax_highlighting::SyntaxHighlighter;

#[doc = include_str!("../docs/view_context.md")]
#[derive(Clone)]
pub struct ViewContext {
    pub size: Size,
    pub theme: Arc<Theme>,
    #[cfg(feature = "syntax")]
    pub(crate) highlighter: Arc<SyntaxHighlighter>,
}

/// Terminal dimensions in columns and rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub width: u16,
    pub height: u16,
}

impl ViewContext {
    pub fn new(size: impl Into<Size>) -> Self {
        Self::new_with_theme(size, Theme::default())
    }

    pub fn new_with_theme(size: impl Into<Size>, theme: Theme) -> Self {
        Self {
            size: size.into(),
            theme: Arc::new(theme),
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
            #[cfg(feature = "syntax")]
            highlighter: self.highlighter.clone(),
        }
    }
}

impl From<(u16, u16)> for Size {
    fn from((width, height): (u16, u16)) -> Self {
        Self { width, height }
    }
}
