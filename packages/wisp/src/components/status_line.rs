use crate::components::reasoning_bar::reasoning_bar;
use crate::tui::soft_wrap::{display_width_line, display_width_text};
use crate::tui::{Component, Line, RenderContext};
use utils::ReasoningEffort;

pub struct StatusLine<'a> {
    pub agent_name: &'a str,
    pub mode_display: Option<&'a str>,
    pub model_display: Option<&'a str>,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub context_pct_left: Option<u8>,
    pub waiting_for_response: bool,
    pub unhealthy_server_count: usize,
}

impl Component for StatusLine<'_> {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let mut left_line = Line::default();
        let sep = context.theme.text_secondary();

        left_line.push_text("  ");
        left_line.push_styled(self.agent_name, context.theme.info());

        if let Some(mode) = self.mode_display {
            left_line.push_styled(" · ", sep);
            left_line.push_styled(mode, context.theme.secondary());
        }

        if let Some(model) = self.model_display {
            left_line.push_styled(" · ", sep);
            left_line.push_styled(model, context.theme.success());
            left_line.push_text(" ");
            left_line.push_styled(
                reasoning_bar(self.reasoning_effort),
                context.theme.success(),
            );
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

        let width = context.size.width as usize;
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
        let status = StatusLine {
            agent_name: "claude-code",
            mode_display: None,
            model_display: None,
            reasoning_effort: None,
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
        let status = StatusLine {
            agent_name: "test-agent",
            mode_display: None,
            model_display: None,
            reasoning_effort: None,
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
        let status = StatusLine {
            agent_name: "aether-acp",
            mode_display: None,
            model_display: Some("gpt-4o"),
            reasoning_effort: None,
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
        let status = StatusLine {
            agent_name: "aether-acp",
            mode_display: None,
            model_display: None,
            reasoning_effort: None,
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
        let status = StatusLine {
            agent_name: "aether",
            mode_display: None,
            model_display: Some("gpt-4o"),
            reasoning_effort: None,
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
        let status = StatusLine {
            agent_name: "aether",
            mode_display: None,
            model_display: Some("gpt-4o"),
            reasoning_effort: None,
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
        let status = StatusLine {
            agent_name: "aether",
            mode_display: None,
            model_display: Some("gpt-4o"),
            reasoning_effort: None,
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
        let status = StatusLine {
            agent_name: "aether",
            mode_display: None,
            model_display: None,
            reasoning_effort: None,
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
        let status = StatusLine {
            agent_name: "aether",
            mode_display: None,
            model_display: Some("gpt-4o"),
            reasoning_effort: None,
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
        let status = StatusLine {
            agent_name: "aether",
            mode_display: None,
            model_display: None,
            reasoning_effort: None,
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
        let status = StatusLine {
            agent_name: "aether",
            mode_display: None,
            model_display: None,
            reasoning_effort: None,
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
        let status = StatusLine {
            agent_name: "aether",
            model_display: None,
            mode_display: None,
            reasoning_effort: None,
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
        let status = StatusLine {
            agent_name: "wisp",
            mode_display: Some("Planner"),
            model_display: Some("gpt-4o"),
            reasoning_effort: None,
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
        let llm_index = text.find("gpt-4o").expect("model position");
        assert!(
            agent_index < mode_index,
            "agent should come before mode in status line"
        );
        assert!(
            mode_index < llm_index,
            "mode should come before model in status line"
        );
    }

    #[test]
    fn renders_mode_with_secondary_color() {
        let status = StatusLine {
            agent_name: "wisp",
            mode_display: Some("Planner"),
            model_display: None,
            reasoning_effort: None,
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
            Some(ctx.theme.secondary()),
            "mode text should be colored with secondary theme color"
        );
    }

    #[test]
    fn renders_agent_with_info_color() {
        let status = StatusLine {
            agent_name: "wisp",
            mode_display: None,
            model_display: None,
            reasoning_effort: None,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        assert_eq!(lines.len(), 1);

        let spans = lines[0].spans();
        let agent_span = spans
            .iter()
            .find(|s| s.text().contains("wisp"))
            .expect("should have a span containing the agent name");
        let style = agent_span.style();
        assert_eq!(
            style.fg,
            Some(ctx.theme.info()),
            "agent name should be colored with info theme color"
        );
    }

    #[test]
    fn renders_model_with_success_color() {
        let status = StatusLine {
            agent_name: "wisp",
            mode_display: None,
            model_display: Some("gpt-4o"),
            reasoning_effort: None,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        assert_eq!(lines.len(), 1);

        let spans = lines[0].spans();
        let model_span = spans
            .iter()
            .find(|s| s.text().contains("gpt-4o"))
            .expect("should have a span containing the model name");
        let style = model_span.style();
        assert_eq!(
            style.fg,
            Some(ctx.theme.success()),
            "model name should be colored with success theme color"
        );
    }

    #[test]
    fn renders_each_element_with_distinct_color() {
        let status = StatusLine {
            agent_name: "wisp",
            mode_display: Some("Planner"),
            model_display: Some("gpt-4o"),
            reasoning_effort: None,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);

        let spans = lines[0].spans();

        let agent_fg = spans
            .iter()
            .find(|s| s.text().contains("wisp"))
            .map(|s| s.style().fg);
        let mode_fg = spans
            .iter()
            .find(|s| s.text().contains("Planner"))
            .map(|s| s.style().fg);
        let llm_fg = spans
            .iter()
            .find(|s| s.text().contains("gpt-4o"))
            .map(|s| s.style().fg);

        assert_ne!(
            agent_fg, mode_fg,
            "agent and mode should have different colors"
        );
        assert_ne!(
            mode_fg, llm_fg,
            "mode and model should have different colors"
        );
        assert_ne!(
            agent_fg, llm_fg,
            "agent and model should have different colors"
        );
    }

    #[test]
    fn renders_reasoning_bar_next_to_model_when_reasoning_set() {
        let status = StatusLine {
            agent_name: "wisp",
            mode_display: None,
            model_display: Some("gpt-4o"),
            reasoning_effort: Some(ReasoningEffort::Medium),
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        let text = lines[0].plain_text();
        assert!(text.contains("gpt-4o"), "should contain model name");
        assert!(
            text.contains("▰▰▱"),
            "should contain reasoning bar for medium effort"
        );
        // Verify order: model should appear before bar
        let model_index = text.find("gpt-4o").expect("model position");
        let bar_index = text.find("▰▰▱").expect("bar position");
        assert!(
            model_index < bar_index,
            "model should come before reasoning bar"
        );
    }

    #[test]
    fn does_not_render_reasoning_bar_when_model_absent() {
        let status = StatusLine {
            agent_name: "wisp",
            mode_display: None,
            model_display: None,
            reasoning_effort: Some(ReasoningEffort::High),
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        let text = lines[0].plain_text();
        assert!(!text.contains('▰'), "should not contain filled bar chars");
        assert!(!text.contains('▱'), "should not contain empty bar chars");
    }

    #[test]
    fn renders_empty_reasoning_bar_for_none_effort() {
        let status = StatusLine {
            agent_name: "wisp",
            mode_display: None,
            model_display: Some("gpt-4o"),
            reasoning_effort: None,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        let text = lines[0].plain_text();
        assert!(text.contains("▱▱▱"), "should contain empty reasoning bar");
    }

    #[test]
    fn renders_reasoning_bar_with_model_semantic_color() {
        let status = StatusLine {
            agent_name: "wisp",
            mode_display: None,
            model_display: Some("gpt-4o"),
            reasoning_effort: Some(ReasoningEffort::Low),
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);

        let spans = lines[0].spans();
        let bar_span = spans
            .iter()
            .find(|s| s.text().contains("▰"))
            .expect("should have a span containing the reasoning bar");
        let style = bar_span.style();
        assert_eq!(
            style.fg,
            Some(ctx.theme.success()),
            "reasoning bar should use success color (same as model)"
        );
    }

    #[test]
    fn reasoning_bar_mapping() {
        use super::reasoning_bar;

        assert_eq!(reasoning_bar(None), "▱▱▱");
        assert_eq!(reasoning_bar(Some(ReasoningEffort::Low)), "▰▱▱");
        assert_eq!(reasoning_bar(Some(ReasoningEffort::Medium)), "▰▰▱");
        assert_eq!(reasoning_bar(Some(ReasoningEffort::High)), "▰▰▰");
    }
}
