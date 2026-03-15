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
            left_line.push_styled(reasoning_bar(reasoning_effort), context.theme.success());
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
