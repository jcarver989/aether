use crate::components::reasoning_bar::reasoning_bar;
use crate::tui::{Line, ViewContext, display_width_text};
use acp_utils::config_option_id::ConfigOptionId;
use agent_client_protocol::{
    self as acp, SessionConfigKind, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigSelectOptions,
};
use utils::ReasoningEffort;

pub struct StatusLine<'a> {
    pub agent_name: &'a str,
    pub config_options: &'a [SessionConfigOption],
    pub context_pct_left: Option<u8>,
    pub waiting_for_response: bool,
    pub unhealthy_server_count: usize,
}

impl StatusLine<'_> {
    #[allow(clippy::similar_names)]
    pub fn render(&self, context: &ViewContext) -> Vec<Line> {
        let mode_display = extract_mode_display(self.config_options);
        let model_display = extract_model_display(self.config_options);
        let reasoning_effort = extract_reasoning_effort(self.config_options);

        let mut left_line = Line::default();
        let sep = context.theme.text_secondary();

        left_line.push_text("  ");
        left_line.push_styled(self.agent_name, context.theme.info());

        if let Some(ref mode) = mode_display {
            left_line.push_styled(" · ", sep);
            left_line.push_styled(mode.as_str(), context.theme.secondary());
        }

        if let Some(ref model) = model_display {
            left_line.push_styled(" · ", sep);
            left_line.push_styled(model.as_str(), context.theme.success());
            left_line.push_text(" ");
            left_line.push_styled(
                reasoning_bar(reasoning_effort),
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
        let left_len = left_line.display_width();

        let padding = width.saturating_sub(left_len + right_len);
        left_line.push_text(" ".repeat(padding));
        left_line.push_styled(right, color);
        vec![left_line]
    }
}

pub(crate) fn is_cycleable_mode_option(option: &SessionConfigOption) -> bool {
    matches!(option.kind, SessionConfigKind::Select(_))
        && option.category == Some(SessionConfigOptionCategory::Mode)
}

pub(crate) fn option_display_name(
    options: &SessionConfigSelectOptions,
    current_value: &acp::SessionConfigValueId,
) -> Option<String> {
    match options {
        SessionConfigSelectOptions::Ungrouped(options) => options
            .iter()
            .find(|option| &option.value == current_value)
            .map(|option| option.name.clone()),
        SessionConfigSelectOptions::Grouped(groups) => groups
            .iter()
            .flat_map(|group| group.options.iter())
            .find(|option| &option.value == current_value)
            .map(|option| option.name.clone()),
        _ => None,
    }
}

pub(crate) fn extract_select_display(
    config_options: &[SessionConfigOption],
    id: ConfigOptionId,
) -> Option<String> {
    let option = config_options
        .iter()
        .find(|option| option.id.0.as_ref() == id.as_str())?;

    let SessionConfigKind::Select(ref select) = option.kind else {
        return None;
    };

    option_display_name(&select.options, &select.current_value)
}

pub(crate) fn extract_mode_display(config_options: &[SessionConfigOption]) -> Option<String> {
    extract_select_display(config_options, ConfigOptionId::Mode)
}

pub(crate) fn extract_model_display(config_options: &[SessionConfigOption]) -> Option<String> {
    let option = config_options
        .iter()
        .find(|option| option.id.0.as_ref() == ConfigOptionId::Model.as_str())?;

    let SessionConfigKind::Select(ref select) = option.kind else {
        return None;
    };

    let options = match &select.options {
        SessionConfigSelectOptions::Ungrouped(options) => options,
        SessionConfigSelectOptions::Grouped(_) => {
            return extract_select_display(config_options, ConfigOptionId::Model);
        }
        _ => return None,
    };

    let current = select.current_value.0.as_ref();
    if current.contains(',') {
        let names: Vec<&str> = current
            .split(',')
            .filter_map(|part| {
                let trimmed = part.trim();
                options
                    .iter()
                    .find(|option| option.value.0.as_ref() == trimmed)
                    .map(|option| option.name.as_str())
            })
            .collect();
        if names.is_empty() {
            None
        } else {
            Some(names.join(" + "))
        }
    } else {
        extract_select_display(config_options, ConfigOptionId::Model)
    }
}

pub(crate) fn extract_reasoning_effort(
    config_options: &[SessionConfigOption],
) -> Option<ReasoningEffort> {
    let option = config_options
        .iter()
        .find(|option| option.id.0.as_ref() == ConfigOptionId::ReasoningEffort.as_str())?;

    let SessionConfigKind::Select(ref select) = option.kind else {
        return None;
    };

    ReasoningEffort::parse(&select.current_value.0).unwrap_or(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode_option(value: impl Into<String>, name: impl Into<String>) -> SessionConfigOption {
        let value = value.into();
        let name = name.into();
        SessionConfigOption::select(
            "mode",
            "Mode",
            value.clone(),
            vec![acp::SessionConfigSelectOption::new(value, name)],
        )
        .category(SessionConfigOptionCategory::Mode)
    }

    fn model_option(value: impl Into<String>, name: impl Into<String>) -> SessionConfigOption {
        let value = value.into();
        let name = name.into();
        SessionConfigOption::select(
            "model",
            "Model",
            value.clone(),
            vec![acp::SessionConfigSelectOption::new(value, name)],
        )
        .category(SessionConfigOptionCategory::Model)
    }

    fn reasoning_option(value: impl Into<String>) -> SessionConfigOption {
        let value = value.into();
        SessionConfigOption::select(
            ConfigOptionId::ReasoningEffort.as_str(),
            "Reasoning",
            value,
            vec![
                acp::SessionConfigSelectOption::new("none", "None"),
                acp::SessionConfigSelectOption::new("low", "Low"),
                acp::SessionConfigSelectOption::new("medium", "Medium"),
                acp::SessionConfigSelectOption::new("high", "High"),
            ],
        )
    }

    #[test]
    fn renders_agent_name() {
        let status = StatusLine {
            agent_name: "claude-code",
            config_options: &[],
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = ViewContext::new((80, 24));
        let lines = status.render(&ctx);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain_text().contains("claude-code"));
    }

    #[test]
    fn renders_with_indentation() {
        let status = StatusLine {
            agent_name: "test-agent",
            config_options: &[],
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = ViewContext::new((80, 24));
        let lines = status.render(&ctx);
        // Should have leading spaces for indentation
        assert!(lines[0].plain_text().contains("  test-agent"));
    }

    #[test]
    fn renders_model_display() {
        let options = vec![model_option("gpt-4o", "gpt-4o")];
        let status = StatusLine {
            agent_name: "aether-acp",
            config_options: &options,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = ViewContext::new((80, 24));
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
            config_options: &[],
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = ViewContext::new((80, 24));
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
        let options = vec![model_option("gpt-4o", "gpt-4o")];
        let status = StatusLine {
            agent_name: "aether",
            config_options: &options,
            context_pct_left: Some(72),
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = ViewContext::new((80, 24));
        let lines = status.render(&ctx);
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(text.contains("aether"), "should contain agent name");
        assert!(text.contains("72% context"), "should contain context usage");
    }

    #[test]
    fn does_not_render_context_when_none() {
        let options = vec![model_option("gpt-4o", "gpt-4o")];
        let status = StatusLine {
            agent_name: "aether",
            config_options: &options,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = ViewContext::new((80, 24));
        let lines = status.render(&ctx);
        let text = lines[0].plain_text();
        assert!(!text.contains("context"), "should not contain context info");
    }

    #[test]
    fn renders_interrupt_message_when_waiting() {
        let options = vec![model_option("gpt-4o", "gpt-4o")];
        let status = StatusLine {
            agent_name: "aether",
            config_options: &options,
            context_pct_left: Some(72),
            waiting_for_response: true,
            unhealthy_server_count: 0,
        };
        let ctx = ViewContext::new((80, 24));
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
            config_options: &[],
            context_pct_left: None,
            waiting_for_response: true,
            unhealthy_server_count: 0,
        };
        let ctx = ViewContext::new((80, 24));
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
        let options = vec![model_option("gpt-4o", "gpt-4o")];
        let status = StatusLine {
            agent_name: "aether",
            config_options: &options,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 1,
        };
        let ctx = ViewContext::new((80, 24));
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
            config_options: &[],
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 3,
        };
        let ctx = ViewContext::new((80, 24));
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
            config_options: &[],
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = ViewContext::new((80, 24));
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
            config_options: &[],
            context_pct_left: Some(50),
            waiting_for_response: false,
            unhealthy_server_count: 2,
        };
        let ctx = ViewContext::new((80, 24));
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
        let options = vec![
            mode_option("planner", "Planner"),
            model_option("gpt-4o", "gpt-4o"),
        ];
        let status = StatusLine {
            agent_name: "wisp",
            config_options: &options,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = ViewContext::new((80, 24));
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
        let options = vec![mode_option("planner", "Planner")];
        let status = StatusLine {
            agent_name: "wisp",
            config_options: &options,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = ViewContext::new((80, 24));
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
            config_options: &[],
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = ViewContext::new((80, 24));
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
        let options = vec![model_option("gpt-4o", "gpt-4o")];
        let status = StatusLine {
            agent_name: "wisp",
            config_options: &options,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = ViewContext::new((80, 24));
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
        let options = vec![
            mode_option("planner", "Planner"),
            model_option("gpt-4o", "gpt-4o"),
        ];
        let status = StatusLine {
            agent_name: "wisp",
            config_options: &options,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = ViewContext::new((80, 24));
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
        let options = vec![
            model_option("gpt-4o", "gpt-4o"),
            reasoning_option("medium"),
        ];
        let status = StatusLine {
            agent_name: "wisp",
            config_options: &options,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = ViewContext::new((80, 24));
        let lines = status.render(&ctx);
        let text = lines[0].plain_text();
        assert!(text.contains("gpt-4o"), "should contain model name");
        assert!(
            text.contains("[■■·]"),
            "should contain reasoning bar for medium effort"
        );
        // Verify order: model should appear before bar
        let model_index = text.find("gpt-4o").expect("model position");
        let bar_index = text.find("[■■·]").expect("bar position");
        assert!(
            model_index < bar_index,
            "model should come before reasoning bar"
        );
    }

    #[test]
    fn does_not_render_reasoning_bar_when_model_absent() {
        let options = vec![reasoning_option("high")];
        let status = StatusLine {
            agent_name: "wisp",
            config_options: &options,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = ViewContext::new((80, 24));
        let lines = status.render(&ctx);
        let text = lines[0].plain_text();
        assert!(!text.contains('■'), "should not contain filled bar chars");
        assert!(!text.contains('·'), "should not contain empty bar chars");
    }

    #[test]
    fn renders_empty_reasoning_bar_for_none_effort() {
        let options = vec![model_option("gpt-4o", "gpt-4o")];
        let status = StatusLine {
            agent_name: "wisp",
            config_options: &options,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = ViewContext::new((80, 24));
        let lines = status.render(&ctx);
        let text = lines[0].plain_text();
        assert!(text.contains("[···]"), "should contain empty reasoning bar");
    }

    #[test]
    fn renders_reasoning_bar_with_model_semantic_color() {
        let options = vec![
            model_option("gpt-4o", "gpt-4o"),
            reasoning_option("low"),
        ];
        let status = StatusLine {
            agent_name: "wisp",
            config_options: &options,
            context_pct_left: None,
            waiting_for_response: false,
            unhealthy_server_count: 0,
        };
        let ctx = ViewContext::new((80, 24));
        let lines = status.render(&ctx);

        let spans = lines[0].spans();
        let bar_span = spans
            .iter()
            .find(|s| s.text().contains("■"))
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

        assert_eq!(reasoning_bar(None), "[···]");
        assert_eq!(reasoning_bar(Some(ReasoningEffort::Low)), "[■··]");
        assert_eq!(reasoning_bar(Some(ReasoningEffort::Medium)), "[■■·]");
        assert_eq!(reasoning_bar(Some(ReasoningEffort::High)), "[■■■]");
    }
}
