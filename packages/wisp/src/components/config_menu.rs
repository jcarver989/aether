use crate::components::wrap_selection;
use crate::tui::{Component, HandlesInput, InputOutcome, Line, RenderContext, Style};
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
    pub current_raw_value: String,
    pub entry_kind: ConfigMenuEntryKind,
    pub multi_select: bool,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigMenuEntryKind {
    Select,
    McpServers,
    ProviderLogins,
}

#[derive(Debug, Clone)]
pub struct ConfigMenuValue {
    pub value: String,
    pub name: String,
    pub description: Option<String>,
    pub is_disabled: bool,
}

#[derive(Debug)]
pub struct ConfigChange {
    pub config_id: String,
    pub new_value: String,
}

pub enum ConfigMenuAction {
    CloseAll,
    OpenSelectedPicker,
    OpenMcpServers,
    OpenProviderLogins,
    OpenModelSelector,
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
                    .display_name
                    .as_deref()
                    .or_else(|| {
                        entry
                            .values
                            .get(entry.current_value_index)
                            .map(|v| v.name.as_str())
                    })
                    .unwrap_or("?");
                let current_disabled = entry.display_name.is_none()
                    && entry
                        .values
                        .get(entry.current_value_index)
                        .is_some_and(|v| v.is_disabled);
                let text = format!("{}{}: {}", prefix, entry.title, current_name);
                if current_disabled {
                    Line::styled(text, context.theme.muted())
                } else if selected {
                    Line::with_style(
                        text,
                        Style::fg(context.theme.text_primary())
                            .bg_color(context.theme.highlight_bg()),
                    )
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
            KeyCode::Enter => {
                let action = match self.selected_entry() {
                    Some(e) if e.entry_kind == ConfigMenuEntryKind::McpServers => {
                        ConfigMenuAction::OpenMcpServers
                    }
                    Some(e) if e.entry_kind == ConfigMenuEntryKind::ProviderLogins => {
                        ConfigMenuAction::OpenProviderLogins
                    }
                    Some(e) if e.multi_select => ConfigMenuAction::OpenModelSelector,
                    _ => ConfigMenuAction::OpenSelectedPicker,
                };
                InputOutcome::action_and_render(action)
            }
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

                let values: Vec<ConfigMenuValue> = flat_options
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

                let multi_select = opt
                    .meta
                    .as_ref()
                    .and_then(|m| m.get("multi_select"))
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);

                let display_name = if multi_select && select.current_value.0.contains(',') {
                    let parts: Vec<&str> =
                        select.current_value.0.split(',').map(str::trim).collect();

                    let names: Vec<&str> = parts
                        .iter()
                        .filter_map(|val| {
                            values
                                .iter()
                                .find(|v| v.value == *val)
                                .map(|v| v.name.as_str())
                        })
                        .collect();

                    if names.is_empty() {
                        Some(format!("{} models", parts.len()))
                    } else {
                        Some(names.join(", "))
                    }
                } else {
                    None
                };

                Some(ConfigMenuEntry {
                    config_id: opt.id.0.to_string(),
                    title: opt.name.clone(),
                    values,
                    current_value_index,
                    current_raw_value: select.current_value.0.to_string(),
                    entry_kind: ConfigMenuEntryKind::Select,
                    multi_select,
                    display_name,
                })
            })
            .collect();

        Self {
            options: entries,
            selected_index: 0,
        }
    }

    pub fn add_mcp_servers_entry(&mut self, summary: &str) {
        self.options.push(ConfigMenuEntry {
            config_id: "__mcp_servers".to_string(),
            title: "MCP Servers".to_string(),
            values: vec![ConfigMenuValue {
                value: String::new(),
                name: summary.to_string(),
                description: None,
                is_disabled: false,
            }],
            current_value_index: 0,
            current_raw_value: String::new(),
            entry_kind: ConfigMenuEntryKind::McpServers,
            multi_select: false,
            display_name: None,
        });
    }

    pub fn add_provider_logins_entry(&mut self, summary: &str) {
        self.options.push(ConfigMenuEntry {
            config_id: "__provider_logins".to_string(),
            title: "Provider Logins".to_string(),
            values: vec![ConfigMenuValue {
                value: String::new(),
                name: summary.to_string(),
                description: None,
                is_disabled: false,
            }],
            current_value_index: 0,
            current_raw_value: String::new(),
            entry_kind: ConfigMenuEntryKind::ProviderLogins,
            multi_select: false,
            display_name: None,
        });
    }

    pub fn move_selection_up(&mut self) {
        wrap_selection(&mut self.selected_index, self.options.len(), -1);
    }

    pub fn move_selection_down(&mut self) {
        wrap_selection(&mut self.selected_index, self.options.len(), 1);
    }

    pub fn update_options(&mut self, options: &[SessionConfigOption]) {
        let prev_index = self.selected_index;
        *self = Self::from_config_options(options);
        self.selected_index = prev_index.min(self.options.len().saturating_sub(1));
    }

    pub fn selected_entry(&self) -> Option<&ConfigMenuEntry> {
        self.options.get(self.selected_index)
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

    #[test]
    fn multi_select_detected_from_meta() {
        let mut meta = serde_json::Map::new();
        meta.insert("multi_select".to_string(), serde_json::Value::Bool(true));
        let opt = make_select_option("model", "Model", "a", &[("a", "A"), ("b", "B")]).meta(meta);
        let menu = ConfigMenu::from_config_options(&[opt]);
        assert!(menu.options[0].multi_select);
    }

    #[test]
    fn multi_select_false_when_no_meta() {
        let opt = make_select_option("model", "Model", "a", &[("a", "A")]);
        let menu = ConfigMenu::from_config_options(&[opt]);
        assert!(!menu.options[0].multi_select);
    }

    #[test]
    fn multi_select_entry_opens_model_selector() {
        let mut meta = serde_json::Map::new();
        meta.insert("multi_select".to_string(), serde_json::Value::Bool(true));
        let opt = make_select_option("model", "Model", "a", &[("a", "A"), ("b", "B")]).meta(meta);
        let mut menu = ConfigMenu::from_config_options(&[opt]);

        let outcome = menu.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(
            outcome.action,
            Some(ConfigMenuAction::OpenModelSelector)
        ));
    }

    #[test]
    fn multi_select_with_comma_value_shows_model_names() {
        let mut meta = serde_json::Map::new();
        meta.insert("multi_select".to_string(), serde_json::Value::Bool(true));
        let opt = make_select_option("model", "Model", "a,b", &[("a", "Alpha"), ("b", "Beta")])
            .meta(meta);
        let menu = ConfigMenu::from_config_options(&[opt]);
        let display = menu.options[0].display_name.as_deref().unwrap();
        assert!(display.contains("Alpha"), "display: {display}");
        assert!(display.contains("Beta"), "display: {display}");
    }

    #[test]
    fn multi_select_with_display_name_not_dimmed_when_first_value_disabled() {
        let mut menu = ConfigMenu {
            options: vec![ConfigMenuEntry {
                config_id: "model".to_string(),
                title: "Model".to_string(),
                values: vec![
                    ConfigMenuValue {
                        value: "a".to_string(),
                        name: "Alpha".to_string(),
                        description: Some("Unavailable: no key".to_string()),
                        is_disabled: true,
                    },
                    ConfigMenuValue {
                        value: "b".to_string(),
                        name: "Beta".to_string(),
                        description: None,
                        is_disabled: false,
                    },
                ],
                current_value_index: 0, // falls back to 0 since comma value doesn't match
                current_raw_value: "b,a".to_string(),
                entry_kind: ConfigMenuEntryKind::Select,
                multi_select: true,
                display_name: Some("Beta, Alpha".to_string()),
            }],
            selected_index: 0,
        };

        let context = RenderContext::new((80, 24));
        let lines = menu.render(&context);
        // Should have highlight_bg, not muted
        let has_highlight = lines[0]
            .spans()
            .iter()
            .any(|s| s.style().bg == Some(context.theme.highlight_bg()));
        assert!(
            has_highlight,
            "multi-select with display_name should get highlight_bg, not muted"
        );
    }

    #[test]
    fn non_multi_select_has_no_display_name() {
        let opt = make_select_option("model", "Model", "a", &[("a", "A")]);
        let menu = ConfigMenu::from_config_options(&[opt]);
        assert!(menu.options[0].display_name.is_none());
    }
}
