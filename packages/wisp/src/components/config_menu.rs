use crate::tui::{Component, HandlesInput, InputOutcome, Line, RenderContext};
use agent_client_protocol::{SessionConfigKind, SessionConfigOption, SessionConfigSelectOptions};
use crossterm::event::{KeyCode, KeyEvent};

pub struct ConfigMenu {
    pub options: Vec<ConfigMenuEntry>,
    pub selected_index: usize,
}

pub struct ConfigMenuEntry {
    pub config_id: String,
    pub title: String,
    pub values: Vec<ConfigMenuValue>,
    pub current_value_index: usize,
}

#[derive(Debug, Clone)]
pub struct ConfigMenuValue {
    pub value: String,
    pub name: String,
    pub description: Option<String>,
    pub is_disabled: bool,
}

pub struct ConfigChange {
    pub config_id: String,
    pub new_value: String,
}

pub enum ConfigMenuAction {
    CloseAll,
    OpenSelectedPicker,
}

impl Component for ConfigMenu {
    fn render(&mut self, context: &RenderContext) -> Vec<Line> {
        if self.options.is_empty() {
            return vec![Line::new("  (no config options)".to_string())];
        }

        self.options
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let selected = i == self.selected_index;
                let prefix = if selected { "▶ " } else { "  " };
                let current_name = entry
                    .values
                    .get(entry.current_value_index)
                    .map_or("?", |v| v.name.as_str());
                let current_disabled = entry
                    .values
                    .get(entry.current_value_index)
                    .is_some_and(|v| v.is_disabled);
                let text = format!("{}{}: {}", prefix, entry.title, current_name);
                if current_disabled {
                    Line::styled(text, context.theme.muted)
                } else if selected {
                    Line::styled(text, context.theme.primary)
                } else {
                    Line::new(text)
                }
            })
            .collect()
    }
}

impl HandlesInput for ConfigMenu {
    type Action = ConfigMenuAction;

    fn handle_key(&mut self, key_event: KeyEvent) -> InputOutcome<Self::Action> {
        match key_event.code {
            KeyCode::Esc => InputOutcome::action_and_render(ConfigMenuAction::CloseAll),
            KeyCode::Up => {
                self.move_selection_up();
                InputOutcome::consumed_and_render()
            }
            KeyCode::Down => {
                self.move_selection_down();
                InputOutcome::consumed_and_render()
            }
            KeyCode::Enter => InputOutcome::action_and_render(ConfigMenuAction::OpenSelectedPicker),
            _ => InputOutcome::consumed(),
        }
    }
}

impl ConfigMenu {
    pub fn from_config_options(options: &[SessionConfigOption]) -> Self {
        let entries: Vec<ConfigMenuEntry> = options
            .iter()
            .filter_map(|opt| {
                let SessionConfigKind::Select(ref select) = opt.kind else {
                    return None;
                };
                let flat_options = match &select.options {
                    SessionConfigSelectOptions::Ungrouped(opts) => opts.clone(),
                    SessionConfigSelectOptions::Grouped(groups) => {
                        groups.iter().flat_map(|g| g.options.clone()).collect()
                    }
                    _ => return None,
                };
                if flat_options.is_empty() {
                    return None;
                }
                let current_value_index = flat_options
                    .iter()
                    .position(|o| o.value == select.current_value)
                    .unwrap_or(0);
                let values = flat_options
                    .into_iter()
                    .map(|o| ConfigMenuValue {
                        value: o.value.0.to_string(),
                        name: o.name,
                        is_disabled: o
                            .description
                            .as_deref()
                            .is_some_and(|d| d.starts_with("Unavailable:")),
                        description: o.description,
                    })
                    .collect();
                Some(ConfigMenuEntry {
                    config_id: opt.id.0.to_string(),
                    title: opt.name.clone(),
                    values,
                    current_value_index,
                })
            })
            .collect();

        Self {
            options: entries,
            selected_index: 0,
        }
    }

    pub fn move_selection_up(&mut self) {
        match self.selected_index {
            _ if self.options.is_empty() => {}
            0 => self.selected_index = self.options.len() - 1,
            i => self.selected_index = i - 1,
        }
    }

    pub fn move_selection_down(&mut self) {
        match self.selected_index {
            _ if self.options.is_empty() => {}
            i if i >= self.options.len() - 1 => self.selected_index = 0,
            _ => self.selected_index += 1,
        }
    }

    pub fn update_options(&mut self, options: &[SessionConfigOption]) {
        let prev_index = self.selected_index;
        *self = Self::from_config_options(options);
        self.selected_index = prev_index.min(self.options.len().saturating_sub(1));
    }

    pub fn selected_entry(&self) -> Option<&ConfigMenuEntry> {
        self.options.get(self.selected_index)
    }

    #[allow(dead_code)]
    pub fn entry_by_id(&self, config_id: &str) -> Option<&ConfigMenuEntry> {
        self.options
            .iter()
            .find(|entry| entry.config_id == config_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::{
        SessionConfigOption, SessionConfigOptionCategory, SessionConfigSelectOption,
    };
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn make_select_option(
        id: &str,
        name: &str,
        current: &str,
        values: &[(&str, &str)],
    ) -> SessionConfigOption {
        let options: Vec<SessionConfigSelectOption> = values
            .iter()
            .map(|(v, n)| SessionConfigSelectOption::new(v.to_string(), n.to_string()))
            .collect();
        SessionConfigOption::select(
            id.to_string(),
            name.to_string(),
            current.to_string(),
            options,
        )
    }

    #[test]
    fn from_config_options_builds_entries() {
        let opts = vec![
            make_select_option(
                "model",
                "Model",
                "gpt-4o",
                &[("gpt-4o", "GPT-4o"), ("claude", "Claude")],
            ),
            make_select_option(
                "mode",
                "Mode",
                "code",
                &[("code", "Code"), ("chat", "Chat")],
            ),
        ];
        let menu = ConfigMenu::from_config_options(&opts);
        assert_eq!(menu.options.len(), 2);
        assert_eq!(menu.options[0].config_id, "model");
        assert_eq!(menu.options[0].current_value_index, 0);
        assert_eq!(menu.options[1].config_id, "mode");
    }

    #[test]
    fn from_config_options_finds_current_value() {
        let opts = vec![make_select_option(
            "model",
            "Model",
            "claude",
            &[
                ("gpt-4o", "GPT-4o"),
                ("claude", "Claude"),
                ("llama", "Llama"),
            ],
        )];
        let menu = ConfigMenu::from_config_options(&opts);
        assert_eq!(menu.options[0].current_value_index, 1);
    }

    #[test]
    fn navigation_wraps_around() {
        let opts = vec![
            make_select_option("a", "A", "v1", &[("v1", "V1")]),
            make_select_option("b", "B", "v1", &[("v1", "V1")]),
            make_select_option("c", "C", "v1", &[("v1", "V1")]),
        ];
        let mut menu = ConfigMenu::from_config_options(&opts);
        assert_eq!(menu.selected_index, 0);

        menu.move_selection_up();
        assert_eq!(menu.selected_index, 2);

        menu.move_selection_down();
        assert_eq!(menu.selected_index, 0);

        menu.move_selection_down();
        menu.move_selection_down();
        menu.move_selection_down();
        assert_eq!(menu.selected_index, 0);
    }

    #[test]
    fn update_options_clamps_index() {
        let opts = vec![
            make_select_option("a", "A", "v1", &[("v1", "V1")]),
            make_select_option("b", "B", "v1", &[("v1", "V1")]),
            make_select_option("c", "C", "v1", &[("v1", "V1")]),
        ];
        let mut menu = ConfigMenu::from_config_options(&opts);
        menu.selected_index = 2;

        let fewer = vec![make_select_option("a", "A", "v1", &[("v1", "V1")])];
        menu.update_options(&fewer);
        assert_eq!(menu.selected_index, 0);
    }

    #[test]
    fn update_options_preserves_index_when_within_bounds() {
        let opts = vec![
            make_select_option("provider", "Provider", "a", &[("a", "A"), ("b", "B")]),
            make_select_option("model", "Model", "m1", &[("m1", "M1"), ("m2", "M2")]),
        ];
        let mut menu = ConfigMenu::from_config_options(&opts);
        menu.selected_index = 1; // Select "Model" row

        // Update with different values but same number of rows
        let new_opts = vec![
            make_select_option("provider", "Provider", "b", &[("a", "A"), ("b", "B")]),
            make_select_option("model", "Model", "m3", &[("m3", "M3")]),
        ];
        menu.update_options(&new_opts);
        assert_eq!(menu.selected_index, 1); // Should still be on "Model" row
    }

    #[test]
    fn component_renders_selected_row() {
        let opts = vec![
            make_select_option(
                "model",
                "Model",
                "gpt-4o",
                &[("gpt-4o", "GPT-4o"), ("claude", "Claude")],
            ),
            make_select_option(
                "mode",
                "Mode",
                "code",
                &[("code", "Code"), ("chat", "Chat")],
            ),
        ];
        let mut menu = ConfigMenu::from_config_options(&opts);

        let context = RenderContext::new((80, 24));
        let lines = menu.render(&context);

        assert_eq!(lines.len(), 2);
        // First line is selected (contains ▶)
        assert!(lines[0].plain_text().contains("▶"));
        assert!(lines[0].plain_text().contains("Model"));
        assert!(lines[0].plain_text().contains("GPT-4o"));
        // Second line is not selected
        assert!(lines[1].plain_text().contains("Mode"));
        assert!(lines[1].plain_text().contains("Code"));
        assert!(!lines[1].plain_text().contains("▶"));
    }

    #[test]
    fn empty_options_renders_placeholder() {
        let mut menu = ConfigMenu::from_config_options(&[]);

        let context = RenderContext::new((80, 24));
        let lines = menu.render(&context);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain_text().contains("no config options"));
    }

    #[test]
    fn from_config_options_skips_empty_values() {
        let empty =
            SessionConfigOption::select("x", "X", "v", Vec::<SessionConfigSelectOption>::new());
        let opts = vec![
            empty,
            make_select_option("model", "Model", "a", &[("a", "A")]),
        ];
        let menu = ConfigMenu::from_config_options(&opts);
        assert_eq!(menu.options.len(), 1);
        assert_eq!(menu.options[0].config_id, "model");
    }

    #[test]
    fn from_config_options_with_category() {
        let opt = make_select_option("model", "Model", "gpt-4o", &[("gpt-4o", "GPT-4o")])
            .category(SessionConfigOptionCategory::Model);
        let menu = ConfigMenu::from_config_options(&[opt]);
        assert_eq!(menu.options.len(), 1);
        assert_eq!(menu.options[0].title, "Model");
    }

    #[test]
    fn handle_key_enter_requests_open_picker() {
        let opts = vec![make_select_option("model", "Model", "a", &[("a", "A")])];
        let mut menu = ConfigMenu::from_config_options(&opts);

        let outcome = menu.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(outcome.consumed);
        assert!(outcome.needs_render);
        assert!(matches!(
            outcome.action,
            Some(ConfigMenuAction::OpenSelectedPicker)
        ));
    }

    #[test]
    fn handle_key_escape_requests_close() {
        let opts = vec![make_select_option("model", "Model", "a", &[("a", "A")])];
        let mut menu = ConfigMenu::from_config_options(&opts);

        let outcome = menu.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(outcome.consumed);
        assert!(outcome.needs_render);
        assert!(matches!(outcome.action, Some(ConfigMenuAction::CloseAll)));
    }
}
