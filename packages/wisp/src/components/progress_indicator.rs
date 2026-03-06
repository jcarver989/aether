use crate::tui::spinner::BRAILLE_FRAMES as FRAMES;
use crate::tui::{Component, Line, RenderContext};

/// Renders a single progress line when tools are actively running.
/// Shows: `⠋ Working... (N/M tools complete)`
pub struct ProgressIndicator {
    pub completed: usize,
    pub total: usize,
    pub tick: u16,
}

impl Component for ProgressIndicator {
    fn render(&mut self, context: &RenderContext) -> Vec<Line> {
        if self.total == 0 || self.completed == self.total {
            return vec![];
        }

        let frame = FRAMES[self.tick as usize % FRAMES.len()];
        let mut line = Line::default();
        line.push_styled(frame.to_string(), context.theme.info());
        line.push_styled(
            format!(
                " Working... ({}/{} tools complete)",
                self.completed, self.total
            ),
            context.theme.muted(),
        );
        vec![line]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> RenderContext {
        RenderContext::new((80, 24))
    }

    #[test]
    fn renders_nothing_when_no_tools() {
        let mut indicator = ProgressIndicator {
            completed: 0,
            total: 0,
            tick: 0,
        };
        assert!(indicator.render(&ctx()).is_empty());
    }

    #[test]
    fn renders_nothing_when_all_complete() {
        let mut indicator = ProgressIndicator {
            completed: 3,
            total: 3,
            tick: 0,
        };
        assert!(indicator.render(&ctx()).is_empty());
    }

    #[test]
    fn renders_progress_when_tools_running() {
        let mut indicator = ProgressIndicator {
            completed: 1,
            total: 3,
            tick: 0,
        };
        let lines = indicator.render(&ctx());
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(text.contains("Working..."));
        assert!(text.contains("1/3 tools complete"));
    }

    #[test]
    fn spinner_animates_with_tick() {
        let mut a = ProgressIndicator {
            completed: 0,
            total: 1,
            tick: 0,
        };
        let mut b = ProgressIndicator {
            completed: 0,
            total: 1,
            tick: 1,
        };
        let text_a = a.render(&ctx())[0].plain_text();
        let text_b = b.render(&ctx())[0].plain_text();
        assert_ne!(text_a, text_b);
    }
}
