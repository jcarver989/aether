use crate::tui::soft_wrap::{display_width_line, display_width_text};
use crate::tui::{Component, Line, RenderContext, Style};

pub struct StatusLine<'a> {
    pub agent_name: &'a str,
    pub mode_display: Option<&'a str>,
    pub mode_index: usize,
    pub model_display: Option<&'a str>,
    pub context_pct_left: Option<u8>,
    pub waiting_for_response: bool,
    pub unhealthy_server_count: usize,
}

impl Component for StatusLine<'_> {
    fn render(&mut self, context: &RenderContext) -> Vec<Line> {
        let mut left_line = Line::default();
        let muted = context.theme.muted();
        let sep = context.theme.text_secondary();

        left_line.push_text("  ");
        left_line.push_styled(self.agent_name, muted);

        if let Some(mode) = self.mode_display {
            left_line.push_styled(" · ", sep);
            let color = context.theme.mode_badge_bg(self.mode_index);
            left_line.push_styled(mode, color);
        }

        if let Some(model) = self.model_display {
            left_line.push_styled(" · ", sep);
            left_line.push_styled(model, muted);
        }

        let (right, color) = if self.waiting_for_response {
            let mut parts = vec!["esc to interrupt".to_string()];
            if let Some(pct) = self.context_pct_left {
                parts.push(format!("{pct}% context"));
            }
            (parts.join(" · "), context.theme.warning())
        } else if let Some(pct) = self.context_pct_left {
            let c = if pct <= 15 {
                context.theme.warning()
            } else {
                context.theme.muted()
            };
            (format!("{pct}% context"), c)
        } else if self.unhealthy_server_count > 0 {
            let count = self.unhealthy_server_count;
            let msg = if count == 1 {
                "1 server needs auth".to_string()
            } else {
                format!("{count} servers unhealthy")
            };
            (msg, context.theme.warning())
        } else {
            return vec![left_line];
        };

        let width = context.size.0 as usize;
        let right_len = display_width_text(&right);
        let left_len = display_width_line(&left_line);

        let padding = width.saturating_sub(left_len + right_len);
        left_line.push_text(" ".repeat(padding));
        left_line.push_styled(right, color);
        vec![left_line]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_agent_name() {
        let mut status = StatusLine {
            agent_name: "claude-code",
            mode_display: None,
            mode_index: 0,
            model_display: None,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
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
            mode_display: None,
            mode_index: 0,
            model_display: None,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
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
            mode_display: None,
            mode_index: 0,
            model_display: Some("gpt-4o"),
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(text.contains("aether-acp"), "should contain agent name");
        assert!(text.contains("gpt-4o"), "should contain model name");
    }

    #[test]
    fn renders_without_model_when_none() {
        let mut status = StatusLine {
            agent_name: "aether-acp",
            mode_display: None,
            mode_index: 0,
            model_display: None,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
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
            mode_display: None,
            mode_index: 0,
            model_display: Some("gpt-4o"),
            context_pct_left: Some(72),
            waiting_for_response: false,
            unhealthy_server_count: 0,
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
            mode_display: None,
            mode_index: 0,
            model_display: Some("gpt-4o"),
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
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
            mode_display: None,
            mode_index: 0,
            model_display: Some("gpt-4o"),
            context_pct_left: Some(72),
            waiting_for_response: true,
            unhealthy_server_count: 0,
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
            text.contains("72% context"),
            "should contain context when waiting"
        );
    }

    #[test]
    fn renders_interrupt_message_without_model_when_waiting() {
        let mut status = StatusLine {
            agent_name: "aether",
            mode_display: None,
            mode_index: 0,
            model_display: None,
            context_pct_left: None,
            waiting_for_response: true,
            unhealthy_server_count: 0,
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

    #[test]
    fn renders_unhealthy_server_singular() {
        let mut status = StatusLine {
            agent_name: "aether",
            mode_display: None,
            mode_index: 0,
            model_display: Some("gpt-4o"),
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 1,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        let text = lines[0].plain_text();
        assert!(
            text.contains("1 server needs auth"),
            "should show singular unhealthy message"
        );
    }

    #[test]
    fn renders_unhealthy_servers_plural() {
        let mut status = StatusLine {
            agent_name: "aether",
            mode_display: None,
            mode_index: 0,
            model_display: None,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 3,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        let text = lines[0].plain_text();
        assert!(
            text.contains("3 servers unhealthy"),
            "should show plural unhealthy message"
        );
    }

    #[test]
    fn zero_unhealthy_servers_shows_nothing() {
        let mut status = StatusLine {
            agent_name: "aether",
            mode_display: None,
            mode_index: 0,
            model_display: None,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        let text = lines[0].plain_text();
        assert!(
            !text.contains("server"),
            "should not show server info when count is 0"
        );
    }

    #[test]
    fn context_usage_takes_precedence_over_unhealthy() {
        let mut status = StatusLine {
            agent_name: "aether",
            model_display: None,
            mode_display: None,
            mode_index: 0,
            context_pct_left: Some(50),
            waiting_for_response: false,
            unhealthy_server_count: 2,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        let text = lines[0].plain_text();
        assert!(
            text.contains("50% context"),
            "context should take precedence"
        );
        assert!(
            !text.contains("unhealthy"),
            "should not show unhealthy when context is shown"
        );
    }

    #[test]
    fn renders_agent_mode_model_in_order() {
        let mut status = StatusLine {
            agent_name: "wisp",
            mode_display: Some("Planner"),
            mode_index: 0,
            model_display: Some("gpt-4o"),
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(text.contains("wisp"), "should contain agent name");
        assert!(text.contains("Planner"), "should contain mode");
        assert!(text.contains("gpt-4o"), "should contain model");

        // Verify order: agent name should appear before mode, mode before model
        let agent_index = text.find("wisp").expect("agent position");
        let mode_index = text.find("Planner").expect("mode position");
        let model_index = text.find("gpt-4o").expect("model position");
        assert!(
            agent_index < mode_index,
            "agent should come before mode in status line"
        );
        assert!(
            mode_index < model_index,
            "mode should come before model in status line"
        );
    }

    #[test]
    fn renders_mode_with_colored_text() {
        let mut status = StatusLine {
            agent_name: "wisp",
            mode_display: Some("Planner"),
            mode_index: 0,
            model_display: None,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        assert_eq!(lines.len(), 1);

        let spans = lines[0].spans();
        let mode_span = spans
            .iter()
            .find(|s| s.text().contains("Planner"))
            .expect("should have a span containing the mode");
        let style = mode_span.style();
        assert_eq!(
            style.fg,
            Some(ctx.theme.mode_badge_bg(0)),
            "mode text should be colored by index"
        );
    }

    #[test]
    fn renders_different_mode_text_colors() {
        let mut status1 = StatusLine {
            agent_name: "wisp",
            mode_display: Some("Planner"),
            mode_index: 0,
            model_display: None,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = RenderContext::new((80, 24));
        let lines1 = status1.render(&ctx);
        let spans1 = lines1[0].spans();
        let mode_span1 = spans1
            .iter()
            .find(|s| s.text().contains("Planner"))
            .expect("planner mode span");
        let fg1 = mode_span1.style().fg;

        let mut status2 = StatusLine {
            agent_name: "wisp",
            mode_display: Some("Coder"),
            mode_index: 1,
            model_display: None,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let lines2 = status2.render(&ctx);
        let spans2 = lines2[0].spans();
        let mode_span2 = spans2
            .iter()
            .find(|s| s.text().contains("Coder"))
            .expect("coder mode span");
        let fg2 = mode_span2.style().fg;

        assert_ne!(
            fg1, fg2,
            "different mode indices should have different text colors"
        );
    }
}
