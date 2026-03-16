use crate::components::model_selector::{ModelSelector, ModelSelectorMessage};
use crate::components::provider_login::{
    ProviderLoginMessage, ProviderLoginOverlay,
};
use crate::components::server_status::{
    ServerStatusMessage, ServerStatusOverlay,
};
use super::menu::{SettingMenuMessage, SettingsMenu};
use super::picker::{SettingsPicker, SettingsPickerMessage};
use crate::tui::Panel;
use crate::tui::{Component, Cursor, Event, Frame, Layout, Line, ViewContext};
use acp_utils::config_option_id::ConfigOptionId;
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

enum SettingsPane {
    Menu,
    Picker(SettingsPicker),
    ModelSelector(ModelSelector),
    ServerStatus(ServerStatusOverlay),
    ProviderLogin(ProviderLoginOverlay),
}

pub struct SettingsOverlay {
    menu: SettingsMenu,
    active_pane: SettingsPane,
    server_statuses: Vec<McpServerStatusEntry>,
    auth_methods: Vec<acp::AuthMethod>,
    current_reasoning_effort: Option<String>,
}

#[derive(Debug)]
pub enum SettingsMessage {
    Close,
    SetConfigOption { config_id: String, value: String },
    SetTheme(crate::tui::Theme),
    AuthenticateServer(String),
    AuthenticateProvider(String),
}

impl SettingsOverlay {
    pub fn new(
        menu: SettingsMenu,
        server_statuses: Vec<McpServerStatusEntry>,
        auth_methods: Vec<acp::AuthMethod>,
    ) -> Self {
        Self {
            menu,
            active_pane: SettingsPane::Menu,
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

    pub fn build_frame(&mut self, ctx: &ViewContext) -> Frame {
        let cursor = if self.has_picker() {
            Cursor::visible(self.cursor_row_offset(), self.cursor_col())
        } else {
            Cursor::hidden()
        };
        let mut layout = Layout::new();
        layout.section(self.render(ctx).into_lines());
        layout.into_frame().with_cursor(cursor)
    }

    pub fn update_child_viewport(&mut self, max_height: usize) {
        match &mut self.active_pane {
            SettingsPane::ModelSelector(ms) => ms.update_viewport(max_height),
            SettingsPane::Picker(p) => p.update_viewport(max_height),
            _ => {}
        }
    }

    pub fn update_config_options(&mut self, options: &[SessionConfigOption]) {
        self.current_reasoning_effort = Self::extract_reasoning_effort(options);
        self.menu.update_options(options);
        super::decorate_menu(&mut self.menu, &self.server_statuses, &self.auth_methods);
    }

    pub fn update_server_statuses(&mut self, statuses: Vec<McpServerStatusEntry>) {
        self.server_statuses = statuses;
        if let SettingsPane::ServerStatus(ref mut overlay) = self.active_pane {
            overlay.update_entries(self.server_statuses.clone());
        }
    }

    pub fn on_authenticate_started(&mut self, method_id: &str) {
        if let SettingsPane::ProviderLogin(ref mut overlay) = self.active_pane {
            overlay.set_authenticating(method_id);
        }
    }

    pub fn on_authenticate_complete(&mut self, method_id: &str) {
        if let SettingsPane::ProviderLogin(ref mut overlay) = self.active_pane {
            overlay.set_logged_in(method_id);
        }
    }

    pub fn on_authenticate_failed(&mut self, method_id: &str) {
        if let SettingsPane::ProviderLogin(ref mut overlay) = self.active_pane {
            overlay.reset_to_needs_login(method_id);
        }
    }

    pub fn cursor_col(&self) -> usize {
        match &self.active_pane {
            SettingsPane::Picker(picker) => {
                let prefix = format!("  {} search: ", picker.title);
                BORDER_LEFT_WIDTH
                    + UnicodeWidthStr::width(prefix.as_str())
                    + UnicodeWidthStr::width(picker.query())
            }
            SettingsPane::ModelSelector(selector) => {
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
            SettingsPane::Picker(_) | SettingsPane::ModelSelector(_) => TOP_CHROME,
            _ => 0,
        }
    }

    pub fn has_picker(&self) -> bool {
        matches!(self.active_pane, SettingsPane::Picker(_))
    }

    fn footer_text(&self) -> &'static str {
        match &self.active_pane {
            SettingsPane::ModelSelector(_) => {
                "[Space/Enter] Toggle  [\u{2190}/\u{2192}] Reasoning  [Esc] Done"
            }
            SettingsPane::Picker(_) => "[Enter] Confirm  [Esc] Back",
            SettingsPane::ServerStatus(_) | SettingsPane::ProviderLogin(_) => {
                "[Enter] Authenticate  [Esc] Back"
            }
            SettingsPane::Menu => "[Enter] Select  [Esc] Close",
        }
    }
}

impl Component for SettingsOverlay {
    type Message = SettingsMessage;

    #[allow(clippy::too_many_lines)]
    async fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        let Event::Key(_key) = event else {
            return None;
        };

        match &mut self.active_pane {
            SettingsPane::ServerStatus(overlay) => {
                let outcome = overlay.on_event(event).await;
                match outcome.unwrap_or_default().into_iter().next() {
                    Some(ServerStatusMessage::Close) => {
                        self.active_pane = SettingsPane::Menu;
                        Some(vec![])
                    }
                    Some(ServerStatusMessage::Authenticate(name)) => {
                        Some(vec![SettingsMessage::AuthenticateServer(name)])
                    }
                    None => Some(vec![]),
                }
            }
            SettingsPane::ProviderLogin(overlay) => {
                let outcome = overlay.on_event(event).await;
                match outcome.unwrap_or_default().into_iter().next() {
                    Some(ProviderLoginMessage::Close) => {
                        self.active_pane = SettingsPane::Menu;
                        Some(vec![])
                    }
                    Some(ProviderLoginMessage::Authenticate(method_id)) => {
                        Some(vec![SettingsMessage::AuthenticateProvider(
                            method_id,
                        )])
                    }
                    None => Some(vec![]),
                }
            }
            SettingsPane::ModelSelector(selector) => {
                let outcome = selector.on_event(event).await;
                match outcome.unwrap_or_default().into_iter().next() {
                    Some(ModelSelectorMessage::Done(changes)) => {
                        self.active_pane = SettingsPane::Menu;
                        if changes.is_empty() {
                            Some(vec![])
                        } else {
                            Some(super::process_config_changes(changes))
                        }
                    }
                    None => Some(vec![]),
                }
            }
            SettingsPane::Picker(picker) => {
                let outcome = picker.on_event(event).await;
                match outcome.unwrap_or_default().into_iter().next() {
                    Some(SettingsPickerMessage::Close) => {
                        self.active_pane = SettingsPane::Menu;
                        Some(vec![])
                    }
                    Some(SettingsPickerMessage::ApplySelection(change)) => {
                        if let Some(change) = change {
                            self.menu.apply_change(&change);
                            self.active_pane = SettingsPane::Menu;
                            Some(super::process_config_changes(vec![change]))
                        } else {
                            self.active_pane = SettingsPane::Menu;
                            Some(vec![])
                        }
                    }
                    None => Some(vec![]),
                }
            }
            SettingsPane::Menu => {
                let outcome = self.menu.on_event(event).await;
                let messages = outcome.unwrap_or_default();
                match messages.as_slice() {
                    [SettingMenuMessage::CloseAll] => Some(vec![SettingsMessage::Close]),
                    [SettingMenuMessage::OpenSelectedPicker] => {
                        if let Some(picker) = self
                            .menu
                            .selected_entry()
                            .and_then(SettingsPicker::from_entry)
                        {
                            self.active_pane = SettingsPane::Picker(picker);
                        }
                        Some(vec![])
                    }
                    [SettingMenuMessage::OpenModelSelector] => {
                        if let Some(entry) = self.menu.selected_entry() {
                            let current =
                                Some(entry.current_raw_value.as_str()).filter(|v| !v.is_empty());
                            self.active_pane =
                                SettingsPane::ModelSelector(ModelSelector::from_model_entry(
                                    entry,
                                    current,
                                    self.current_reasoning_effort.as_deref(),
                                ));
                        }
                        Some(vec![])
                    }
                    [SettingMenuMessage::OpenMcpServers] => {
                        self.active_pane = SettingsPane::ServerStatus(ServerStatusOverlay::new(
                            self.server_statuses.clone(),
                        ));
                        Some(vec![])
                    }
                    [SettingMenuMessage::OpenProviderLogins] => {
                        let entries = super::build_login_entries(&self.auth_methods);
                        self.active_pane =
                            SettingsPane::ProviderLogin(ProviderLoginOverlay::new(entries));
                        Some(vec![])
                    }
                    _ => Some(vec![]),
                }
            }
        }
    }

    fn render(&mut self, context: &ViewContext) -> Frame {
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

        let child_lines = match &mut self.active_pane {
            SettingsPane::ServerStatus(overlay) => overlay.render(&child_context).into_lines(),
            SettingsPane::ProviderLogin(overlay) => overlay.render(&child_context).into_lines(),
            SettingsPane::ModelSelector(selector) => selector.render(&child_context).into_lines(),
            SettingsPane::Picker(picker) => picker.render(&child_context).into_lines(),
            SettingsPane::Menu => self.menu.render(&child_context).into_lines(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::provider_login::ProviderLoginStatus;
    use crate::tui::{KeyCode, KeyEvent, KeyModifiers};
    use acp_utils::config_option_id::THEME_CONFIG_ID;
    use acp_utils::notifications::McpServerStatus;
    use agent_client_protocol::SessionConfigSelectOption;

    fn make_menu() -> SettingsMenu {
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
        SettingsMenu::from_config_options(&options)
    }

    fn make_multi_select_menu() -> SettingsMenu {
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
        SettingsMenu::from_config_options(&options)
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
    fn render_footer(overlay: &mut SettingsOverlay) -> String {
        let context = ViewContext::new((80, 24));
        let height = (context.size.height.saturating_sub(1)) as usize;
        overlay.update_child_viewport(height.saturating_sub(4));
        let frame = overlay.render(&context);
        let lines = frame.lines();
        lines[lines.len() - 2].plain_text()
    }

    fn make_auth_methods() -> Vec<acp::AuthMethod> {
        vec![
            acp::AuthMethod::Agent(acp::AuthMethodAgent::new("anthropic", "Anthropic")),
            acp::AuthMethod::Agent(acp::AuthMethodAgent::new("openrouter", "OpenRouter")),
        ]
    }

    #[tokio::test]
    async fn esc_closes_overlay() {
        let mut overlay = SettingsOverlay::new(make_menu(), vec![], vec![]);
        let outcome = overlay.on_event(&Event::Key(key(KeyCode::Esc))).await;
        let messages = outcome.unwrap();
        assert!(matches!(
            messages.as_slice(),
            [SettingsMessage::Close]
        ));
    }

    #[tokio::test]
    async fn enter_opens_picker() {
        let mut overlay = SettingsOverlay::new(make_menu(), vec![], vec![]);
        let outcome = overlay.on_event(&Event::Key(key(KeyCode::Enter))).await;
        assert!(outcome.is_some());
        assert!(overlay.has_picker());
    }

    #[tokio::test]
    async fn picker_esc_closes_picker_not_overlay() {
        let mut overlay = SettingsOverlay::new(make_menu(), vec![], vec![]);
        overlay.on_event(&Event::Key(key(KeyCode::Enter))).await; // open picker
        assert!(overlay.has_picker());

        let outcome = overlay.on_event(&Event::Key(key(KeyCode::Esc))).await;
        assert!(outcome.is_some());
        assert!(!overlay.has_picker());
        // No messages — overlay remains open
        assert!(outcome.unwrap().is_empty());
    }

    #[tokio::test]
    async fn picker_confirm_returns_settings_change_action() {
        let mut overlay = SettingsOverlay::new(make_menu(), vec![], vec![]);
        overlay.on_event(&Event::Key(key(KeyCode::Enter))).await; // open picker
        overlay.on_event(&Event::Key(key(KeyCode::Down))).await; // move to second option
        let outcome = overlay.on_event(&Event::Key(key(KeyCode::Enter))).await; // confirm

        let messages = outcome.unwrap();
        match messages.as_slice() {
            [SettingsMessage::SetConfigOption { config_id, value }] => {
                assert_eq!(config_id, "provider");
                assert_eq!(value, "ollama");
            }
            other => panic!("expected SetConfigOption, got: {other:?}"),
        }
    }

    #[test]
    fn settings_overlay_picker_confirm_updates_menu_row_immediately() {
        use crate::test_helpers::with_wisp_home;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        with_wisp_home(temp_dir.path(), || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let mut menu = SettingsMenu::from_config_options(&[]);
                menu.add_theme_entry(None, &["nord.tmTheme".to_string()]);
                let mut overlay = SettingsOverlay::new(menu, vec![], vec![]);

                overlay.on_event(&Event::Key(key(KeyCode::Enter))).await;
                let _ = overlay.on_event(&Event::Key(key(KeyCode::Down))).await;
                let _ = overlay.on_event(&Event::Key(key(KeyCode::Enter))).await;

                assert_eq!(overlay.menu.options()[0].config_id, THEME_CONFIG_ID);
                assert_eq!(overlay.menu.options()[0].current_raw_value, "nord.tmTheme");
                assert_eq!(overlay.menu.options()[0].current_value_index, 1);
            });
        });
    }

    #[test]
    fn settings_overlay_picker_confirm_persists_theme_to_settings() {
        use crate::settings::load_or_create_settings;
        use crate::test_helpers::with_wisp_home;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        with_wisp_home(temp_dir.path(), || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let mut menu = SettingsMenu::from_config_options(&[]);
                menu.add_theme_entry(None, &["nord.tmTheme".to_string()]);
                let mut overlay = SettingsOverlay::new(menu, vec![], vec![]);

                overlay.on_event(&Event::Key(key(KeyCode::Enter))).await;
                let _ = overlay.on_event(&Event::Key(key(KeyCode::Down))).await;
                let _ = overlay.on_event(&Event::Key(key(KeyCode::Enter))).await;

                let settings = load_or_create_settings();
                assert_eq!(
                    settings.theme.file.as_deref(),
                    Some("nord.tmTheme"),
                    "theme selection should be persisted to settings.json"
                );
            });
        });
    }

    #[test]
    fn cursor_col_without_picker_is_zero() {
        let overlay = SettingsOverlay::new(make_menu(), vec![], vec![]);
        assert_eq!(overlay.cursor_col(), 0);
    }

    #[tokio::test]
    async fn cursor_col_with_picker_includes_border_and_prefix() {
        let mut overlay = SettingsOverlay::new(make_menu(), vec![], vec![]);
        overlay.on_event(&Event::Key(key(KeyCode::Enter))).await; // open picker for Provider
        let col = overlay.cursor_col();
        // "│ " (2) + "  Provider search: " (19) + query (0) = should be > 0
        assert!(col > 0);
    }

    #[tokio::test]
    async fn picker_cursor_row_offset_matches_submenu_only_layout() {
        let mut overlay = SettingsOverlay::new(make_menu(), vec![], vec![]);
        overlay.on_event(&Event::Key(key(KeyCode::Enter))).await;

        assert_eq!(overlay.cursor_row_offset(), TOP_CHROME);
    }

    #[tokio::test]
    async fn model_selector_esc_without_toggle_returns_no_change() {
        let mut overlay = SettingsOverlay::new(make_multi_select_menu(), vec![], vec![]);

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
        let mut overlay = SettingsOverlay::new(make_multi_select_menu(), vec![], vec![]);

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
        let mut overlay = SettingsOverlay::new(make_multi_select_menu(), vec![], vec![]);

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
        use crate::settings::types::{
            SettingsMenuEntry, SettingsMenuEntryKind, SettingsMenuValue,
        };
        use acp_utils::config_meta::SelectOptionMeta;

        let menu = SettingsMenu::from_entries(vec![SettingsMenuEntry {
            config_id: "model".to_string(),
            title: "Model".to_string(),
            values: vec![
                SettingsMenuValue {
                    value: "claude-opus".to_string(),
                    name: "Claude Opus".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta {
                        supports_reasoning: true,
                    },
                },
                SettingsMenuValue {
                    value: "gpt-4o".to_string(),
                    name: "GPT-4o".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta::default(),
                },
            ],
            current_value_index: 0,
            current_raw_value: "claude-opus".to_string(),
            entry_kind: SettingsMenuEntryKind::Select,
            multi_select: true,
            display_name: None,
        }]);

        let mut overlay = SettingsOverlay::new(menu, vec![], vec![]);
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
            matches!(m, SettingsMessage::SetConfigOption { config_id, .. } if config_id == "reasoning_effort")
        });
        assert!(
            reasoning_msg.is_some(),
            "should have reasoning_effort SetConfigOption; got: {messages:?}"
        );
        match reasoning_msg.unwrap() {
            SettingsMessage::SetConfigOption { value, .. } => {
                assert_eq!(
                    value, "high",
                    "reasoning should be high after one right from medium"
                );
            }
            other => panic!("expected SetConfigOption, got: {other:?}"),
        }
    }

    #[test]
    fn update_settings_options_preserves_mcp_servers_entry() {
        use crate::settings::types::SettingsMenuEntryKind;
        use crate::test_helpers::with_wisp_home;

        let temp_dir = tempfile::TempDir::new().unwrap();
        let themes_dir = temp_dir.path().join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();
        std::fs::write(themes_dir.join("custom.tmTheme"), "x").unwrap();

        with_wisp_home(temp_dir.path(), || {
            let mut menu = make_menu();
            menu.add_mcp_servers_entry("1 connected, 1 needs auth");
            let statuses = make_server_statuses();
            let mut overlay = SettingsOverlay::new(menu, statuses, vec![]);

            // Verify MCP servers entry exists initially
            assert!(
                overlay
                    .menu
                    .options()
                    .iter()
                    .any(|e| e.entry_kind == SettingsMenuEntryKind::McpServers),
                "MCP servers entry should exist before update"
            );

            // Simulate settings update (e.g. after model selection)
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
                    .any(|e| e.entry_kind == SettingsMenuEntryKind::McpServers),
                "MCP servers entry should survive update_config_options"
            );
        });
    }

    #[tokio::test]
    async fn authenticate_complete_sets_logged_in_status() {
        let mut menu = make_menu();
        menu.add_provider_logins_entry("2 needs login");
        let mut overlay = SettingsOverlay::new(menu, vec![], make_auth_methods());
        overlay.on_event(&Event::Key(key(KeyCode::Down))).await;
        overlay.on_event(&Event::Key(key(KeyCode::Down))).await;
        overlay.on_event(&Event::Key(key(KeyCode::Enter))).await;
        assert!(matches!(
            overlay.active_pane,
            SettingsPane::ProviderLogin(_)
        ));

        overlay.on_authenticate_complete("anthropic");

        // Overlay should stay open (entries are not removed)
        assert!(matches!(
            overlay.active_pane,
            SettingsPane::ProviderLogin(_)
        ));

        if let SettingsPane::ProviderLogin(ref overlay_inner) = overlay.active_pane {
            let entry = overlay_inner
                .entries()
                .iter()
                .find(|e| e.method_id == "anthropic")
                .expect("anthropic entry should still exist");
            assert_eq!(entry.status, ProviderLoginStatus::LoggedIn);
        }
    }

}
