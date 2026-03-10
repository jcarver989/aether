use crate::rendering::soft_wrap::display_width_line;
use crate::{Line, RenderContext};

/// A status bar with left and right sections, right-aligning the right section.
///
/// The right section is automatically padded to align to the right edge of the terminal.
///
/// # Example
///
/// ```
/// use tui::{StatusBar, Line, RenderContext};
///
/// fn render_status(ctx: &RenderContext) -> Vec<Line> {
///     let status = StatusBar::new()
///         .left(|line| {
///             line.push_text("Ready");
///         })
///         .right(|line| {
///             line.push_text("Ctrl+Q to quit");
///         });
///     
///     status.render(ctx)
/// }
/// ```
pub struct StatusBar<L, R>
where
    L: FnOnce(&mut Line),
    R: FnOnce(&mut Line),
{
    left: Option<L>,
    right: Option<R>,
}

impl StatusBar<fn(&mut Line), fn(&mut Line)> {
    /// Create a new empty status bar.
    pub fn new() -> Self {
        Self {
            left: None,
            right: None,
        }
    }
}

impl Default for StatusBar<fn(&mut Line), fn(&mut Line)> {
    fn default() -> Self {
        Self::new()
    }
}

impl<L, R> StatusBar<L, R>
where
    L: FnOnce(&mut Line),
    R: FnOnce(&mut Line),
{
    /// Set the left section content using a closure.
    pub fn left<L2>(self, f: L2) -> StatusBar<L2, R>
    where
        L2: FnOnce(&mut Line),
    {
        StatusBar {
            left: Some(f),
            right: self.right,
        }
    }

    /// Set the right section content using a closure.
    pub fn right<R2>(self, f: R2) -> StatusBar<L, R2>
    where
        R2: FnOnce(&mut Line),
    {
        StatusBar {
            left: self.left,
            right: Some(f),
        }
    }

    /// Render the status bar to a single line.
    pub fn render(self, context: &RenderContext) -> Vec<Line> {
        let mut line = Line::default();

        // Build left section
        if let Some(left_fn) = self.left {
            left_fn(&mut line);
        }

        // Build right section if provided
        if let Some(right_fn) = self.right {
            let mut right_line = Line::default();
            right_fn(&mut right_line);

            let width = context.size.width as usize;
            let left_len = display_width_line(&line);
            let right_len = display_width_line(&right_line);

            let padding = width.saturating_sub(left_len + right_len);
            line.push_text(" ".repeat(padding));
            line.append_line(&right_line);
        }

        vec![line]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_left_section_only() {
        let ctx = RenderContext::new((40, 10));
        let status = StatusBar::new().left(|line| {
            line.push_text("Ready");
        });
        let lines = status.render(&ctx);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain_text().contains("Ready"));
    }

    #[test]
    fn renders_left_and_right_sections() {
        let ctx = RenderContext::new((40, 10));
        let status = StatusBar::new()
            .left(|line| {
                line.push_text("Left");
            })
            .right(|line| {
                line.push_text("Right");
            });
        let lines = status.render(&ctx);
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(text.contains("Left"));
        assert!(text.contains("Right"));

        // Verify order: Left should appear before Right
        let left_idx = text.find("Left").expect("Left position");
        let right_idx = text.find("Right").expect("Right position");
        assert!(left_idx < right_idx, "Left should come before Right");
    }

    #[test]
    fn right_section_is_right_aligned() {
        let ctx = RenderContext::new((20, 10));
        let status = StatusBar::new()
            .left(|line| {
                line.push_text("A");
            })
            .right(|line| {
                line.push_text("B");
            });
        let lines = status.render(&ctx);
        let text = lines[0].plain_text();
        // Width is 20, "A" is 1 char, "B" is 1 char
        // So there should be 18 spaces between them
        assert!(text.starts_with("A"));
        assert!(text.ends_with("B"));
        assert_eq!(text.len(), 20);
    }

    #[test]
    fn handles_wide_content() {
        let ctx = RenderContext::new((10, 10));
        let status = StatusBar::new()
            .left(|line| {
                line.push_text("12345");
            })
            .right(|line| {
                line.push_text("67890");
            });
        let lines = status.render(&ctx);
        let text = lines[0].plain_text();
        // Content exceeds width, but we don't truncate
        assert!(text.contains("12345"));
        assert!(text.contains("67890"));
    }

    #[test]
    fn empty_status_bar() {
        let ctx = RenderContext::new((40, 10));
        let status = StatusBar::new();
        let lines = status.render(&ctx);
        assert_eq!(lines.len(), 1);
        // Should just be empty (no content)
        let text = lines[0].plain_text();
        assert!(text.is_empty());
    }
}
