use crate::tui::{Component, Line, RenderContext};
use crossterm::style::Stylize;

pub struct StatusLine<'a> {
    pub agent_name: &'a str,
    pub model_display: Option<&'a str>,
}

impl Component for StatusLine<'_> {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let text = match self.model_display {
            Some(model) => format!("  {} · {}", self.agent_name, model),
            None => format!("  {}", self.agent_name),
        };
        let styled = text.with(context.theme.muted);
        vec![Line::new(format!("{styled}"))]
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
}
