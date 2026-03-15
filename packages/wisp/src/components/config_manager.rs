use crate::components::config_menu::ConfigMenu;
use crate::components::config_overlay::{ConfigOverlay, ConfigOverlayMessage};
use crate::components::server_status::server_status_summary;
use crate::settings::{list_theme_files, load_or_create_settings};
use crate::tui::{Component, Cursor, Event, Frame, Layout, Theme, ViewContext};
use acp_utils::config_option_id::ConfigOptionId;
use acp_utils::notifications::{McpServerStatus, McpServerStatusEntry};
use agent_client_protocol::{
    self as acp, SessionConfigKind, SessionConfigOption, SessionConfigSelectOptions,
};
use utils::ReasoningEffort;

pub enum ConfigManagerMessage {
    SetConfigOption { config_id: String, value: String },
    SetTheme(Theme),
    AuthenticateServer(String),
    AuthenticateProvider(String),
}

pub struct ConfigManager {
    config_options: Vec<SessionConfigOption>,
    config_overlay: Option<ConfigOverlay>,
    server_statuses: Vec<McpServerStatusEntry>,
    auth_methods: Vec<acp::AuthMethod>,
}

impl ConfigManager {
    pub fn new(
        config_options: &[SessionConfigOption],
        auth_methods: Vec<acp::AuthMethod>,
    ) -> Self {
        Self {
            config_options: config_options.to_vec(),
            config_overlay: None,
            server_statuses: Vec::new(),
            auth_methods,
        }
    }

    pub fn config_options(&self) -> &[SessionConfigOption] {
        &self.config_options
    }

    pub fn unhealthy_server_count(&self) -> usize {
        self.server_statuses
            .iter()
            .filter(|status| !matches!(status.status, McpServerStatus::Connected { .. }))
            .count()
    }

    pub fn is_overlay_open(&self) -> bool {
        self.config_overlay.is_some()
    }

    pub fn on_overlay_event(&mut self, event: &Event) -> Option<Vec<ConfigManagerMessage>> {
        let overlay = self.config_overlay.as_mut()?;
        let outcome = overlay.on_event(event);
        let overlay_messages = outcome.unwrap_or_default();

        let mut messages = Vec::new();
        for msg in overlay_messages {
            match msg {
                ConfigOverlayMessage::Close => {
                    self.config_overlay = None;
                    return Some(messages);
                }
                ConfigOverlayMessage::SetConfigOption { config_id, value } => {
                    messages.push(ConfigManagerMessage::SetConfigOption { config_id, value });
                }
                ConfigOverlayMessage::SetTheme(theme) => {
                    messages.push(ConfigManagerMessage::SetTheme(theme));
                }
                ConfigOverlayMessage::AuthenticateServer(name) => {
                    messages.push(ConfigManagerMessage::AuthenticateServer(name));
                }
                ConfigOverlayMessage::AuthenticateProvider(method_id) => {
                    self.on_authenticate_started(&method_id);
                    messages.push(ConfigManagerMessage::AuthenticateProvider(method_id));
                }
            }
        }
        Some(messages)
    }

    pub fn open_overlay(&mut self) {
        let menu = ConfigMenu::from_config_options(&self.config_options);
        let menu = self.decorate_config_menu(menu);
        self.config_overlay = Some(
            ConfigOverlay::new(
                menu,
                self.server_statuses.clone(),
                self.auth_methods.clone(),
            )
            .with_reasoning_effort_from_options(&self.config_options),
        );
    }

    pub fn close_overlay(&mut self) {
        self.config_overlay = None;
    }

    pub fn cycle_quick_option(&self) -> Option<(String, String)> {
        let option = self
            .config_options
            .iter()
            .find(|option| crate::components::status_line::is_cycleable_mode_option(option))?;

        let SessionConfigKind::Select(ref select) = option.kind else {
            return None;
        };

        let SessionConfigSelectOptions::Ungrouped(ref options) = select.options else {
            return None;
        };

        if options.is_empty() {
            return None;
        }

        let current_index = options
            .iter()
            .position(|entry| entry.value == select.current_value)
            .unwrap_or(0);
        let next_index = (current_index + 1) % options.len();
        options
            .get(next_index)
            .map(|next| (option.id.0.to_string(), next.value.0.to_string()))
    }

    pub fn cycle_reasoning_option(&self) -> Option<(String, String)> {
        let has_reasoning = self
            .config_options
            .iter()
            .any(|option| option.id.0.as_ref() == ConfigOptionId::ReasoningEffort.as_str());

        if has_reasoning {
            let current =
                crate::components::status_line::extract_reasoning_effort(&self.config_options);
            let next = ReasoningEffort::cycle_next(current);
            Some((
                ConfigOptionId::ReasoningEffort.as_str().to_string(),
                ReasoningEffort::config_str(next).to_string(),
            ))
        } else {
            None
        }
    }

    pub fn update_config_options(&mut self, config_options: &[SessionConfigOption]) {
        self.config_options = config_options.to_vec();
        if let Some(ref mut overlay) = self.config_overlay {
            overlay.update_config_options(config_options);
        }
    }

    pub fn update_server_statuses(&mut self, servers: Vec<McpServerStatusEntry>) {
        if let Some(ref mut overlay) = self.config_overlay {
            overlay.update_server_statuses(servers.clone());
        }
        self.server_statuses = servers;
    }

    fn on_authenticate_started(&mut self, method_id: &str) {
        if let Some(ref mut overlay) = self.config_overlay {
            overlay.on_authenticate_started(method_id);
        }
    }

    pub fn on_authenticate_complete(&mut self, method_id: &str) {
        self.auth_methods
            .retain(|method| method.id().0.as_ref() != method_id);
        if let Some(ref mut overlay) = self.config_overlay {
            overlay.remove_auth_method(method_id);
        }
    }

    pub fn on_authenticate_failed(&mut self, method_id: &str) {
        if let Some(ref mut overlay) = self.config_overlay {
            overlay.on_authenticate_failed(method_id);
        }
    }

    pub fn build_overlay_frame(&self, ctx: &ViewContext) -> Option<Frame> {
        let overlay = self.config_overlay.as_ref()?;
        let cursor = if overlay.has_picker() {
            Cursor::visible(overlay.cursor_row_offset(), overlay.cursor_col())
        } else {
            Cursor::hidden()
        };

        let mut layout = Layout::new();
        layout.section(overlay.render(ctx).into_lines());
        Some(layout.into_frame().with_cursor(cursor))
    }

    pub fn update_overlay_viewport(&mut self, max_height: usize) {
        if let Some(ref mut overlay) = self.config_overlay
            && max_height >= 3 {
                overlay.update_child_viewport(max_height.saturating_sub(4));
            }
    }

    fn decorate_config_menu(&self, mut menu: ConfigMenu) -> ConfigMenu {
        let settings = load_or_create_settings();
        let theme_files = list_theme_files();
        menu.add_theme_entry(settings.theme.file.as_deref(), &theme_files);

        let server_summary = server_status_summary(&self.server_statuses);
        menu.add_mcp_servers_entry(&server_summary);
        if !self.auth_methods.is_empty() {
            let summary = format!("{} needs login", self.auth_methods.len());
            menu.add_provider_logins_entry(&summary);
        }
        menu
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn new_initializes_defaults() {
        let cm = ConfigManager::new(&[], vec![]);
        assert!(!cm.is_overlay_open());
        assert!(cm.config_options().is_empty());
        assert_eq!(cm.unhealthy_server_count(), 0);
    }

    #[test]
    fn open_and_close_overlay() {
        let mut cm = ConfigManager::new(&[], vec![]);
        cm.open_overlay();
        assert!(cm.is_overlay_open());
        cm.close_overlay();
        assert!(!cm.is_overlay_open());
    }

    #[test]
    fn update_config_options_stores_options() {
        let mut cm = ConfigManager::new(&[], vec![]);
        let options = vec![acp::SessionConfigOption::select(
            "model",
            "Model",
            "m1",
            vec![acp::SessionConfigSelectOption::new("m1", "M1")],
        )];
        cm.update_config_options(&options);
        assert_eq!(cm.config_options().len(), 1);
    }

    #[test]
    fn on_authenticate_complete_removes_method() {
        let mut cm = ConfigManager::new(
            &[],
            vec![acp::AuthMethod::Agent(acp::AuthMethodAgent::new(
                "anthropic",
                "Anthropic",
            ))],
        );
        cm.on_authenticate_complete("anthropic");
        assert!(cm.auth_methods.is_empty());
    }
}
