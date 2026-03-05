use crate::components::config_menu::{ConfigChange, ConfigMenu, ConfigMenuAction};
use crate::components::config_picker::{ConfigPicker, ConfigPickerAction};
use crate::components::container::Container;
use crate::components::model_selector::{ModelSelector, ModelSelectorAction};
use crate::components::provider_login::{
    ProviderLoginAction, ProviderLoginEntry, ProviderLoginOverlay, ProviderLoginStatus,
    provider_login_summary,
};
use crate::components::server_status::{
    ServerStatusAction, ServerStatusOverlay, server_status_summary,
};
use crate::tui::{Component, HandlesInput, InputOutcome, Line, RenderContext};
use acp_utils::notifications::McpServerStatusEntry;
use agent_client_protocol::{self as acp, SessionConfigOption};
use crossterm::event::KeyEvent;
use unicode_width::UnicodeWidthStr;

const MIN_HEIGHT: usize = 3;
const MIN_WIDTH: usize = 6;
/// Container chrome above child content: top border (1) + blank line (1).
const TOP_CHROME: usize = 2;
/// Container left border width: "│ " = 2 chars.
const BORDER_LEFT_WIDTH: usize = 2;
/// Gap between menu and picker children inside the container.
const GAP: usize = 1;

pub struct ConfigOverlay {
    menu: ConfigMenu,
    picker: Option<ConfigPicker>,
    model_selector: Option<ModelSelector>,
    server_overlay: Option<ServerStatusOverlay>,
    provider_login_overlay: Option<ProviderLoginOverlay>,
    server_statuses: Vec<McpServerStatusEntry>,
    auth_methods: Vec<acp::AuthMethod>,
}

#[derive(Debug)]
pub enum ConfigOverlayAction {
    Close,
    ApplyConfigChange(ConfigChange),
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
        }
    }

    pub fn with_server_overlay(mut self) -> Self {
        self.server_overlay = Some(ServerStatusOverlay::new(self.server_statuses.clone()));
        self
    }

    pub fn update_config_options(&mut self, options: &[SessionConfigOption]) {
        self.menu.update_options(options);
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
    /// Only meaningful when a picker is open (cursor sits on the search line).
    pub fn cursor_row_offset(&self) -> usize {
        if self.picker.is_some() || self.model_selector.is_some() {
            let menu_lines = self.menu.options.len().max(1);
            TOP_CHROME + menu_lines + GAP
        } else {
            0
        }
    }

    pub fn menu_selected_index(&self) -> usize {
        self.menu.selected_index
    }

    pub fn picker_config_id(&self) -> Option<&str> {
        self.picker.as_ref().map(|p| p.config_id.as_str())
    }

    pub fn has_picker(&self) -> bool {
        self.picker.is_some()
    }

    fn footer_text(&self) -> &'static str {
        if self.model_selector.is_some() {
            "[Space/Enter] Toggle  [Esc] Done"
        } else if self.picker.is_some() {
            "[Enter] Confirm  [Esc] Back"
        } else if self.server_overlay.is_some() || self.provider_login_overlay.is_some() {
            "[Enter] Authenticate  [Esc] Back"
        } else {
            "[Enter] Select  [Esc] Close"
        }
    }
}

impl Component for ConfigOverlay {
    fn render(&mut self, context: &RenderContext) -> Vec<Line> {
        let height = (context.size.1.saturating_sub(1)) as usize;
        let width = context.size.0 as usize;
        if height < MIN_HEIGHT || width < MIN_WIDTH {
            return vec![Line::new("(terminal too small)")];
        }

        let footer = self.footer_text();

        let mut children: Vec<&mut dyn Component> = vec![&mut self.menu];
        if let Some(ref mut selector) = self.model_selector {
            children.push(selector);
        } else if let Some(ref mut picker) = self.picker {
            children.push(picker);
        } else if let Some(ref mut server_overlay) = self.server_overlay {
            children.push(server_overlay);
        } else if let Some(ref mut provider_login_overlay) = self.provider_login_overlay {
            children.push(provider_login_overlay);
        }

        Container::new(children)
            .title(" Configuration ")
            .footer(footer)
            .border_color(context.theme.muted)
            .fill_height(height)
            .gap(GAP)
            .render(context)
    }
}

impl HandlesInput for ConfigOverlay {
    type Action = ConfigOverlayAction;

    fn handle_key(&mut self, key_event: KeyEvent) -> InputOutcome<Self::Action> {
        // Server overlay has highest priority
        if let Some(ref mut overlay) = self.server_overlay {
            let outcome = overlay.handle_key(key_event);
            return match outcome.action {
                Some(ServerStatusAction::Close) => {
                    self.server_overlay = None;
                    InputOutcome::consumed_and_render()
                }
                Some(ServerStatusAction::Authenticate(name)) => {
                    InputOutcome::action_and_render(ConfigOverlayAction::AuthenticateServer(name))
                }
                None => {
                    if outcome.needs_render {
                        InputOutcome::consumed_and_render()
                    } else {
                        InputOutcome::consumed()
                    }
                }
            };
        }

        // Provider login overlay has second priority
        if let Some(ref mut overlay) = self.provider_login_overlay {
            let outcome = overlay.handle_key(key_event);
            return match outcome.action {
                Some(ProviderLoginAction::Close) => {
                    self.provider_login_overlay = None;
                    InputOutcome::consumed_and_render()
                }
                Some(ProviderLoginAction::Authenticate(method_id)) => {
                    InputOutcome::action_and_render(ConfigOverlayAction::AuthenticateProvider(
                        method_id,
                    ))
                }
                None => {
                    if outcome.needs_render {
                        InputOutcome::consumed_and_render()
                    } else {
                        InputOutcome::consumed()
                    }
                }
            };
        }

        // Model selector has third priority
        if let Some(ref mut selector) = self.model_selector {
            let outcome = selector.handle_key(key_event);
            return match outcome.action {
                Some(ModelSelectorAction::Done(change)) => {
                    self.model_selector = None;
                    match change {
                        Some(change) => InputOutcome::action_and_render(
                            ConfigOverlayAction::ApplyConfigChange(change),
                        ),
                        None => InputOutcome::consumed_and_render(),
                    }
                }
                None => {
                    if outcome.needs_render {
                        InputOutcome::consumed_and_render()
                    } else {
                        InputOutcome::consumed()
                    }
                }
            };
        }

        // Picker has third priority
        if let Some(ref mut picker) = self.picker {
            let outcome = picker.handle_key(key_event);
            return match outcome.action {
                Some(ConfigPickerAction::Close) => {
                    self.picker = None;
                    InputOutcome::consumed_and_render()
                }
                Some(ConfigPickerAction::ApplySelection(change)) => {
                    self.picker = None;
                    match change {
                        Some(change) => InputOutcome::action_and_render(
                            ConfigOverlayAction::ApplyConfigChange(change),
                        ),
                        None => InputOutcome::consumed_and_render(),
                    }
                }
                None => {
                    if outcome.needs_render {
                        InputOutcome::consumed_and_render()
                    } else {
                        InputOutcome::consumed()
                    }
                }
            };
        }

        // Menu handles remaining input
        let outcome = self.menu.handle_key(key_event);
        match outcome.action {
            Some(ConfigMenuAction::CloseAll) => {
                InputOutcome::action_and_render(ConfigOverlayAction::Close)
            }
            Some(ConfigMenuAction::OpenSelectedPicker) => {
                self.picker = self
                    .menu
                    .selected_entry()
                    .and_then(ConfigPicker::from_entry);
                InputOutcome::consumed_and_render()
            }
            Some(ConfigMenuAction::OpenModelSelector) => {
                if let Some(entry) = self.menu.selected_entry() {
                    let current = Some(entry.current_raw_value.as_str()).filter(|v| !v.is_empty());
                    self.model_selector = Some(ModelSelector::from_model_entry(entry, current));
                }
                InputOutcome::consumed_and_render()
            }
            Some(ConfigMenuAction::OpenMcpServers) => {
                self.server_overlay = Some(ServerStatusOverlay::new(self.server_statuses.clone()));
                InputOutcome::consumed_and_render()
            }
            Some(ConfigMenuAction::OpenProviderLogins) => {
                let entries = self.build_login_entries();
                self.provider_login_overlay = Some(ProviderLoginOverlay::new(entries));
                InputOutcome::consumed_and_render()
            }
            None => {
                if outcome.needs_render {
                    InputOutcome::consumed_and_render()
                } else {
                    InputOutcome::consumed()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acp_utils::notifications::McpServerStatus;
    use agent_client_protocol::SessionConfigSelectOption;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
        let context = RenderContext::new((80, 24));
        let lines = overlay.render(&context);
        lines[lines.len() - 2].plain_text()
    }

    #[test]
    fn bordered_box_fills_terminal_height_minus_one() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        let context = RenderContext::new((80, 24));
        let lines = overlay.render(&context);
        // Should fill exactly height - 1 = 23 lines
        assert_eq!(lines.len(), 23);
    }

    #[test]
    fn title_contains_configuration() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        let context = RenderContext::new((80, 24));
        let lines = overlay.render(&context);
        assert!(lines[0].plain_text().contains("Configuration"));
    }

    #[test]
    fn footer_shows_select_and_close_for_menu() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        let context = RenderContext::new((80, 24));
        let lines = overlay.render(&context);
        let footer = lines[lines.len() - 2].plain_text(); // second to last (last is bottom border)
        assert!(footer.contains("[Enter] Select"), "footer: {footer}");
        assert!(footer.contains("[Esc] Close"), "footer: {footer}");
    }

    #[test]
    fn footer_shows_confirm_and_back_for_picker() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        // Open picker
        overlay.handle_key(key(KeyCode::Enter));
        let context = RenderContext::new((80, 24));
        let lines = overlay.render(&context);
        let footer = lines[lines.len() - 2].plain_text();
        assert!(footer.contains("[Enter] Confirm"), "footer: {footer}");
        assert!(footer.contains("[Esc] Back"), "footer: {footer}");
    }

    #[test]
    fn footer_shows_authenticate_and_back_for_servers() {
        let menu = make_menu();
        let statuses = make_server_statuses();
        let mut overlay = ConfigOverlay::new(menu, statuses, vec![]).with_server_overlay();
        let context = RenderContext::new((80, 24));
        let lines = overlay.render(&context);
        let footer = lines[lines.len() - 2].plain_text();
        assert!(footer.contains("[Enter] Authenticate"), "footer: {footer}");
        assert!(footer.contains("[Esc] Back"), "footer: {footer}");
    }

    #[test]
    fn selected_entry_has_bg_color() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        let context = RenderContext::new((80, 24));
        let lines = overlay.render(&context);
        // The first menu entry (Provider) should be selected with highlight_bg
        let selected_line = &lines[2]; // title + blank + first entry
        let has_bg = selected_line
            .spans()
            .iter()
            .any(|s| s.style().bg == Some(context.theme.highlight_bg));
        assert!(has_bg, "selected entry should have highlight_bg");
    }

    #[test]
    fn esc_closes_overlay() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        let outcome = overlay.handle_key(key(KeyCode::Esc));
        assert!(matches!(outcome.action, Some(ConfigOverlayAction::Close)));
    }

    #[test]
    fn enter_opens_picker() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        let outcome = overlay.handle_key(key(KeyCode::Enter));
        assert!(outcome.consumed);
        assert!(overlay.has_picker());
    }

    #[test]
    fn picker_esc_closes_picker_not_overlay() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        overlay.handle_key(key(KeyCode::Enter)); // open picker
        assert!(overlay.has_picker());

        let outcome = overlay.handle_key(key(KeyCode::Esc));
        assert!(outcome.consumed);
        assert!(!overlay.has_picker());
        // No Close action — overlay remains open
        assert!(outcome.action.is_none());
    }

    #[test]
    fn picker_confirm_returns_config_change_action() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        overlay.handle_key(key(KeyCode::Enter)); // open picker
        overlay.handle_key(key(KeyCode::Down)); // move to second option
        let outcome = overlay.handle_key(key(KeyCode::Enter)); // confirm

        match outcome.action {
            Some(ConfigOverlayAction::ApplyConfigChange(change)) => {
                assert_eq!(change.config_id, "provider");
                assert_eq!(change.new_value, "ollama");
            }
            other => panic!("expected ApplyConfigChange, got: {other:?}"),
        }
    }

    #[test]
    fn server_overlay_esc_closes_server_not_config_overlay() {
        let menu = make_menu();
        let statuses = make_server_statuses();
        let mut overlay = ConfigOverlay::new(menu, statuses, vec![]).with_server_overlay();
        assert!(render_footer(&mut overlay).contains("Authenticate"));

        let outcome = overlay.handle_key(key(KeyCode::Esc));
        assert!(outcome.consumed);
        assert!(render_footer(&mut overlay).contains("[Enter] Select"));
        assert!(outcome.action.is_none());
    }

    #[test]
    fn cursor_col_without_picker_is_zero() {
        let overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        assert_eq!(overlay.cursor_col(), 0);
    }

    #[test]
    fn cursor_col_with_picker_includes_border_and_prefix() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        overlay.handle_key(key(KeyCode::Enter)); // open picker for Provider
        let col = overlay.cursor_col();
        // "│ " (2) + "  Provider search: " (19) + query (0) = should be > 0
        assert!(col > 0);
    }

    #[test]
    fn narrow_terminal_does_not_panic() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        let context = RenderContext::new((4, 3));
        let lines = overlay.render(&context);
        assert!(!lines.is_empty());
    }

    #[test]
    fn very_small_terminal_shows_fallback() {
        let mut overlay = ConfigOverlay::new(make_menu(), vec![], vec![]);
        let context = RenderContext::new((3, 2));
        let lines = overlay.render(&context);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain_text().contains("too small"));
    }

    #[test]
    fn update_config_options_preserves_mcp_servers_entry() {
        use crate::components::config_menu::ConfigMenuEntryKind;

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

        // MCP servers entry should still be present after update
        assert!(
            overlay
                .menu
                .options
                .iter()
                .any(|e| e.entry_kind == ConfigMenuEntryKind::McpServers),
            "MCP servers entry should survive update_config_options"
        );
    }

    #[test]
    fn multi_select_entry_opens_model_selector() {
        let mut overlay = ConfigOverlay::new(make_multi_select_menu(), vec![], vec![]);

        // Navigate to the model entry (index 1: provider=0, model=1)
        overlay.handle_key(key(KeyCode::Down));
        overlay.handle_key(key(KeyCode::Enter));

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
        overlay.handle_key(key(KeyCode::Down));
        overlay.handle_key(key(KeyCode::Enter));
        assert!(render_footer(&mut overlay).contains("Toggle"));

        // Selector pre-selects current model (gpt-4o); Esc without toggling returns no change
        let outcome = overlay.handle_key(key(KeyCode::Esc));
        assert!(outcome.consumed);
        assert!(render_footer(&mut overlay).contains("[Enter] Select"));
        assert!(
            outcome.action.is_none(),
            "escape without toggling should produce no change"
        );
    }

    #[test]
    fn model_selector_esc_after_deselecting_all_returns_no_change() {
        let mut overlay = ConfigOverlay::new(make_multi_select_menu(), vec![], vec![]);

        overlay.handle_key(key(KeyCode::Down));
        overlay.handle_key(key(KeyCode::Enter)); // open model selector
        // Deselect the pre-selected model
        overlay.handle_key(key(KeyCode::Char(' ')));

        let outcome = overlay.handle_key(key(KeyCode::Esc));
        assert!(render_footer(&mut overlay).contains("[Enter] Select"));
        assert!(outcome.action.is_none()); // No selections => no change
    }

    #[test]
    fn model_selector_enter_toggles_not_confirms() {
        let mut overlay = ConfigOverlay::new(make_multi_select_menu(), vec![], vec![]);

        overlay.handle_key(key(KeyCode::Down));
        overlay.handle_key(key(KeyCode::Enter)); // open model selector
        assert!(render_footer(&mut overlay).contains("Toggle"));

        // Enter should toggle, not close the selector
        overlay.handle_key(key(KeyCode::Enter));
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
        overlay.handle_key(key(KeyCode::Enter)); // open picker

        // Render at a tall terminal (60 rows)
        let context_tall = RenderContext::new((80, 60));
        let lines_tall = overlay.render(&context_tall);
        let tall_model_lines = lines_tall
            .iter()
            .filter(|l| l.plain_text().contains("Model "))
            .count();

        // Render at a short terminal (15 rows)
        let context_short = RenderContext::new((80, 15));
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
    fn footer_shows_toggle_when_model_selector_open() {
        let mut overlay = ConfigOverlay::new(make_multi_select_menu(), vec![], vec![]);

        overlay.handle_key(key(KeyCode::Down));
        overlay.handle_key(key(KeyCode::Enter));

        let context = RenderContext::new((80, 24));
        let lines = overlay.render(&context);
        let footer = lines[lines.len() - 2].plain_text();
        assert!(footer.contains("Toggle"), "footer: {footer}");
        assert!(footer.contains("[Esc] Done"), "footer: {footer}");
    }
}
