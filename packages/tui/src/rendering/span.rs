use super::style::Style;

/// A contiguous run of text sharing a single [`Style`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub(crate) text: String,
    pub(crate) style: Style,
}

impl Span {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: Style::default(),
        }
    }

    pub fn with_style(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn style(&self) -> Style {
        self.style
    }
}
