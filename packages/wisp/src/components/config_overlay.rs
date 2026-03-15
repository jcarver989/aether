use crate::components::config_menu::{ConfigChange, ConfigMenu, ConfigMenuMessage};
use crate::components::config_picker::{ConfigPicker, ConfigPickerMessage};
use crate::components::model_selector::{ModelSelector, ModelSelectorMessage};
use crate::components::provider_login::{
    ProviderLoginEntry, ProviderLoginMessage, ProviderLoginOverlay, ProviderLoginStatus,
    provider_login_summary,
};
use crate::components::server_status::{
    ServerStatusMessage, ServerStatusOverlay, server_status_summary,
};
use crate::settings::{list_theme_files, load_or_create_settings, save_settings};
use crate::tui::Panel;
use crate::tui::{Component, Event, Frame, Line, ViewContext};
use acp_utils::config_option_id::{ConfigOptionId, THEME_CONFIG_ID};
use acp_utils::notifications::McpServerStatusEntry;
use agent_client_protocol::{self as acp, SessionConfigKind, SessionConfigOption};
use unicode_width::UnicodeWidthStr;

const MIN_HEIGHT: usize = 3;
const MIN_WIDTH: usize = 6;
/// Panel chrome above child content: top border (1) + blank line (1).
const TOP_CHROME: usize = 2;
/// Panel left border width: "│ " = 2 chars.
const BORDER_LEFT_WIDTH: usize = 2;
/// Gap between children inside the container.
const GAP: usize = 1;

enum ConfigPane {
    Menu,
    Picker(ConfigPicker),
    ModelSelector(ModelSelector),
    ServerStatus(ServerStatusOverlay),
    ProviderLogin(ProviderLoginOverlay),
}

pub struct ConfigOverlay {
    menu: ConfigMenu,
    active_pane: ConfigPane,
    server_statuses: Vec<McpServerStatusEntry>,
    auth_methods: Vec<acp::AuthMethod>,
    current_reasoning_effort: Option<String>,
}

#[derive(Debug)]
pub enum ConfigOverlayMessage {
    Close,
    SetConfigOption { config_id: String, value: String },
    SetTheme(crate::tui::Theme),
    AuthenticateServer(String),
    AuthenticateProvider(String),
}

impl ConfigOverlay {
    pub fn new(
        menu: ConfigMenu,
        server_statuses: Vec<McpServerStatusEntry>,
        auth_methods: Vec<acp::AuthMethod>,
    ) -> Self {
        Self {
            menu,
            active_pane: ConfigPane::Menu,
            server_statuses,
            auth_methods,
            current_reasoning_effort: None,
        }
    }

    pub fn with_reasoning_effort_from_options(mut self, options: &[SessionConfigOption]) -> Self {
        self.current_reasoning_effort = Self::extract_reasoning_effort(options);
        self
    }

    fn extract_reasoning_effort(options: &[SessionConfigOption]) -> Option<String> {
        options
            .iter()
            .find(|opt| opt.id.0.as_ref() == ConfigOptionId::ReasoningEffort.as_str())
            .and_then(|opt| match &opt.kind {
                SessionConfigKind::Select(select) => {
                    let value = select.current_value.0.trim();
                    (!value.is_empty() && value != "none").then(|| value.to_string())
                }
                _ => None,
            })
    }

    pub fn update_child_viewport(&mut self, max_height: usize) {
        match &mut self.active_pane {
            ConfigPane::ModelSelector(ms) => ms.update_viewport(max_height),
            ConfigPane::Picker(p) => p.update_viewport(max_height),
            _ => {}
        }
    }

    pub fn update_config_options(&mut self, options: &[SessionConfigOption]) {
        self.current_reasoning_effort = Self::extract_reasoning_effort(options);

        self.menu.update_options(options);

        let settings = load_or_create_settings();
        let theme_files = list_theme_files();
        self.menu
            .add_theme_entry(settings.theme.file.as_deref(), &theme_files);

        let summary = server_status_summary(&self.server_statuses);
        self.menu.add_mcp_servers_entry(&summary);
        if !self.auth_methods.is_empty() {
            let login_entries = self.build_login_entries();
            let login_summary = provider_login_summary(&login_entries);
            self.menu.add_provider_logins_entry(&login_summary);
        }
    }

    pub fn update_server_statuses(&mut self, statuses: Vec<McpServerStatusEntry>) {
        self.server_statuses = statuses;
        if let ConfigPane::ServerStatus(ref mut overlay) = self.active_pane {
            overlay.update_entries(self.server_statuses.clone());
        }
    }

    pub fn on_authenticate_started(&mut self, method_id: &str) {
        if let ConfigPane::ProviderLogin(ref mut overlay) = self.active_pane {
            overlay.set_authenticating(method_id);
        }
    }

    pub fn remove_auth_method(&mut self, method_id: &str) {
        self.auth_methods.retain(|m| m.id().0.as_ref() != method_id);
        if let ConfigPane::ProviderLogin(ref mut overlay) = self.active_pane {
            overlay.remove_entry(method_id);
            if overlay.is_empty() {
                self.active_pane = ConfigPane::Menu;
            }
        }
    }

    fn build_login_entries(&self) -> Vec<ProviderLoginEntry> {
        self.auth_methods
            .iter()
            .map(|m| ProviderLoginEntry {
                method_id: m.id().0.to_string(),
                name: m.name().to_string(),
                status: ProviderLoginStatus::NeedsLogin,
            })
            .collect()
    }

    pub fn on_authenticate_failed(&mut self, method_id: &str) {
        if let ConfigPane::ProviderLogin(ref mut overlay) = self.active_pane {
            overlay.reset_to_needs_login(method_id);
        }
    }

    pub fn cursor_col(&self) -> usize {
        match &self.active_pane {
            ConfigPane::Picker(picker) => {
                let prefix = format!("  {} search: ", picker.title);
                BORDER_LEFT_WIDTH
                    + UnicodeWidthStr::width(prefix.as_str())
                    + UnicodeWidthStr::width(picker.query())
            }
            ConfigPane::ModelSelector(selector) => {
                let prefix = "  Model search: ";
                BORDER_LEFT_WIDTH
                    + UnicodeWidthStr::width(prefix)
                    + UnicodeWidthStr::width(selector.query())
            }
            _ => 0,
        }
    }

    /// Returns the row offset of the cursor within the overlay (0-indexed from top of overlay).
    /// Only meaningful when a search-based submenu is open (picker or model selector).
    pub fn cursor_row_offset(&self) -> usize {
        match &self.active_pane {
            ConfigPane::Picker(_) | ConfigPane::ModelSelector(_) => TOP_CHROME,
            _ => 0,
        }
    }

    pub fn has_picker(&self) -> bool {
        matches!(self.active_pane, ConfigPane::Picker(_))
    }

    fn process_config_changes(changes: Vec<ConfigChange>) -> Vec<ConfigOverlayMessage> {
        let mut messages = Vec::new();
        for change in changes {
            if change.config_id == THEME_CONFIG_ID {
                let file = theme_file_from_picker_value(&change.new_value);
                let mut settings = load_or_create_settings();
                settings.theme.file = file;
                if let Err(err) = save_settings(&settings) {
                    tracing::warn!("Failed to persist theme setting: {err}");
                }
                let theme = crate::settings::load_theme(&settings);
                messages.push(ConfigOverlayMessage::SetTheme(theme));
            } else {
                messages.push(ConfigOverlayMessage::SetConfigOption {
                    config_id: change.config_id,
                    value: change.new_value,
                });
            }
        }
        messages
    }

    fn footer_text(&self) -> &'static str {
        match &self.active_pane {
            ConfigPane::ModelSelector(_) => {
                "[Space/Enter] Toggle  [\u{2190}/\u{2192}] Reasoning  [Esc] Done"
            }
            ConfigPane::Picker(_) => "[Enter] Confirm  [Esc] Back",
            ConfigPane::ServerStatus(_) | ConfigPane::ProviderLogin(_) => {
                "[Enter] Authenticate  [Esc] Back"
            }
            ConfigPane::Menu => "[Enter] Select  [Esc] Close",
        }
    }
}

impl Component for ConfigOverlay {
    type Message = ConfigOverlayMessage;

    #[allow(clippy::too_many_lines)]
    async fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        let Event::Key(_key) = event else {
            return None;
        };

        match &mut self.active_pane {
            ConfigPane::ServerStatus(overlay) => {
                let outcome = overlay.on_event(event).await;
                match outcome.unwrap_or_default().into_iter().next() {
                    Some(ServerStatusMessage::Close) => {
                        self.active_pane = ConfigPane::Menu;
                        Some(vec![])
                    }
                    Some(ServerStatusMessage::Authenticate(name)) => {
                        Some(vec![ConfigOverlayMessage::AuthenticateServer(name)])
                    }
                    None => Some(vec![]),
                }
            }
            ConfigPane::ProviderLogin(overlay) => {
                let outcome = overlay.on_event(event).await;
                match outcome.unwrap_or_default().into_iter().next() {
                    Some(ProviderLoginMessage::Close) => {
                        self.active_pane = ConfigPane::Menu;
                        Some(vec![])
                    }
                    Some(ProviderLoginMessage::Authenticate(method_id)) => {
                        Some(vec![ConfigOverlayMessage::AuthenticateProvider(method_id)])
                    }
                    None => Some(vec![]),
                }
            }
            ConfigPane::ModelSelector(selector) => {
                let outcome = selector.on_event(event).await;
                match outcome.unwrap_or_default().into_iter().next() {
                    Some(ModelSelectorMessage::Done(changes)) => {
                        self.active_pane = ConfigPane::Menu;
                        if changes.is_empty() {
                            Some(vec![])
                        } else {
                            Some(Self::process_config_changes(changes))
                        }
                    }
                    None => Some(vec![]),
                }
            }
            ConfigPane::Picker(picker) => {
                let outcome = picker.on_event(event).await;
                match outcome.unwrap_or_default().into_iter().next() {
                    Some(ConfigPickerMessage::Close) => {
                        self.active_pane = ConfigPane::Menu;
                        Some(vec![])
                    }
                    Some(ConfigPickerMessage::ApplySelection(change)) => {
                        if let Some(change) = change {
                            self.menu.apply_change(&change);
                            self.active_pane = ConfigPane::Menu;
                            Some(Self::process_config_changes(vec![change]))
                        } else {
                            self.active_pane = ConfigPane::Menu;
                            Some(vec![])
                        }
                    }
                    None => Some(vec![]),
                }
            }
            ConfigPane::Menu => {
                let outcome = self.menu.on_event(event).await;
                let messages = outcome.unwrap_or_default();
                match messages.as_slice() {
                    [ConfigMenuMessage::CloseAll] => Some(vec![ConfigOverlayMessage::Close]),
                    [ConfigMenuMessage::OpenSelectedPicker] => {
                        if let Some(picker) = self
                            .menu
                            .selected_entry()
                            .and_then(ConfigPicker::from_entry)
                        {
                            self.active_pane = ConfigPane::Picker(picker);
                        }
                        Some(vec![])
                    }
                    [ConfigMenuMessage::OpenModelSelector] => {
                        if let Some(entry) = self.menu.selected_entry() {
                            let current =
                                Some(entry.current_raw_value.as_str()).filter(|v| !v.is_empty());
                            self.active_pane =
                                ConfigPane::ModelSelector(ModelSelector::from_model_entry(
                                    entry,
                                    current,
                                    self.current_reasoning_effort.as_deref(),
                                ));
                        }
                        Some(vec![])
                    }
                    [ConfigMenuMessage::OpenMcpServers] => {
                        self.active_pane = ConfigPane::ServerStatus(ServerStatusOverlay::new(
                            self.server_statuses.clone(),
                        ));
                        Some(vec![])
                    }
                    [ConfigMenuMessage::OpenProviderLogins] => {
                        let entries = self.build_login_entries();
                        self.active_pane =
                            ConfigPane::ProviderLogin(ProviderLoginOverlay::new(entries));
                        Some(vec![])
                    }
                    _ => Some(vec![]),
                }
            }
        }
    }

    fn render(&self, context: &ViewContext) -> Frame {
        let height = (context.size.height.saturating_sub(1)) as usize;
        let width = context.size.width as usize;
        if height < MIN_HEIGHT || width < MIN_WIDTH {
            return Frame::new(vec![Line::new("(terminal too small)")]);
        }

        let footer = self.footer_text();
        #[allow(clippy::cast_possible_truncation)]
        let child_max_height = height.saturating_sub(4) as u16;
        let inner_w = Panel::inner_width(context.size.width);
        let child_context = context.with_size((inner_w, child_max_height));

        let child_lines = match &self.active_pane {
            ConfigPane::ServerStatus(overlay) => overlay.render(&child_context).into_lines(),
            ConfigPane::ProviderLogin(overlay) => overlay.render(&child_context).into_lines(),
            ConfigPane::ModelSelector(selector) => selector.render(&child_context).into_lines(),
            ConfigPane::Picker(picker) => picker.render(&child_context).into_lines(),
            ConfigPane::Menu => self.menu.render(&child_context).into_lines(),
        };

        let mut container = Panel::new(context.theme.muted())
            .title(" Configuration ")
            .footer(footer)
            .fill_height(height)
            .gap(GAP);
        container.push(child_lines);
        Frame::new(container.render(context))
    }
}

fn theme_file_from_picker_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{KeyCode, KeyEvent, KeyModifiers};
    use acp_utils::config_option_id::THEME_CONFIG_ID;
    use acp_utils::notifications::McpServerStatus;
    use agent_client_protocol::SessionConfigSelectOption;

    fn make_menu() -> ConfigMenu {
        let options = vec![
            agent_client_protocol::SessionConfigOption::select(
                "provider",
                "Provider",
                "openrouter",
                vec![
                    SessionConfigSelectOption::new("openrouter", "OpenRouter"),
                    SessionConfigSelectOption::new("ollama", "Ollama"),
                ],
            ),
            agent_client_protocol::SessionConfigOption::select(
                "model",
                "Model",
                "gpt-4o",
                vec![
                    SessionConfigSelectOption::new("gpt-4o", "GPT-4o"),
                    SessionConfigSelectOption::new("claude", "Claude"),
                ],
            ),
        ];
        ConfigMenu::from_config_options(&options)
    }

    fn make_multi_select_menu() -> ConfigMenu {
        let mut meta = serde_json::Map::new();
        meta.insert("multi_select".to_string(), serde_json::Value::Bool(true));
        let options = vec![
            agent_client_protocol::SessionConfigOption::select(
                "provider",
                "Provider",
                "openrouter",
                vec![
                    SessionConfigSelectOption::new("openrouter", "OpenRouter"),
                    SessionConfigSelectOption::new("ollama", "Ollama"),
                ],
            ),
            agent_client_protocol::SessionConfigOption::select(
                "model",
                "Model",
                "gpt-4o",
                vec![
                    SessionConfigSelectOption::new("gpt-4o", "GPT-4o"),
                    SessionConfigSelectOption::new("claude", "Claude"),
                ],
            )
            .meta(meta),
        ];
        ConfigMenu::from_config_options(&options)
    }

    fn make_server_statuses() -> Vec<McpServerStatusEntry> {
        vec![
            McpServerStatusEntry {
                name: "github".to_string(),
                status: McpServerStatus::Connected { tool_count: 5 },
            },
            McpServerStatusEntry {
                name: "linear".to_string(),
                status: McpServerStatus::NeedsOAuth,
            },
        ]
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// Render the overlay and return the footer line text.
    fn render_footer(overlay: &mut ConfigOverlay) -> String {
        let context = ViewContext::new((80, 24));
        let height = (context.size.height.saturating_sub(1)) as usize;
        overlay.update_child_viewport(height.saturating_sub(4));
        let frame = overlay.render(&context);
        let lines = frame.lines();
        lines[lines.len() - 2].plain_text()
    }

    fn render_plain_text(overlay: &mut ConfigOverlay) -> Vec<String> {
        let context = ViewContext::new((80, 24));
        let height = (context.size.height.saturating_sub(1)) as usize;
        overlay.update_child_viewport(height.saturating_sub(4));
        overlay
            .render(&context)
            .into_lines()
            .into_iter()
            .map(|line| line.plain_text())
            .collect()
    }

    fn make_auth_methods() -> Vec<acp::AuthMethod> {
        vec![
            acp::AuthMethod::Agent(acp::AuthMethodAgent::new("anthropic", "Anthropic")),
            acp::AuthMethod::Agent(acp::AuthMethodAgent::new("openrouter", "OpenRouter")),
        ]
    }

    #[tokio::test]
    async fn esc_closes_overlay() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        let outcome = overlay.on_event(&Event::Key(key(KeyCode::Esc))).await;
        let messages = outcome.unwrap();
        assert!(matches!(messages.as_slice(), [ConfigOverlayMessage::Close]));
    }

    #[tokio::test]
    async fn enter_opens_picker() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        let outcome = overlay.on_event(&Event::Key(key(KeyCode::Enter))).await;
        assert!(outcome.is_some());
        assert!(overlay.has_picker());
    }

    #[tokio::test]
    async fn picker_esc_closes_picker_not_overlay() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        overlay.on_event(&Event::Key(key(KeyCode::Enter))).await; // open picker
        assert!(overlay.has_picker());

        let outcome = overlay.on_event(&Event::Key(key(KeyCode::Esc))).await;
        assert!(outcome.is_some());
        assert!(!overlay.has_picker());
        // No messages — overlay remains open
        assert!(outcome.unwrap().is_empty());
    }

    #[tokio::test]
    async fn picker_confirm_returns_config_change_action() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        overlay.on_event(&Event::Key(key(KeyCode::Enter))).await; // open picker
        overlay.on_event(&Event::Key(key(KeyCode::Down))).await; // move to second option
        let outcome = overlay.on_event(&Event::Key(key(KeyCode::Enter))).await; // confirm

        let messages = outcome.unwrap();
        match messages.as_slice() {
            [ConfigOverlayMessage::SetConfigOption { config_id, value }] => {
                assert_eq!(config_id, "provider");
                assert_eq!(value, "ollama");
            }
            other => panic!("expected SetConfigOption, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn config_overlay_picker_confirm_updates_menu_row_immediately() {
        let mut menu = ConfigMenu::from_config_options(&[]);
        menu.add_theme_entry(None, &["nord.tmTheme".to_string()]);
        let mut overlay = ConfigOverlay::new(menu, vec![], vec![]);

        overlay.on_event(&Event::Key(key(KeyCode::Enter))).await; // open picker on Theme
        let _ = overlay.on_event(&Event::Key(key(KeyCode::Down))).await; // select nord.tmTheme
        let _ = overlay.on_event(&Event::Key(key(KeyCode::Enter))).await; // confirm

        assert_eq!(overlay.menu.options()[0].config_id, THEME_CONFIG_ID);
        assert_eq!(overlay.menu.options()[0].current_raw_value, "nord.tmTheme");
        assert_eq!(overlay.menu.options()[0].current_value_index, 1);
    }

    #[test]
    fn cursor_col_without_picker_is_zero() {
        let overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        assert_eq!(overlay.cursor_col(), 0);
    }

    #[tokio::test]
    async fn cursor_col_with_picker_includes_border_and_prefix() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        overlay.on_event(&Event::Key(key(KeyCode::Enter))).await; // open picker for Provider
        let col = overlay.cursor_col();
        // "│ " (2) + "  Provider search: " (19) + query (0) = should be > 0
        assert!(col > 0);
    }

    #[tokio::test]
    async fn picker_cursor_row_offset_matches_submenu_only_layout() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        overlay.on_event(&Event::Key(key(KeyCode::Enter))).await;

        assert_eq!(overlay.cursor_row_offset(), TOP_CHROME);
    }

    #[tokio::test]
    async fn model_selector_esc_without_toggle_returns_no_change() {
        let mut overlay = ConfigOverlay::new(make_multi_select_menu(), vec![], vec![]);

        // Navigate to model and open model selector
        overlay.on_event(&Event::Key(key(KeyCode::Down))).await;
        overlay.on_event(&Event::Key(key(KeyCode::Enter))).await;
        assert!(render_footer(&mut overlay).contains("Toggle"));

        // Selector pre-selects current model (gpt-4o); Esc without toggling returns no change
        let outcome = overlay.on_event(&Event::Key(key(KeyCode::Esc))).await;
        assert!(outcome.is_some());
        assert!(render_footer(&mut overlay).contains("[Enter] Select"));
        assert!(
            outcome.unwrap().is_empty(),
            "escape without toggling should produce no change"
        );
    }

    #[tokio::test]
    async fn model_selector_esc_after_deselecting_all_returns_no_change() {
        let mut overlay = ConfigOverlay::new(make_multi_select_menu(), vec![], vec![]);

        overlay.on_event(&Event::Key(key(KeyCode::Down))).await;
        overlay.on_event(&Event::Key(key(KeyCode::Enter))).await; // open model selector
        // Deselect the pre-selected model
        overlay.on_event(&Event::Key(key(KeyCode::Char(' ')))).await;

        let outcome = overlay.on_event(&Event::Key(key(KeyCode::Esc))).await;
        assert!(render_footer(&mut overlay).contains("[Enter] Select"));
        assert!(outcome.unwrap().is_empty()); // No selections => no change
    }

    #[tokio::test]
    async fn model_selector_enter_toggles_not_confirms() {
        let mut overlay = ConfigOverlay::new(make_multi_select_menu(), vec![], vec![]);

        overlay.on_event(&Event::Key(key(KeyCode::Down))).await;
        overlay.on_event(&Event::Key(key(KeyCode::Enter))).await; // open model selector
        assert!(render_footer(&mut overlay).contains("Toggle"));

        // Enter should toggle, not close the selector
        overlay.on_event(&Event::Key(key(KeyCode::Enter))).await;
        let footer = render_footer(&mut overlay);
        assert!(
            footer.contains("Toggle"),
            "Enter should toggle, not close; got: {footer}"
        );
    }

    #[tokio::test]
    async fn model_selector_uses_overlay_reasoning_prefill_after_menu_removal() {
        use crate::components::config_menu::{
            ConfigMenuEntry, ConfigMenuEntryKind, ConfigMenuValue,
        };
        use acp_utils::config_meta::SelectOptionMeta;

        let menu = ConfigMenu::from_entries(vec![ConfigMenuEntry {
            config_id: "model".to_string(),
            title: "Model".to_string(),
            values: vec![
                ConfigMenuValue {
                    value: "claude-opus".to_string(),
                    name: "Claude Opus".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta {
                        supports_reasoning: true,
                    },
                },
                ConfigMenuValue {
                    value: "gpt-4o".to_string(),
                    name: "GPT-4o".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta::default(),
                },
            ],
            current_value_index: 0,
            current_raw_value: "claude-opus".to_string(),
            entry_kind: ConfigMenuEntryKind::Select,
            multi_select: true,
            display_name: None,
        }]);

        let mut overlay = ConfigOverlay::new(menu, vec![], vec![]);
        let options_with_reasoning = vec![
            agent_client_protocol::SessionConfigOption::select(
                "model",
                "Model",
                "claude-opus",
                vec![
                    SessionConfigSelectOption::new("claude-opus", "Claude Opus"),
                    SessionConfigSelectOption::new("gpt-4o", "GPT-4o"),
                ],
            ),
            agent_client_protocol::SessionConfigOption::select(
                "reasoning_effort",
                "Reasoning Effort",
                "medium",
                vec![
                    SessionConfigSelectOption::new("none", "None"),
                    SessionConfigSelectOption::new("low", "Low"),
                    SessionConfigSelectOption::new("medium", "Medium"),
                    SessionConfigSelectOption::new("high", "High"),
                ],
            ),
        ];
        overlay = overlay.with_reasoning_effort_from_options(&options_with_reasoning);

        overlay.on_event(&Event::Key(key(KeyCode::Enter))).await;
        assert!(
            render_footer(&mut overlay).contains("Toggle"),
            "model selector should be open"
        );

        overlay.on_event(&Event::Key(key(KeyCode::Right))).await;

        let outcome = overlay.on_event(&Event::Key(key(KeyCode::Esc))).await;

        let messages = outcome.unwrap();
        let reasoning_msg = messages.iter().find(|m| {
            matches!(m, ConfigOverlayMessage::SetConfigOption { config_id, .. } if config_id == "reasoning_effort")
        });
        assert!(
            reasoning_msg.is_some(),
            "should have reasoning_effort SetConfigOption; got: {messages:?}"
        );
        match reasoning_msg.unwrap() {
            ConfigOverlayMessage::SetConfigOption { value, .. } => {
                assert_eq!(
                    value, "high",
                    "reasoning should be high after one right from medium"
                );
            }
            other => panic!("expected SetConfigOption, got: {other:?}"),
        }
    }

    #[test]
    fn update_config_options_preserves_mcp_servers_entry() {
        use crate::components::config_menu::ConfigMenuEntryKind;
        use crate::test_helpers::with_wisp_home;

        let temp_dir = tempfile::TempDir::new().unwrap();
        let themes_dir = temp_dir.path().join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();
        std::fs::write(themes_dir.join("custom.tmTheme"), "x").unwrap();

        with_wisp_home(temp_dir.path(), || {
            let mut menu = make_menu();
            menu.add_mcp_servers_entry("1 connected, 1 needs auth");
            let statuses = make_server_statuses();
            let mut overlay = ConfigOverlay::new(menu, statuses, vec![]);

            // Verify MCP servers entry exists initially
            assert!(
                overlay
                    .menu
                    .options()
                    .iter()
                    .any(|e| e.entry_kind == ConfigMenuEntryKind::McpServers),
                "MCP servers entry should exist before update"
            );

            // Simulate config update (e.g. after model selection)
            let new_options = vec![
                agent_client_protocol::SessionConfigOption::select(
                    "provider",
                    "Provider",
                    "ollama",
                    vec![
                        SessionConfigSelectOption::new("openrouter", "OpenRouter"),
                        SessionConfigSelectOption::new("ollama", "Ollama"),
                    ],
                ),
                agent_client_protocol::SessionConfigOption::select(
                    "model",
                    "Model",
                    "llama",
                    vec![SessionConfigSelectOption::new("llama", "Llama")],
                ),
            ];
            overlay.update_config_options(&new_options);

            // Theme and MCP entries should still be present after update
            assert!(
                overlay
                    .menu
                    .options()
                    .iter()
                    .any(|e| e.config_id == THEME_CONFIG_ID),
                "Theme entry should survive update_config_options"
            );
            assert!(
                overlay
                    .menu
                    .options()
                    .iter()
                    .any(|e| e.entry_kind == ConfigMenuEntryKind::McpServers),
                "MCP servers entry should survive update_config_options"
            );
        });
    }

    #[tokio::test]
    async fn provider_login_overlay_closes_when_empty() {
        let mut menu = make_menu();
        menu.add_provider_logins_entry("2 needs login");
        let mut overlay = ConfigOverlay::new(menu, vec![], make_auth_methods());
        overlay.on_event(&Event::Key(key(KeyCode::Down))).await;
        overlay.on_event(&Event::Key(key(KeyCode::Down))).await;
        overlay.on_event(&Event::Key(key(KeyCode::Enter))).await;
        assert!(matches!(overlay.active_pane, ConfigPane::ProviderLogin(_)));

        overlay.remove_auth_method("anthropic");
        overlay.remove_auth_method("openrouter");

        assert!(matches!(overlay.active_pane, ConfigPane::Menu));

        let lines = render_plain_text(&mut overlay);
        let text = lines.join("\n");

        assert!(text.contains("Provider: OpenRouter"), "rendered:\n{text}");
    }

    #[test]
    fn theme_default_value_maps_to_none() {
        assert_eq!(theme_file_from_picker_value("   "), None);
    }

    #[test]
    fn theme_value_maps_to_some() {
        assert_eq!(
            theme_file_from_picker_value("catppuccin.tmTheme"),
            Some("catppuccin.tmTheme".to_string())
        );
    }

    #[test]
    fn process_theme_change_persists_and_produces_set_theme() {
        use crate::test_helpers::{CUSTOM_TMTHEME, with_wisp_home};
        use crate::tui::Color;
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let themes_dir = temp_dir.path().join("themes");
        fs::create_dir_all(&themes_dir).unwrap();
        fs::write(themes_dir.join("custom.tmTheme"), CUSTOM_TMTHEME).unwrap();

        with_wisp_home(temp_dir.path(), || {
            let _overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
            let messages = ConfigOverlay::process_config_changes(vec![ConfigChange {
                config_id: THEME_CONFIG_ID.to_string(),
                new_value: "custom.tmTheme".to_string(),
            }]);

            let theme_msg = messages.iter().find_map(|m| {
                if let ConfigOverlayMessage::SetTheme(theme) = m {
                    Some(theme)
                } else {
                    None
                }
            });
            assert!(theme_msg.is_some(), "should produce SetTheme message");
            assert_eq!(
                theme_msg.unwrap().text_primary(),
                Color::Rgb {
                    r: 0x11,
                    g: 0x22,
                    b: 0x33
                }
            );

            let loaded = crate::settings::load_or_create_settings();
            assert_eq!(loaded.theme.file.as_deref(), Some("custom.tmTheme"));
        });
    }

    #[test]
    fn process_theme_change_persists_default_as_none() {
        use crate::settings::{ThemeSettings as WispThemeSettings, WispSettings};
        use crate::test_helpers::with_wisp_home;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        with_wisp_home(temp_dir.path(), || {
            save_settings(&WispSettings {
                theme: WispThemeSettings {
                    file: Some("old.tmTheme".to_string()),
                },
            })
            .unwrap();

            let _overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
            let _messages = ConfigOverlay::process_config_changes(vec![ConfigChange {
                config_id: THEME_CONFIG_ID.to_string(),
                new_value: "   ".to_string(),
            }]);

            let loaded = crate::settings::load_or_create_settings();
            assert_eq!(loaded.theme.file, None);
        });
    }

    #[test]
    fn process_non_theme_change_produces_set_config_option() {
        let _overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        let messages = ConfigOverlay::process_config_changes(vec![ConfigChange {
            config_id: "provider".to_string(),
            new_value: "ollama".to_string(),
        }]);

        match messages.as_slice() {
            [ConfigOverlayMessage::SetConfigOption { config_id, value }] => {
                assert_eq!(config_id, "provider");
                assert_eq!(value, "ollama");
            }
            other => panic!("expected SetConfigOption, got: {other:?}"),
        }
    }
}
