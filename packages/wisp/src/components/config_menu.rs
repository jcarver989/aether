use crate::tui::{
    Component, Event, Frame, Line, SelectItem, SelectList, SelectListMessage, ViewContext,
};
use acp_utils::config_meta::{ConfigOptionMeta, SelectOptionMeta};
use acp_utils::config_option_id::{ConfigOptionId, THEME_CONFIG_ID};
use agent_client_protocol::{SessionConfigKind, SessionConfigOption, SessionConfigSelectOptions};

pub struct ConfigMenu {
    list: SelectList<ConfigMenuEntry>,
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
    pub meta: SelectOptionMeta,
}

#[derive(Debug)]
pub struct ConfigChange {
    pub config_id: String,
    pub new_value: String,
}

pub enum ConfigMenuMessage {
    CloseAll,
    OpenSelectedPicker,
    OpenMcpServers,
    OpenProviderLogins,
    OpenModelSelector,
}

impl SelectItem for ConfigMenuEntry {
    fn render_item(&self, selected: bool, ctx: &ViewContext) -> Line {
        let prefix = if selected { "▶ " } else { "  " };
        let current_name = self
            .display_name
            .as_deref()
            .or_else(|| {
                self.values
                    .get(self.current_value_index)
                    .map(|v| v.name.as_str())
            })
            .unwrap_or("?");
        let current_disabled = self.display_name.is_none()
            && self
                .values
                .get(self.current_value_index)
                .is_some_and(|v| v.is_disabled);
        let text = format!("{}{}: {}", prefix, self.title, current_name);
        if current_disabled {
            Line::styled(text, ctx.theme.muted())
        } else if selected {
            Line::with_style(text, ctx.theme.selected_row_style())
        } else {
            Line::new(text)
        }
    }
}

impl Component for ConfigMenu {
    type Message = ConfigMenuMessage;

    async fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        let outcome = self.list.on_event(event).await;
        match outcome.as_deref() {
            Some([SelectListMessage::Close]) => Some(vec![ConfigMenuMessage::CloseAll]),
            Some([SelectListMessage::Select(_)]) => {
                let msg = match self.list.selected_item() {
                    Some(e) if e.entry_kind == ConfigMenuEntryKind::McpServers => {
                        ConfigMenuMessage::OpenMcpServers
                    }
                    Some(e) if e.entry_kind == ConfigMenuEntryKind::ProviderLogins => {
                        ConfigMenuMessage::OpenProviderLogins
                    }
                    Some(e) if e.multi_select => ConfigMenuMessage::OpenModelSelector,
                    _ => ConfigMenuMessage::OpenSelectedPicker,
                };
                Some(vec![msg])
            }
            _ => outcome.map(|_| vec![]),
        }
    }

    fn render(&mut self, context: &ViewContext) -> Frame {
        self.list.render(context)
    }
}

impl ConfigMenu {
    pub fn from_config_options(options: &[SessionConfigOption]) -> Self {
        let entries: Vec<ConfigMenuEntry> = options
            .iter()
            .filter(|opt| opt.id.0.as_ref() != ConfigOptionId::ReasoningEffort.as_str())
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
                        meta: SelectOptionMeta::from_meta(o.meta.as_ref()),
                    })
                    .collect();

                let multi_select = ConfigOptionMeta::from_meta(opt.meta.as_ref()).multi_select;

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
            list: SelectList::new(entries, "no config options"),
        }
    }

    #[allow(dead_code)] // Used by integration tests
    pub fn from_entries(entries: Vec<ConfigMenuEntry>) -> Self {
        Self {
            list: SelectList::new(entries, "no config options"),
        }
    }

    #[cfg(test)]
    pub fn options(&self) -> &[ConfigMenuEntry] {
        self.list.items()
    }

    #[cfg(test)]
    pub fn selected_index(&self) -> usize {
        self.list.selected_index()
    }

    pub fn add_theme_entry(&mut self, current_theme_file: Option<&str>, theme_files: &[String]) {
        let mut values = Vec::with_capacity(theme_files.len() + 1);
        values.push(ConfigMenuValue {
            value: String::new(),
            name: "Default".to_string(),
            description: None,
            is_disabled: false,
            meta: SelectOptionMeta::default(),
        });

        values.extend(theme_files.iter().map(|file| ConfigMenuValue {
            value: file.clone(),
            name: file.clone(),
            description: None,
            is_disabled: false,
            meta: SelectOptionMeta::default(),
        }));

        let current_value_index = current_theme_file
            .and_then(|file| values.iter().position(|v| v.value == file))
            .unwrap_or(0);
        let current_raw_value = values
            .get(current_value_index)
            .map(|v| v.value.clone())
            .unwrap_or_default();

        self.list.push(ConfigMenuEntry {
            config_id: THEME_CONFIG_ID.to_string(),
            title: "Theme".to_string(),
            values,
            current_value_index,
            current_raw_value,
            entry_kind: ConfigMenuEntryKind::Select,
            multi_select: false,
            display_name: None,
        });
    }

    pub fn add_mcp_servers_entry(&mut self, summary: &str) {
        self.list.push(ConfigMenuEntry {
            config_id: "__mcp_servers".to_string(),
            title: "MCP Servers".to_string(),
            values: vec![ConfigMenuValue {
                value: String::new(),
                name: summary.to_string(),
                description: None,
                is_disabled: false,
                meta: SelectOptionMeta::default(),
            }],
            current_value_index: 0,
            current_raw_value: String::new(),
            entry_kind: ConfigMenuEntryKind::McpServers,
            multi_select: false,
            display_name: None,
        });
    }

    pub fn add_provider_logins_entry(&mut self, summary: &str) {
        self.list.push(ConfigMenuEntry {
            config_id: "__provider_logins".to_string(),
            title: "Provider Logins".to_string(),
            values: vec![ConfigMenuValue {
                value: String::new(),
                name: summary.to_string(),
                description: None,
                is_disabled: false,
                meta: SelectOptionMeta::default(),
            }],
            current_value_index: 0,
            current_raw_value: String::new(),
            entry_kind: ConfigMenuEntryKind::ProviderLogins,
            multi_select: false,
            display_name: None,
        });
    }

    pub fn update_options(&mut self, options: &[SessionConfigOption]) {
        let prev_index = self.list.selected_index();
        *self = Self::from_config_options(options);
        let max = self.list.len().saturating_sub(1);
        self.list.set_selected(prev_index.min(max));
    }

    pub fn selected_entry(&self) -> Option<&ConfigMenuEntry> {
        self.list.selected_item()
    }

    pub fn apply_change(&mut self, change: &ConfigChange) {
        let Some(entry) = self
            .list
            .items_mut()
            .iter_mut()
            .find(|entry| entry.config_id == change.config_id)
        else {
            return;
        };

        entry.current_raw_value.clone_from(&change.new_value);
        if let Some(index) = entry
            .values
            .iter()
            .position(|value| value.value == change.new_value)
        {
            entry.current_value_index = index;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{KeyCode, KeyEvent, KeyModifiers};
    use agent_client_protocol::{
        SessionConfigOption, SessionConfigOptionCategory, SessionConfigSelectOption,
    };

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
        assert_eq!(menu.options().len(), 2);
        assert_eq!(menu.options()[0].config_id, "model");
        assert_eq!(menu.options()[0].current_value_index, 0);
        assert_eq!(menu.options()[1].config_id, "mode");
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
        assert_eq!(menu.options()[0].current_value_index, 1);
    }

    #[tokio::test]
    async fn navigation_wraps_around() {
        let opts = vec![
            make_select_option("a", "A", "v1", &[("v1", "V1")]),
            make_select_option("b", "B", "v1", &[("v1", "V1")]),
            make_select_option("c", "C", "v1", &[("v1", "V1")]),
        ];
        let mut menu = ConfigMenu::from_config_options(&opts);
        assert_eq!(menu.selected_index(), 0);

        menu.on_event(&Event::Key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE))).await;
        assert_eq!(menu.selected_index(), 2);

        menu.on_event(&Event::Key(KeyEvent::new(
            KeyCode::Down,
            KeyModifiers::NONE,
        ))).await;
        assert_eq!(menu.selected_index(), 0);

        menu.on_event(&Event::Key(KeyEvent::new(
            KeyCode::Down,
            KeyModifiers::NONE,
        ))).await;
        menu.on_event(&Event::Key(KeyEvent::new(
            KeyCode::Down,
            KeyModifiers::NONE,
        ))).await;
        menu.on_event(&Event::Key(KeyEvent::new(
            KeyCode::Down,
            KeyModifiers::NONE,
        ))).await;
        assert_eq!(menu.selected_index(), 0);
    }

    #[test]
    fn update_options_clamps_index() {
        let opts = vec![
            make_select_option("a", "A", "v1", &[("v1", "V1")]),
            make_select_option("b", "B", "v1", &[("v1", "V1")]),
            make_select_option("c", "C", "v1", &[("v1", "V1")]),
        ];
        let mut menu = ConfigMenu::from_config_options(&opts);
        menu.list.set_selected(2);

        let fewer = vec![make_select_option("a", "A", "v1", &[("v1", "V1")])];
        menu.update_options(&fewer);
        assert_eq!(menu.selected_index(), 0);
    }

    #[test]
    fn update_options_preserves_index_when_within_bounds() {
        let opts = vec![
            make_select_option("provider", "Provider", "a", &[("a", "A"), ("b", "B")]),
            make_select_option("model", "Model", "m1", &[("m1", "M1"), ("m2", "M2")]),
        ];
        let mut menu = ConfigMenu::from_config_options(&opts);
        menu.list.set_selected(1);

        let new_opts = vec![
            make_select_option("provider", "Provider", "b", &[("a", "A"), ("b", "B")]),
            make_select_option("model", "Model", "m3", &[("m3", "M3")]),
        ];
        menu.update_options(&new_opts);
        assert_eq!(menu.selected_index(), 1);
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
        assert_eq!(menu.options().len(), 1);
        assert_eq!(menu.options()[0].config_id, "model");
    }

    #[test]
    fn from_config_options_with_category() {
        let opt = make_select_option("model", "Model", "gpt-4o", &[("gpt-4o", "GPT-4o")])
            .category(SessionConfigOptionCategory::Model);
        let menu = ConfigMenu::from_config_options(&[opt]);
        assert_eq!(menu.options().len(), 1);
        assert_eq!(menu.options()[0].title, "Model");
    }

    #[tokio::test]
    async fn handle_key_enter_requests_open_picker() {
        let opts = vec![make_select_option("model", "Model", "a", &[("a", "A")])];
        let mut menu = ConfigMenu::from_config_options(&opts);

        let outcome = menu.on_event(&Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        ))).await;

        assert!(outcome.is_some());

        let messages = outcome.unwrap();
        assert!(matches!(
            messages.as_slice(),
            [ConfigMenuMessage::OpenSelectedPicker]
        ));
    }

    #[tokio::test]
    async fn handle_key_escape_requests_close() {
        let opts = vec![make_select_option("model", "Model", "a", &[("a", "A")])];
        let mut menu = ConfigMenu::from_config_options(&opts);

        let outcome = menu.on_event(&Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))).await;

        assert!(outcome.is_some());

        let messages = outcome.unwrap();
        assert!(matches!(messages.as_slice(), [ConfigMenuMessage::CloseAll]));
    }

    #[test]
    fn multi_select_detected_from_meta() {
        let meta = ConfigOptionMeta { multi_select: true };
        let opt = make_select_option("model", "Model", "a", &[("a", "A"), ("b", "B")])
            .meta(meta.into_meta());
        let menu = ConfigMenu::from_config_options(&[opt]);
        assert!(menu.options()[0].multi_select);
    }

    #[test]
    fn multi_select_false_when_no_meta() {
        let opt = make_select_option("model", "Model", "a", &[("a", "A")]);
        let menu = ConfigMenu::from_config_options(&[opt]);
        assert!(!menu.options()[0].multi_select);
    }

    #[tokio::test]
    async fn multi_select_entry_opens_model_selector() {
        let meta = ConfigOptionMeta { multi_select: true };
        let opt = make_select_option("model", "Model", "a", &[("a", "A"), ("b", "B")])
            .meta(meta.into_meta());
        let mut menu = ConfigMenu::from_config_options(&[opt]);

        let outcome = menu.on_event(&Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        ))).await;
        let messages = outcome.unwrap();
        assert!(matches!(
            messages.as_slice(),
            [ConfigMenuMessage::OpenModelSelector]
        ));
    }

    #[test]
    fn multi_select_with_comma_value_shows_model_names() {
        let meta = ConfigOptionMeta { multi_select: true };
        let opt = make_select_option("model", "Model", "a,b", &[("a", "Alpha"), ("b", "Beta")])
            .meta(meta.into_meta());
        let menu = ConfigMenu::from_config_options(&[opt]);
        let display = menu.options()[0].display_name.as_deref().unwrap();
        assert!(display.contains("Alpha"), "display: {display}");
        assert!(display.contains("Beta"), "display: {display}");
    }

    #[test]
    fn apply_change_updates_matching_entry_value_and_index() {
        let mut menu = ConfigMenu::from_config_options(&[]);
        let files = vec!["catppuccin.tmTheme".to_string(), "nord.tmTheme".to_string()];
        menu.add_theme_entry(None, &files);

        menu.apply_change(&ConfigChange {
            config_id: THEME_CONFIG_ID.to_string(),
            new_value: "nord.tmTheme".to_string(),
        });

        let theme = &menu.options()[0];
        assert_eq!(theme.current_raw_value, "nord.tmTheme");
        assert_eq!(theme.current_value_index, 2);
    }

    #[test]
    fn add_theme_entry_inserts_theme_row() {
        let mut menu = ConfigMenu::from_config_options(&[]);
        let files = vec!["catppuccin.tmTheme".to_string(), "nord.tmTheme".to_string()];

        menu.add_theme_entry(None, &files);

        assert_eq!(menu.options().len(), 1);
        let theme = &menu.options()[0];
        assert_eq!(theme.config_id, THEME_CONFIG_ID);
        assert_eq!(theme.title, "Theme");
        assert_eq!(theme.entry_kind, ConfigMenuEntryKind::Select);
        assert!(!theme.multi_select);
        assert_eq!(theme.values.len(), 3);
        assert_eq!(theme.values[0].name, "Default");
        assert_eq!(theme.values[0].value, "");
        assert_eq!(theme.values[1].value, "catppuccin.tmTheme");
        assert_eq!(theme.values[2].value, "nord.tmTheme");
    }

    #[test]
    fn add_theme_entry_selects_default_when_current_none() {
        let mut menu = ConfigMenu::from_config_options(&[]);
        let files = vec!["catppuccin.tmTheme".to_string()];

        menu.add_theme_entry(None, &files);

        let theme = &menu.options()[0];
        assert_eq!(theme.current_value_index, 0);
        assert_eq!(theme.current_raw_value, "");
    }

    #[test]
    fn add_theme_entry_selects_matching_theme_file() {
        let mut menu = ConfigMenu::from_config_options(&[]);
        let files = vec!["catppuccin.tmTheme".to_string(), "nord.tmTheme".to_string()];

        menu.add_theme_entry(Some("nord.tmTheme"), &files);

        let theme = &menu.options()[0];
        assert_eq!(theme.current_value_index, 2);
        assert_eq!(theme.current_raw_value, "nord.tmTheme");
    }

    #[test]
    fn add_theme_entry_falls_back_to_default_when_current_missing() {
        let mut menu = ConfigMenu::from_config_options(&[]);
        let files = vec!["catppuccin.tmTheme".to_string()];

        menu.add_theme_entry(Some("missing.tmTheme"), &files);

        let theme = &menu.options()[0];
        assert_eq!(theme.current_value_index, 0);
        assert_eq!(theme.current_raw_value, "");
    }

    #[test]
    fn non_multi_select_has_no_display_name() {
        let opt = make_select_option("model", "Model", "a", &[("a", "A")]);
        let menu = ConfigMenu::from_config_options(&[opt]);
        assert!(menu.options()[0].display_name.is_none());
    }

    #[test]
    fn from_config_options_excludes_reasoning_effort_entry() {
        let opts = vec![
            make_select_option(
                "model",
                "Model",
                "gpt-4o",
                &[("gpt-4o", "GPT-4o"), ("claude", "Claude")],
            ),
            make_select_option(
                "reasoning_effort",
                "Reasoning Effort",
                "high",
                &[
                    ("none", "None"),
                    ("low", "Low"),
                    ("medium", "Medium"),
                    ("high", "High"),
                ],
            ),
        ];
        let menu = ConfigMenu::from_config_options(&opts);

        assert!(
            menu.options().iter().any(|e| e.config_id == "model"),
            "menu should contain model entry"
        );

        assert!(
            !menu
                .options()
                .iter()
                .any(|e| e.config_id == "reasoning_effort"),
            "menu should NOT contain reasoning_effort entry"
        );
    }
}
