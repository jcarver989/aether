use crate::tui::{Component, Line, RenderContext};
use crossterm::style::Stylize;

pub struct StatusLine<'a> {
    pub agent_name: &'a str,
    pub model_display: Option<&'a str>,
    pub context_pct_left: Option<u8>,
}

impl Component for StatusLine<'_> {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let left = match self.model_display {
            Some(model) => format!("  {} · {}", self.agent_name, model),
            None => format!("  {}", self.agent_name),
        };

        let Some(pct) = self.context_pct_left else {
            let styled = left.with(context.theme.muted);
            return vec![Line::new(format!("{styled}"))];
        };

        let right = format!("{}% context", pct);
        let width = context.size.0 as usize;
        let left_visible_len = left.len();
        let right_visible_len = right.len();

        let color = if pct <= 15 {
            context.theme.warning
        } else {
            context.theme.muted
        };

        let padding = width.saturating_sub(left_visible_len + right_visible_len);
        let styled_left = left.with(context.theme.muted);
        let styled_right = right.with(color);
        vec![Line::new(format!(
            "{styled_left}{:padding$}{styled_right}",
            "",
        ))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_agent_name() {
        let status = StatusLine {
            agent_name: "claude-code",
            model_display: None,
            context_pct_left: None,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].as_str().contains("claude-code"));
    }

    #[test]
    fn renders_with_indentation() {
        let status = StatusLine {
            agent_name: "test-agent",
            model_display: None,
            context_pct_left: None,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        // Should have leading spaces for indentation
        assert!(lines[0].as_str().contains("  test-agent"));
    }

    #[test]
    fn renders_model_display() {
        let status = StatusLine {
            agent_name: "aether-acp",
            model_display: Some("gpt-4o"),
            context_pct_left: None,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        assert_eq!(lines.len(), 1);
        let text = lines[0].as_str();
        assert!(text.contains("aether-acp"), "should contain agent name");
        assert!(text.contains("gpt-4o"), "should contain model name");
        assert!(
            text.contains("·"),
            "should contain separator between agent and model"
        );
    }

    #[test]
    fn renders_without_model_when_none() {
        let status = StatusLine {
            agent_name: "aether-acp",
            model_display: None,
            context_pct_left: None,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        let text = lines[0].as_str();
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
            model_display: Some("gpt-4o"),
            context_pct_left: Some(72),
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        assert_eq!(lines.len(), 1);
        let text = lines[0].as_str();
        assert!(text.contains("aether"), "should contain agent name");
        assert!(text.contains("72% context"), "should contain context usage");
    }

    #[test]
    fn does_not_render_context_when_none() {
        let status = StatusLine {
            agent_name: "aether",
            model_display: Some("gpt-4o"),
            context_pct_left: None,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = status.render(&ctx);
        let text = lines[0].as_str();
        assert!(!text.contains("context"), "should not contain context info");
    }
}
