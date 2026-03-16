use tui::BRAILLE_FRAMES as FRAMES;
use tui::{Line, ViewContext};

/// Renders a spinner with "(esc to interrupt)" when the agent is busy.
/// Visible whenever we're waiting for a response OR tools are actively running.
#[derive(Default)]
pub struct ProgressIndicator {
    tools_running: bool,
    waiting_for_response: bool,
    tick: u16,
}

impl ProgressIndicator {
    pub fn update(&mut self, completed: usize, total: usize, waiting_for_response: bool) {
        self.tools_running = total > 0 && completed < total;
        self.waiting_for_response = waiting_for_response;
    }

    #[cfg(test)]
    pub fn set_tick(&mut self, tick: u16) {
        self.tick = tick;
    }

    fn is_active(&self) -> bool {
        self.tools_running || self.waiting_for_response
    }

    /// Advance the animation state. Call this on tick events.
    pub fn on_tick(&mut self) {
        if self.is_active() {
            self.tick = self.tick.wrapping_add(1);
        }
    }
}

impl ProgressIndicator {
    pub fn render(&self, context: &ViewContext) -> Vec<Line> {
        if !self.is_active() {
            return vec![];
        }

        let frame = FRAMES[self.tick as usize % FRAMES.len()];
        let mut line = Line::default();
        line.push_styled(frame.to_string(), context.theme.info());
        line.push_styled(" (esc to interrupt)".to_string(), context.theme.text_secondary());
        vec![line]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> ViewContext {
        ViewContext::new((80, 24))
    }

    #[test]
    fn renders_nothing_when_idle() {
        let indicator = ProgressIndicator::default();
        assert!(indicator.render(&ctx()).is_empty());
    }

    #[test]
    fn renders_nothing_when_all_complete_and_not_waiting() {
        let mut indicator = ProgressIndicator::default();
        indicator.update(3, 3, false);
        assert!(indicator.render(&ctx()).is_empty());
    }

    #[test]
    fn renders_when_tools_running() {
        let mut indicator = ProgressIndicator::default();
        indicator.update(1, 3, false);
        let lines = indicator.render(&ctx());
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(text.contains("esc to interrupt"));
    }

    #[test]
    fn renders_when_waiting_for_response_without_tools() {
        let mut indicator = ProgressIndicator::default();
        indicator.update(0, 0, true);
        let lines = indicator.render(&ctx());
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(text.contains("esc to interrupt"));
    }

    #[test]
    fn spinner_animates_with_tick() {
        let mut a = ProgressIndicator::default();
        a.update(0, 1, false);
        let mut b = ProgressIndicator::default();
        b.update(0, 1, false);
        b.set_tick(1);
        let text_a = a.render(&ctx())[0].plain_text();
        let text_b = b.render(&ctx())[0].plain_text();
        assert_ne!(text_a, text_b);
    }

    #[test]
    fn on_tick_advances_when_running() {
        let mut indicator = ProgressIndicator::default();
        indicator.update(1, 3, false);
        indicator.on_tick();
        let lines = indicator.render(&ctx());
        assert!(!lines.is_empty());
    }

    #[test]
    fn on_tick_advances_when_waiting() {
        let mut indicator = ProgressIndicator::default();
        indicator.update(0, 0, true);
        let frame_before = indicator.tick;
        indicator.on_tick();
        assert_ne!(indicator.tick, frame_before);
    }

    #[test]
    fn on_tick_noop_when_idle() {
        let mut indicator = ProgressIndicator::default();
        indicator.update(3, 3, false);
        indicator.on_tick();
        assert!(indicator.render(&ctx()).is_empty());
    }
}
