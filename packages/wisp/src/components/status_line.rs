use crate::tui::{Component, Line, RenderContext};

pub struct StatusLine<'a> {
    pub agent_name: &'a str,
    pub model_display: Option<&'a str>,
    pub context_pct_left: Option<u8>,
    pub waiting_for_response: bool,
}

impl Component for StatusLine<'_> {
    fn render(&mut self, context: &RenderContext) -> Vec<Line> {
        let left = match self.model_display {
            Some(model) => format!("  {} · {}", self.agent_name, model),
            None => format!("  {}", self.agent_name),
        };

        let (right, color) = if self.waiting_for_response {
            ("esc to interrupt".to_string(), context.theme.warning)
        } else if let Some(pct) = self.context_pct_left {
            let c = if pct <= 15 {
                context.theme.warning
            } else {
                context.theme.muted
            };
            (format!("{pct}% context"), c)
        } else {
            return vec![Line::styled(left, context.theme.muted)];
        };

        let width = context.size.0 as usize;
        let left_visible_len = left.len();
        let right_visible_len = right.len();

        let padding = width.saturating_sub(left_visible_len + right_visible_len);
        let mut line = Line::default();
        line.push_styled(left, context.theme.muted);
        line.push_text(" ".repeat(padding));
        line.push_styled(right, color);
        vec![line]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_agent_name() {
        let mut status = StatusLine {
            agent_name: "claude-code",
            model_display: None,
            context_pct_left: None,
            waiting_for_response: false,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain_text().contains("claude-code"));
    }

    #[test]
    fn renders_with_indentation() {
        let mut status = StatusLine {
            agent_name: "test-agent",
            model_display: None,
            context_pct_left: None,
            waiting_for_response: false,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        // Should have leading spaces for indentation
        assert!(lines[0].plain_text().contains("  test-agent"));
    }

    #[test]
    fn renders_model_display() {
        let mut status = StatusLine {
            agent_name: "aether-acp",
            model_display: Some("gpt-4o"),
            context_pct_left: None,
            waiting_for_response: false,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(text.contains("aether-acp"), "should contain agent name");
        assert!(text.contains("gpt-4o"), "should contain model name");
        assert!(
            text.contains("·"),
            "should contain separator between agent and model"
        );
    }

    #[test]
    fn renders_without_model_when_none() {
        let mut status = StatusLine {
            agent_name: "aether-acp",
            model_display: None,
            context_pct_left: None,
            waiting_for_response: false,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        let text = lines[0].plain_text();
        assert!(text.contains("aether-acp"));
        assert!(
            !text.contains("·"),
            "should not contain separator when no model"
        );
    }

    #[test]
    fn renders_context_usage_right_aligned() {
        let mut status = StatusLine {
            agent_name: "aether",
            model_display: Some("gpt-4o"),
            context_pct_left: Some(72),
            waiting_for_response: false,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(text.contains("aether"), "should contain agent name");
        assert!(text.contains("72% context"), "should contain context usage");
    }

    #[test]
    fn does_not_render_context_when_none() {
        let mut status = StatusLine {
            agent_name: "aether",
            model_display: Some("gpt-4o"),
            context_pct_left: None,
            waiting_for_response: false,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        let text = lines[0].plain_text();
        assert!(!text.contains("context"), "should not contain context info");
    }

    #[test]
    fn renders_interrupt_message_when_waiting() {
        let mut status = StatusLine {
            agent_name: "aether",
            model_display: Some("gpt-4o"),
            context_pct_left: Some(72),
            waiting_for_response: true,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        let text = lines[0].plain_text();
        assert!(text.contains("aether"), "should contain agent name");
        assert!(
            text.contains("esc to interrupt"),
            "should contain interrupt message"
        );
        assert!(
            !text.contains("72% context"),
            "should not contain context when waiting"
        );
    }

    #[test]
    fn renders_interrupt_message_without_model_when_waiting() {
        let mut status = StatusLine {
            agent_name: "aether",
            model_display: None,
            context_pct_left: None,
            waiting_for_response: true,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        let text = lines[0].plain_text();
        assert!(text.contains("aether"), "should contain agent name");
        assert!(
            text.contains("esc to interrupt"),
            "should contain interrupt message"
        );
    }
}
