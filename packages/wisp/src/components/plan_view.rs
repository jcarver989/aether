use agent_client_protocol::{PlanEntry, PlanEntryStatus};

use crate::tui::spinner::BRAILLE_FRAMES;
use crate::tui::{Component, Line, RenderContext};

/// Renders the agent's task plan as a compact checklist.
///
/// ```text
/// Plan
///   ✓ Research AI agent patterns
///   ⠋ Implement task tracking
///   ○ Write integration tests
/// ```
pub struct PlanView<'a> {
    pub entries: &'a [PlanEntry],
    pub tick: u16,
}

impl Component for PlanView<'_> {
    fn render(&mut self, context: &RenderContext) -> Vec<Line> {
        if self.entries.is_empty() {
            return vec![];
        }

        let mut lines = Vec::with_capacity(self.entries.len() + 2);
        lines.push(Line::default());

        let mut header = Line::default();
        header.push_styled("Plan".to_string(), context.theme.muted);
        lines.push(header);

        for entry in self.entries {
            let mut line = Line::default();
            match entry.status {
                PlanEntryStatus::Completed => {
                    line.push_styled("  ✓ ".to_string(), context.theme.success);
                    line.push_styled(entry.content.clone(), context.theme.muted);
                }
                PlanEntryStatus::InProgress => {
                    let frame = BRAILLE_FRAMES[self.tick as usize % BRAILLE_FRAMES.len()];
                    line.push_styled(format!("  {frame} "), context.theme.info);
                    line.push_text(entry.content.clone());
                }
                _ => {
                    line.push_styled("  ○ ".to_string(), context.theme.muted);
                    line.push_styled(entry.content.clone(), context.theme.muted);
                }
            }
            lines.push(line);
        }

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::{PlanEntry, PlanEntryPriority, PlanEntryStatus};

    fn ctx() -> RenderContext {
        RenderContext::new((80, 24))
    }

    fn entry(content: &str, status: PlanEntryStatus) -> PlanEntry {
        PlanEntry::new(content.to_string(), PlanEntryPriority::Medium, status)
    }

    #[test]
    fn empty_entries_render_nothing() {
        let mut view = PlanView {
            entries: &[],
            tick: 0,
        };
        assert!(view.render(&ctx()).is_empty());
    }

    #[test]
    fn renders_header_plus_entries() {
        let entries = vec![
            entry("Research", PlanEntryStatus::Completed),
            entry("Implement", PlanEntryStatus::InProgress),
            entry("Test", PlanEntryStatus::Pending),
        ];
        let mut view = PlanView {
            entries: &entries,
            tick: 0,
        };
        let lines = view.render(&ctx());
        // margin + header + 3 entries
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0].plain_text(), "");
        assert_eq!(lines[1].plain_text(), "Plan");
    }

    #[test]
    fn completed_entry_has_checkmark() {
        let entries = vec![entry("Done task", PlanEntryStatus::Completed)];
        let mut view = PlanView {
            entries: &entries,
            tick: 0,
        };
        let lines = view.render(&ctx());
        let text = lines[2].plain_text();
        assert!(text.contains('✓'));
        assert!(text.contains("Done task"));
    }

    #[test]
    fn in_progress_entry_has_spinner() {
        let entries = vec![entry("Working", PlanEntryStatus::InProgress)];
        let mut view = PlanView {
            entries: &entries,
            tick: 0,
        };
        let lines = view.render(&ctx());
        let text = lines[2].plain_text();
        assert!(text.contains(BRAILLE_FRAMES[0]));
        assert!(text.contains("Working"));
    }

    #[test]
    fn pending_entry_has_circle() {
        let entries = vec![entry("Todo", PlanEntryStatus::Pending)];
        let mut view = PlanView {
            entries: &entries,
            tick: 0,
        };
        let lines = view.render(&ctx());
        let text = lines[2].plain_text();
        assert!(text.contains('○'));
        assert!(text.contains("Todo"));
    }

    #[test]
    fn spinner_animates_with_tick() {
        let entries = vec![entry("Working", PlanEntryStatus::InProgress)];
        let mut view_a = PlanView {
            entries: &entries,
            tick: 0,
        };
        let mut view_b = PlanView {
            entries: &entries,
            tick: 1,
        };
        let text_a = view_a.render(&ctx())[2].plain_text();
        let text_b = view_b.render(&ctx())[2].plain_text();
        assert_ne!(text_a, text_b);
    }
}
