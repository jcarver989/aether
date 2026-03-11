use crate::components::config_menu::{ConfigChange, ConfigMenu, ConfigMenuMessage};
use crate::components::config_picker::{ConfigPicker, ConfigPickerMessage};
use crate::tui::Panel;
use crate::components::model_selector::{ModelSelector, ModelSelectorMessage};
use crate::components::provider_login::{
    ProviderLoginEntry, ProviderLoginMessage, ProviderLoginOverlay, ProviderLoginStatus,
    provider_login_summary,
};
use crate::components::server_status::{
    ServerStatusMessage, ServerStatusOverlay, server_status_summary,
};
use crate::settings::{list_theme_files, load_or_create_settings};
use crate::tui::{Line, Outcome, ViewContext, Widget, WidgetEvent};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActiveConfigOverlayView {
    Menu,
    ServerOverlay,
    ProviderLoginOverlay,
    ModelSelector,
    Picker,
}

pub struct ConfigOverlay {
    menu: ConfigMenu,
    picker: Option<ConfigPicker>,
    model_selector: Option<ModelSelector>,
    server_overlay: Option<ServerStatusOverlay>,
    provider_login_overlay: Option<ProviderLoginOverlay>,
    server_statuses: Vec<McpServerStatusEntry>,
    auth_methods: Vec<acp::AuthMethod>,
    current_reasoning_effort: Option<String>,
}

#[derive(Debug)]
pub enum ConfigOverlayMessage {
    Close,
    ApplyConfigChanges(Vec<ConfigChange>),
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
            picker: None,
            model_selector: None,
            server_overlay: None,
            provider_login_overlay: None,
            server_statuses,
            auth_methods,
            current_reasoning_effort: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_server_overlay(mut self) -> Self {
        self.server_overlay = Some(ServerStatusOverlay::new(self.server_statuses.clone()));
        self
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

    pub(crate) fn update_child_viewport(&mut self, max_height: usize) {
        match self.active_view() {
            ActiveConfigOverlayView::ModelSelector => {
                if let Some(ref mut ms) = self.model_selector {
                    ms.update_viewport(max_height);
                }
            }
            ActiveConfigOverlayView::Picker => {
                if let Some(ref mut p) = self.picker {
                    p.update_viewport(max_height);
                }
            }
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
        if let Some(ref mut overlay) = self.server_overlay {
            overlay.update_entries(self.server_statuses.clone());
        }

    }

    pub fn on_authenticate_started(&mut self, method_id: &str) {
        if let Some(ref mut overlay) = self.provider_login_overlay {
            overlay.set_authenticating(method_id);
        }

    }

    pub fn remove_auth_method(&mut self, method_id: &str) {
        self.auth_methods.retain(|m| m.id.0.as_ref() != method_id);
        if let Some(ref mut overlay) = self.provider_login_overlay {
            overlay.remove_entry(method_id);
            if overlay.entries.is_empty() {
                self.provider_login_overlay = None;
            }
        }

    }

    fn build_login_entries(&self) -> Vec<ProviderLoginEntry> {
        self.auth_methods
            .iter()
            .map(|m| ProviderLoginEntry {
                method_id: m.id.0.to_string(),
                name: m.name.clone(),
                status: ProviderLoginStatus::NeedsLogin,
            })
            .collect()
    }

    pub fn on_authenticate_failed(&mut self, method_id: &str) {
        if let Some(entry) = self
            .provider_login_overlay
            .as_mut()
            .and_then(|o| o.entries.iter_mut().find(|e| e.method_id == method_id))
        {
            entry.status = ProviderLoginStatus::NeedsLogin;
        }

    }

    pub fn cursor_col(&self) -> usize {
        if let Some(ref picker) = self.picker {
            let prefix = format!("  {} search: ", picker.title);
            BORDER_LEFT_WIDTH
                + UnicodeWidthStr::width(prefix.as_str())
                + UnicodeWidthStr::width(picker.query())
        } else if let Some(ref selector) = self.model_selector {
            let prefix = "  Model search: ";
            BORDER_LEFT_WIDTH
                + UnicodeWidthStr::width(prefix)
                + UnicodeWidthStr::width(selector.query())
        } else {
            0
        }
    }

    /// Returns the row offset of the cursor within the overlay (0-indexed from top of overlay).
    /// Only meaningful when a search-based submenu is open (picker or model selector).
    pub fn cursor_row_offset(&self) -> usize {
        match self.active_view() {
            ActiveConfigOverlayView::Picker | ActiveConfigOverlayView::ModelSelector => TOP_CHROME,
            _ => 0,
        }
    }

    pub fn has_picker(&self) -> bool {
        self.picker.is_some()
    }

    fn active_view(&self) -> ActiveConfigOverlayView {
        if self.server_overlay.is_some() {
            ActiveConfigOverlayView::ServerOverlay
        } else if self.provider_login_overlay.is_some() {
            ActiveConfigOverlayView::ProviderLoginOverlay
        } else if self.model_selector.is_some() {
            ActiveConfigOverlayView::ModelSelector
        } else if self.picker.is_some() {
            ActiveConfigOverlayView::Picker
        } else {
            ActiveConfigOverlayView::Menu
        }
    }

    fn footer_text(&self) -> &'static str {
        match self.active_view() {
            ActiveConfigOverlayView::ModelSelector => {
                "[Space/Enter] Toggle  [\u{2190}/\u{2192}] Reasoning  [Esc] Done"
            }
            ActiveConfigOverlayView::Picker => "[Enter] Confirm  [Esc] Back",
            ActiveConfigOverlayView::ServerOverlay
            | ActiveConfigOverlayView::ProviderLoginOverlay => "[Enter] Authenticate  [Esc] Back",
            ActiveConfigOverlayView::Menu => "[Enter] Select  [Esc] Close",
        }
    }
}

impl Widget for ConfigOverlay {
    type Message = ConfigOverlayMessage;

    #[allow(clippy::too_many_lines)]
    fn on_event(&mut self, event: &WidgetEvent) -> Outcome<Self::Message> {
        let WidgetEvent::Key(_key) = event else {
            return Outcome::ignored();
        };

        // Server overlay has highest priority
        if let Some(ref mut overlay) = self.server_overlay {
            let outcome = overlay.on_event(event);
            return match outcome.into_messages().into_iter().next() {
                Some(ServerStatusMessage::Close) => {
                    self.server_overlay = None;
                    Outcome::consumed()
                }
                Some(ServerStatusMessage::Authenticate(name)) => {
                    Outcome::message(ConfigOverlayMessage::AuthenticateServer(name))
                }
                None => Outcome::consumed(),
            };
        }

        // Provider login overlay has second priority
        if let Some(ref mut overlay) = self.provider_login_overlay {
            let outcome = overlay.on_event(event);
            return match outcome.into_messages().into_iter().next() {
                Some(ProviderLoginMessage::Close) => {
                    self.provider_login_overlay = None;
                    Outcome::consumed()
                }
                Some(ProviderLoginMessage::Authenticate(method_id)) => {
                    Outcome::message(ConfigOverlayMessage::AuthenticateProvider(method_id))
                }
                None => Outcome::consumed(),
            };
        }

        // Model selector has third priority
        if let Some(ref mut selector) = self.model_selector {
            let outcome = selector.on_event(event);
            return match outcome.into_messages().into_iter().next() {
                Some(ModelSelectorMessage::Done(changes)) => {
                    self.model_selector = None;
                    if changes.is_empty() {
                        Outcome::consumed()
                    } else {
                        Outcome::message(ConfigOverlayMessage::ApplyConfigChanges(changes))
                    }
                }
                None => Outcome::consumed(),
            };
        }

        // Picker has fourth priority
        if let Some(ref mut picker) = self.picker {
            let outcome = picker.on_event(event);
            return match outcome.into_messages().into_iter().next() {
                Some(ConfigPickerMessage::Close) => {
                    self.picker = None;
                    Outcome::consumed()
                }
                Some(ConfigPickerMessage::ApplySelection(change)) => {
                    self.picker = None;
                    match change {
                        Some(change) => {
                            self.menu.apply_change(&change);
                            Outcome::message(ConfigOverlayMessage::ApplyConfigChanges(vec![
                                change,
                            ]))
                        }
                        None => Outcome::consumed(),
                    }
                }
                None => Outcome::consumed(),
            };
        }

        // Menu handles remaining input
        let outcome = self.menu.on_event(event);
        let messages = outcome.into_messages();
        match messages.as_slice() {
            [ConfigMenuMessage::CloseAll] => Outcome::message(ConfigOverlayMessage::Close),
            [ConfigMenuMessage::OpenSelectedPicker] => {
                self.picker = self
                    .menu
                    .selected_entry()
                    .and_then(ConfigPicker::from_entry);
                Outcome::consumed()
            }
            [ConfigMenuMessage::OpenModelSelector] => {
                if let Some(entry) = self.menu.selected_entry() {
                    let current = Some(entry.current_raw_value.as_str()).filter(|v| !v.is_empty());
                    self.model_selector = Some(ModelSelector::from_model_entry(
                        entry,
                        current,
                        self.current_reasoning_effort.as_deref(),
                    ));
                }
                Outcome::consumed()
            }
            [ConfigMenuMessage::OpenMcpServers] => {
                self.server_overlay = Some(ServerStatusOverlay::new(self.server_statuses.clone()));
                Outcome::consumed()
            }
            [ConfigMenuMessage::OpenProviderLogins] => {
                let entries = self.build_login_entries();
                self.provider_login_overlay = Some(ProviderLoginOverlay::new(entries));
                Outcome::consumed()
            }
            _ => Outcome::consumed(),
        }
    }

    fn render(&self, context: &ViewContext) -> Vec<Line> {
        let height = (context.size.height.saturating_sub(1)) as usize;
        let width = context.size.width as usize;
        if height < MIN_HEIGHT || width < MIN_WIDTH {
            return vec![Line::new("(terminal too small)")];
        }

        let footer = self.footer_text();
        let child_max_height = height.saturating_sub(4) as u16;
        let inner_w = Panel::inner_width(context.size.width);
        let child_context = context.with_size((inner_w, child_max_height));

        let child_lines = match self.active_view() {
            ActiveConfigOverlayView::ServerOverlay => self
                .server_overlay
                .as_ref()
                .expect("active server overlay")
                .render(&child_context),
            ActiveConfigOverlayView::ProviderLoginOverlay => self
                .provider_login_overlay
                .as_ref()
                .expect("active provider login overlay")
                .render(&child_context),
            ActiveConfigOverlayView::ModelSelector => self
                .model_selector
                .as_ref()
                .expect("active model selector")
                .render(&child_context),
            ActiveConfigOverlayView::Picker => self
                .picker
                .as_ref()
                .expect("active picker")
                .render(&child_context),
            ActiveConfigOverlayView::Menu => self.menu.render(&child_context),
        };

        let mut container = Panel::new()
            .title(" Configuration ")
            .footer(footer)
            .border_color(context.theme.muted())
            .fill_height(height)
            .gap(GAP);
        container.push(child_lines);
        container.render(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::Line;
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
        let lines = overlay.render(&context);
        lines[lines.len() - 2].plain_text()
    }

    fn render_plain_text(overlay: &mut ConfigOverlay) -> Vec<String> {
        let context = ViewContext::new((80, 24));
        let height = (context.size.height.saturating_sub(1)) as usize;
        overlay.update_child_viewport(height.saturating_sub(4));
        overlay
            .render(&context)
            .into_iter()
            .map(|line| line.plain_text())
            .collect()
    }

    fn make_auth_methods() -> Vec<acp::AuthMethod> {
        vec![
            acp::AuthMethod::new("anthropic", "Anthropic"),
            acp::AuthMethod::new("openrouter", "OpenRouter"),
        ]
    }

    #[test]
    fn bordered_box_fills_terminal_height_minus_one() {
        let overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        let context = ViewContext::new((80, 24));
        let lines = overlay.render(&context);
        // Should fill exactly height - 1 = 23 lines
        assert_eq!(lines.len(), 23);
    }

    #[test]
    fn title_contains_configuration() {
        let overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        let context = ViewContext::new((80, 24));
        let lines = overlay.render(&context);
        assert!(lines[0].plain_text().contains("Configuration"));
    }

    #[test]
    fn footer_shows_select_and_close_for_menu() {
        let overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        let context = ViewContext::new((80, 24));
        let lines = overlay.render(&context);
        let footer = lines[lines.len() - 2].plain_text(); // second to last (last is bottom border)
        assert!(footer.contains("[Enter] Select"), "footer: {footer}");
        assert!(footer.contains("[Esc] Close"), "footer: {footer}");
    }

    #[test]
    fn footer_shows_confirm_and_back_for_picker() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        // Open picker
        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Enter)));
        let context = ViewContext::new((80, 24));
        let lines = overlay.render(&context);
        let footer = lines[lines.len() - 2].plain_text();
        assert!(footer.contains("[Enter] Confirm"), "footer: {footer}");
        assert!(footer.contains("[Esc] Back"), "footer: {footer}");
    }

    #[test]
    fn footer_shows_authenticate_and_back_for_servers() {
        let menu = make_menu();
        let statuses = make_server_statuses();
        let overlay = ConfigOverlay::new(menu, statuses, vec![]).with_server_overlay();
        let context = ViewContext::new((80, 24));
        let lines = overlay.render(&context);
        let footer = lines[lines.len() - 2].plain_text();
        assert!(footer.contains("[Enter] Authenticate"), "footer: {footer}");
        assert!(footer.contains("[Esc] Back"), "footer: {footer}");
    }

    #[test]
    fn selected_entry_has_bg_color() {
        let overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        let context = ViewContext::new((80, 24));
        let lines = overlay.render(&context);
        let selected_line = lines
            .iter()
            .find(|line| line.plain_text().contains("Provider: OpenRouter"))
            .expect("expected provider row to be rendered");
        let has_bg = selected_line
            .spans()
            .iter()
            .any(|s| s.style().bg == Some(context.theme.highlight_bg()));
        assert!(has_bg, "selected entry should have highlight_bg");
    }

    #[test]
    fn render_root_menu_shows_top_level_rows() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);

        let lines = render_plain_text(&mut overlay);
        let text = lines.join("\n");

        assert!(text.contains("Provider: OpenRouter"), "rendered:\n{text}");
        assert!(text.contains("Model: GPT-4o"), "rendered:\n{text}");
        assert!(text.contains("[Enter] Select"), "rendered:\n{text}");
        assert!(text.contains("[Esc] Close"), "rendered:\n{text}");
    }

    #[test]
    fn render_picker_hides_top_level_rows() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Enter)));

        let lines = render_plain_text(&mut overlay);
        let text = lines.join("\n");

        assert!(text.contains("Provider search:"), "rendered:\n{text}");
        assert!(!text.contains("Provider: OpenRouter"), "rendered:\n{text}");
        assert!(!text.contains("Model: GPT-4o"), "rendered:\n{text}");
        assert!(text.contains("[Enter] Confirm"), "rendered:\n{text}");
        assert!(text.contains("[Esc] Back"), "rendered:\n{text}");
    }

    #[test]
    fn render_model_selector_hides_top_level_rows() {
        let mut overlay = ConfigOverlay::new(make_multi_select_menu(), vec![], vec![]);
        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Down)));
        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Enter)));

        let lines = render_plain_text(&mut overlay);
        let text = lines.join("\n");

        assert!(text.contains("Model search:"), "rendered:\n{text}");
        assert!(!text.contains("Provider: OpenRouter"), "rendered:\n{text}");
        assert!(!text.contains("Model: GPT-4o"), "rendered:\n{text}");
        assert!(text.contains("Toggle"), "rendered:\n{text}");
        assert!(text.contains("Reasoning"), "rendered:\n{text}");
        assert!(text.contains("[Esc] Done"), "rendered:\n{text}");
    }

    #[test]
    fn render_server_overlay_hides_top_level_rows() {
        let menu = make_menu();
        let statuses = make_server_statuses();
        let mut overlay = ConfigOverlay::new(menu, statuses, vec![]).with_server_overlay();

        let lines = render_plain_text(&mut overlay);
        let text = lines.join("\n");

        assert!(text.contains("github  ✓ 5 tools"), "rendered:\n{text}");
        assert!(
            text.contains("linear  ⚡ needs authentication"),
            "rendered:\n{text}"
        );
        assert!(!text.contains("Provider: OpenRouter"), "rendered:\n{text}");
        assert!(!text.contains("Model: GPT-4o"), "rendered:\n{text}");
        assert!(text.contains("[Enter] Authenticate"), "rendered:\n{text}");
        assert!(text.contains("[Esc] Back"), "rendered:\n{text}");
    }

    #[test]
    fn render_provider_login_overlay_hides_top_level_rows() {
        let mut menu = make_menu();
        menu.add_provider_logins_entry("2 needs login");
        let mut overlay = ConfigOverlay::new(menu, vec![], make_auth_methods());
        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Down)));
        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Down)));
        let outcome = overlay.on_event(&WidgetEvent::Key(key(KeyCode::Enter)));
        assert!(outcome.is_handled());

        let lines = render_plain_text(&mut overlay);
        let text = lines.join("\n");

        assert!(
            text.contains("Anthropic  ⚡ needs login"),
            "rendered:\n{text}"
        );
        assert!(
            text.contains("OpenRouter  ⚡ needs login"),
            "rendered:\n{text}"
        );
        assert!(!text.contains("Provider: OpenRouter"), "rendered:\n{text}");
        assert!(!text.contains("Model: GPT-4o"), "rendered:\n{text}");
        assert!(text.contains("[Enter] Authenticate"), "rendered:\n{text}");
        assert!(text.contains("[Esc] Back"), "rendered:\n{text}");
    }

    #[test]
    fn picker_cursor_row_offset_matches_submenu_only_layout() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Enter)));

        assert_eq!(overlay.cursor_row_offset(), TOP_CHROME);
    }

    #[test]
    fn esc_closes_overlay() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        let outcome = overlay.on_event(&WidgetEvent::Key(key(KeyCode::Esc)));
        let messages = outcome.into_messages();
        assert!(matches!(
            messages.as_slice(),
            [ConfigOverlayMessage::Close]
        ));
    }

    #[test]
    fn config_overlay_picker_confirm_updates_menu_row_immediately() {
        let mut menu = ConfigMenu::from_config_options(&[]);
        menu.add_theme_entry(None, &["nord.tmTheme".to_string()]);
        let mut overlay = ConfigOverlay::new(menu, vec![], vec![]);

        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Enter))); // open picker on Theme
        let _ = overlay.on_event(&WidgetEvent::Key(key(KeyCode::Down))); // select nord.tmTheme
        let _ = overlay.on_event(&WidgetEvent::Key(key(KeyCode::Enter))); // confirm

        assert_eq!(overlay.menu.options[0].config_id, THEME_CONFIG_ID);
        assert_eq!(overlay.menu.options[0].current_raw_value, "nord.tmTheme");
        assert_eq!(overlay.menu.options[0].current_value_index, 1);
    }

    #[test]
    fn enter_opens_picker() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        let outcome = overlay.on_event(&WidgetEvent::Key(key(KeyCode::Enter)));
        assert!(outcome.is_handled());
        assert!(overlay.has_picker());
    }

    #[test]
    fn picker_esc_closes_picker_not_overlay() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Enter))); // open picker
        assert!(overlay.has_picker());

        let outcome = overlay.on_event(&WidgetEvent::Key(key(KeyCode::Esc)));
        assert!(outcome.is_handled());
        assert!(!overlay.has_picker());
        // No messages — overlay remains open
        assert!(outcome.into_messages().is_empty());
    }

    #[test]
    fn picker_confirm_returns_config_change_action() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Enter))); // open picker
        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Down))); // move to second option
        let outcome = overlay.on_event(&WidgetEvent::Key(key(KeyCode::Enter))); // confirm

        let messages = outcome.into_messages();
        match messages.as_slice() {
            [ConfigOverlayMessage::ApplyConfigChanges(changes)] => {
                assert_eq!(changes.len(), 1);
                assert_eq!(changes[0].config_id, "provider");
                assert_eq!(changes[0].new_value, "ollama");
            }
            other => panic!("expected ApplyConfigChanges, got: {other:?}"),
        }
    }

    #[test]
    fn server_overlay_esc_closes_server_not_config_overlay() {
        let menu = make_menu();
        let statuses = make_server_statuses();
        let mut overlay = ConfigOverlay::new(menu, statuses, vec![]).with_server_overlay();
        assert!(render_footer(&mut overlay).contains("Authenticate"));

        let outcome = overlay.on_event(&WidgetEvent::Key(key(KeyCode::Esc)));
        assert!(outcome.is_handled());
        assert!(render_footer(&mut overlay).contains("[Enter] Select"));
        assert!(outcome.into_messages().is_empty());
    }

    #[test]
    fn cursor_col_without_picker_is_zero() {
        let overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        assert_eq!(overlay.cursor_col(), 0);
    }

    #[test]
    fn cursor_col_with_picker_includes_border_and_prefix() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Enter))); // open picker for Provider
        let col = overlay.cursor_col();
        // "│ " (2) + "  Provider search: " (19) + query (0) = should be > 0
        assert!(col > 0);
    }

    #[test]
    fn narrow_terminal_does_not_panic() {
        let overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        let context = ViewContext::new((4, 3));
        let lines = overlay.render(&context);
        assert!(!lines.is_empty());
    }

    #[test]
    fn very_small_terminal_shows_fallback() {
        let overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        let context = ViewContext::new((3, 2));
        let lines = overlay.render(&context);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain_text().contains("too small"));
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
                    .options
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
                    .options
                    .iter()
                    .any(|e| e.config_id == THEME_CONFIG_ID),
                "Theme entry should survive update_config_options"
            );
            assert!(
                overlay
                    .menu
                    .options
                    .iter()
                    .any(|e| e.entry_kind == ConfigMenuEntryKind::McpServers),
                "MCP servers entry should survive update_config_options"
            );
        });
    }

    #[test]
    fn multi_select_entry_opens_model_selector() {
        let mut overlay = ConfigOverlay::new(make_multi_select_menu(), vec![], vec![]);

        // Navigate to the model entry (index 1: provider=0, model=1)
        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Down)));
        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Enter)));

        let footer = render_footer(&mut overlay);
        assert!(
            footer.contains("Toggle"),
            "expected model selector, got: {footer}"
        );
    }

    #[test]
    fn model_selector_esc_without_toggle_returns_no_change() {
        let mut overlay = ConfigOverlay::new(make_multi_select_menu(), vec![], vec![]);

        // Navigate to model and open model selector
        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Down)));
        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Enter)));
        assert!(render_footer(&mut overlay).contains("Toggle"));

        // Selector pre-selects current model (gpt-4o); Esc without toggling returns no change
        let outcome = overlay.on_event(&WidgetEvent::Key(key(KeyCode::Esc)));
        assert!(outcome.is_handled());
        assert!(render_footer(&mut overlay).contains("[Enter] Select"));
        assert!(
            outcome.into_messages().is_empty(),
            "escape without toggling should produce no change"
        );
    }

    #[test]
    fn model_selector_esc_after_deselecting_all_returns_no_change() {
        let mut overlay = ConfigOverlay::new(make_multi_select_menu(), vec![], vec![]);

        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Down)));
        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Enter))); // open model selector
        // Deselect the pre-selected model
        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Char(' '))));

        let outcome = overlay.on_event(&WidgetEvent::Key(key(KeyCode::Esc)));
        assert!(render_footer(&mut overlay).contains("[Enter] Select"));
        assert!(outcome.into_messages().is_empty()); // No selections => no change
    }

    #[test]
    fn model_selector_enter_toggles_not_confirms() {
        let mut overlay = ConfigOverlay::new(make_multi_select_menu(), vec![], vec![]);

        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Down)));
        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Enter))); // open model selector
        assert!(render_footer(&mut overlay).contains("Toggle"));

        // Enter should toggle, not close the selector
        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Enter)));
        let footer = render_footer(&mut overlay);
        assert!(
            footer.contains("Toggle"),
            "Enter should toggle, not close; got: {footer}"
        );
    }

    #[test]
    fn tall_terminal_shows_more_picker_items() {
        // Create a menu with many model options
        let many_models: Vec<SessionConfigSelectOption> = (0..20)
            .map(|i| SessionConfigSelectOption::new(format!("model-{i}"), format!("Model {i}")))
            .collect();
        let options = vec![agent_client_protocol::SessionConfigOption::select(
            "model",
            "Model",
            "model-0",
            many_models,
        )];
        let menu = ConfigMenu::from_config_options(&options);
        let mut overlay = ConfigOverlay::new(menu, vec![], vec![]);
        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Enter))); // open picker

        // Render at a tall terminal (60 rows)
        let context_tall = ViewContext::new((80, 60));
        let height_tall = (context_tall.size.height.saturating_sub(1)) as usize;
        overlay.update_child_viewport(height_tall.saturating_sub(4));
        let lines_tall = overlay.render(&context_tall);
        let tall_model_lines = lines_tall
            .iter()
            .filter(|l| l.plain_text().contains("Model "))
            .count();

        // Render at a short terminal (15 rows)
        let context_short = ViewContext::new((80, 15));
        let height_short = (context_short.size.height.saturating_sub(1)) as usize;
        overlay.update_child_viewport(height_short.saturating_sub(4));
        let lines_short = overlay.render(&context_short);
        let short_model_lines = lines_short
            .iter()
            .filter(|l| l.plain_text().contains("Model "))
            .count();

        assert!(
            tall_model_lines > short_model_lines,
            "tall terminal ({tall_model_lines} items) should show more picker items than short ({short_model_lines})"
        );
    }

    #[test]
    fn update_config_options_never_renders_reasoning_row() {
        // Initial options include model + reasoning_effort
        let initial_options = vec![
            agent_client_protocol::SessionConfigOption::select(
                "model",
                "Model",
                "claude-opus",
                vec![
                    SessionConfigSelectOption::new("claude-opus", "Claude Opus"),
                    SessionConfigSelectOption::new("deepseek-chat", "DeepSeek Chat"),
                ],
            ),
            agent_client_protocol::SessionConfigOption::select(
                "reasoning_effort",
                "Reasoning Effort",
                "high",
                vec![
                    SessionConfigSelectOption::new("none", "None"),
                    SessionConfigSelectOption::new("low", "Low"),
                    SessionConfigSelectOption::new("medium", "Medium"),
                    SessionConfigSelectOption::new("high", "High"),
                ],
            ),
        ];
        let menu = ConfigMenu::from_config_options(&initial_options);
        let mut overlay = ConfigOverlay::new(menu, vec![], vec![]);

        // Rendered lines do not contain Reasoning Effort
        let context = ViewContext::new((80, 24));
        let lines = overlay.render(&context);
        let text: Vec<String> = lines.iter().map(Line::plain_text).collect();
        assert!(
            !text.iter().any(|l| l.contains("Reasoning Effort")),
            "Reasoning Effort should NOT appear initially; got:\n{}",
            text.join("\n")
        );

        // After update to model-only options, still no Reasoning Effort
        let updated_options = vec![agent_client_protocol::SessionConfigOption::select(
            "model",
            "Model",
            "deepseek-chat",
            vec![
                SessionConfigSelectOption::new("claude-opus", "Claude Opus"),
                SessionConfigSelectOption::new("deepseek-chat", "DeepSeek Chat"),
            ],
        )];
        overlay.update_config_options(&updated_options);

        let lines = overlay.render(&context);
        let text: Vec<String> = lines.iter().map(Line::plain_text).collect();
        assert!(
            !text.iter().any(|l| l.contains("Reasoning Effort")),
            "Reasoning Effort should NOT appear after update; got:\n{}",
            text.join("\n")
        );
    }

    #[test]
    fn footer_shows_toggle_when_model_selector_open() {
        let mut overlay = ConfigOverlay::new(make_multi_select_menu(), vec![], vec![]);

        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Down)));
        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Enter)));

        let context = ViewContext::new((80, 24));
        let lines = overlay.render(&context);
        let footer = lines[lines.len() - 2].plain_text();
        assert!(footer.contains("Toggle"), "footer: {footer}");
        assert!(footer.contains("[Esc] Done"), "footer: {footer}");
    }

    #[test]
    fn model_selector_uses_overlay_reasoning_prefill_after_menu_removal() {
        use crate::components::config_menu::{
            ConfigMenuEntry, ConfigMenuEntryKind, ConfigMenuValue,
        };
        use acp_utils::config_meta::SelectOptionMeta;

        let menu = ConfigMenu {
            options: vec![ConfigMenuEntry {
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
            }],
            selected_index: 0,
        };

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

        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Enter)));
        assert!(
            render_footer(&mut overlay).contains("Toggle"),
            "model selector should be open"
        );

        overlay.on_event(&WidgetEvent::Key(key(KeyCode::Right)));

        let outcome = overlay.on_event(&WidgetEvent::Key(key(KeyCode::Esc)));

        let messages = outcome.into_messages();
        match messages.as_slice() {
            [ConfigOverlayMessage::ApplyConfigChanges(changes)] => {
                let reasoning_change = changes.iter().find(|c| c.config_id == "reasoning_effort");
                assert!(
                    reasoning_change.is_some(),
                    "should have reasoning_effort change; got: {changes:?}"
                );
                assert_eq!(
                    reasoning_change.unwrap().new_value,
                    "high",
                    "reasoning should be high after one right from medium"
                );
            }
            other => panic!("expected ApplyConfigChanges, got: {other:?}"),
        }
    }
}
