use crate::tui::components::spinner::BRAILLE_FRAMES as FRAMES;
use crate::tui::{Component, Line, RenderContext, TickableComponent};
use std::time::Instant;

/// Renders a single progress line when tools are actively running.
/// Shows: `⠋ Working... (N/M tools complete)`
#[derive(Default)]
pub struct ProgressIndicator {
    pub completed: usize,
    pub total: usize,
    tick: u16,
}

impl ProgressIndicator {
    pub fn update(&mut self, completed: usize, total: usize) {
        self.completed = completed;
        self.total = total;
    }

    #[cfg(test)]
    pub fn set_tick(&mut self, tick: u16) {
        self.tick = tick;
    }
}

impl TickableComponent for ProgressIndicator {
    fn on_tick(&mut self, _now: Instant) {
        if self.total > 0 && self.completed < self.total {
            self.tick = self.tick.wrapping_add(1);
        }
    }
}

impl Component for ProgressIndicator {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
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
        let indicator = ProgressIndicator::default();
        assert!(indicator.render(&ctx()).is_empty());
    }

    #[test]
    fn renders_nothing_when_all_complete() {
        let mut indicator = ProgressIndicator::default();
        indicator.update(3, 3);
        assert!(indicator.render(&ctx()).is_empty());
    }

    #[test]
    fn renders_progress_when_tools_running() {
        let mut indicator = ProgressIndicator::default();
        indicator.update(1, 3);
        let lines = indicator.render(&ctx());
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(text.contains("Working..."));
        assert!(text.contains("1/3 tools complete"));
    }

    #[test]
    fn spinner_animates_with_tick() {
        let mut a = ProgressIndicator::default();
        a.update(0, 1);
        let mut b = ProgressIndicator::default();
        b.update(0, 1);
        b.set_tick(1);
        let text_a = a.render(&ctx())[0].plain_text();
        let text_b = b.render(&ctx())[0].plain_text();
        assert_ne!(text_a, text_b);
    }

    #[test]
    fn on_tick_advances_when_running() {
        let mut indicator = ProgressIndicator::default();
        indicator.update(1, 3);
        indicator.on_tick(Instant::now());
        let lines = indicator.render(&ctx());
        assert!(!lines.is_empty());
    }

    #[test]
    fn on_tick_noop_when_all_complete() {
        let mut indicator = ProgressIndicator::default();
        indicator.update(3, 3);
        indicator.on_tick(Instant::now());
        assert!(indicator.render(&ctx()).is_empty());
    }

    #[test]
    fn on_tick_noop_when_empty() {
        let mut indicator = ProgressIndicator::default();
        indicator.on_tick(Instant::now());
        assert!(indicator.render(&ctx()).is_empty());
    }
}
