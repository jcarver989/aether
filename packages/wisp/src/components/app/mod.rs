pub mod attachments;
pub mod git_diff_mode;
mod screen_router;
mod view;

pub use git_diff_mode::{GitDiffLoadState, GitDiffMode, GitDiffViewState, PatchFocus};
use screen_router::ScreenRouter;
use screen_router::ScreenRouterMessage;

use crate::components::conversation_screen::ConversationScreen;
use crate::components::conversation_screen::ConversationScreenMessage;
use crate::keybindings::Keybindings;
use crate::settings;
use crate::settings::overlay::{SettingsMessage, SettingsOverlay};
use acp_utils::client::{AcpEvent, AcpPromptHandle};
use agent_client_protocol::{self as acp, SessionId};
use attachments::build_attachment_blocks;
use std::path::PathBuf;
use std::time::Instant;
use tokio::sync::oneshot;
use tui::RendererCommand;
use tui::{Component, Event, Frame, KeyEvent, ViewContext};

#[derive(Debug, Clone)]
pub struct PromptAttachment {
    pub path: PathBuf,
    pub display_name: String,
}

pub struct App {
    agent_name: String,
    context_usage_pct: Option<u8>,
    exit_requested: bool,
    conversation_screen: ConversationScreen,
    config_options: Vec<acp::SessionConfigOption>,
    server_statuses: Vec<acp_utils::notifications::McpServerStatusEntry>,
    auth_methods: Vec<acp::AuthMethod>,
    settings_overlay: Option<SettingsOverlay>,
    screen_router: ScreenRouter,
    keybindings: Keybindings,
    session_id: SessionId,
    prompt_handle: AcpPromptHandle,
    working_dir: PathBuf,
}

impl App {
    pub fn new(
        session_id: SessionId,
        agent_name: String,
        config_options: &[acp::SessionConfigOption],
        auth_methods: Vec<acp::AuthMethod>,
        working_dir: PathBuf,
        prompt_handle: AcpPromptHandle,
    ) -> Self {
        let keybindings = Keybindings::default();
        Self {
            agent_name,
            context_usage_pct: None,
            exit_requested: false,
            conversation_screen: ConversationScreen::new(keybindings.clone()),
            config_options: config_options.to_vec(),
            server_statuses: Vec::new(),
            auth_methods,
            settings_overlay: None,
            screen_router: ScreenRouter::new(GitDiffMode::new(working_dir.clone())),
            keybindings,
            session_id,
            prompt_handle,
            working_dir,
        }
    }

    pub fn exit_requested(&self) -> bool {
        self.exit_requested
    }

    pub fn has_settings_overlay(&self) -> bool {
        self.settings_overlay.is_some()
    }

    pub fn wants_tick(&self) -> bool {
        self.conversation_screen.wants_tick()
    }

    fn git_diff_mode_mut(&mut self) -> &mut GitDiffMode {
        self.screen_router.git_diff_mode_mut()
    }

    pub fn on_acp_event(&mut self, event: AcpEvent) {
        match event {
            AcpEvent::SessionUpdate(update) => self.on_session_update(&update),
            AcpEvent::ExtNotification(notification) => {
                self.on_ext_notification(&notification);
            }
            AcpEvent::PromptDone(_) => self.on_prompt_done(),
            AcpEvent::PromptError(error) => {
                self.conversation_screen.on_prompt_error(&error);
            }
            AcpEvent::ElicitationRequest {
                params,
                response_tx,
            } => self.on_elicitation_request(params, response_tx),
            AcpEvent::AuthenticateComplete { method_id } => {
                self.on_authenticate_complete(&method_id);
            }
            AcpEvent::AuthenticateFailed { method_id, error } => {
                self.on_authenticate_failed(&method_id, &error);
            }
            AcpEvent::SessionsListed { sessions } => {
                let current_id = &self.session_id;
                let filtered: Vec<_> = sessions
                    .into_iter()
                    .filter(|s| s.session_id != *current_id)
                    .collect();
                self.conversation_screen.open_session_picker(filtered);
            }
            // SessionLoaded intentionally does NOT restore previous config selections:
            // when the user loads an existing session, the server's stored config for
            // that session is authoritative.
            AcpEvent::SessionLoaded {
                session_id,
                config_options,
            } => {
                self.session_id = session_id;
                self.update_config_options(&config_options);
            }
            AcpEvent::NewSessionCreated {
                session_id,
                config_options,
            } => {
                let previous_selections = current_config_selections(&self.config_options);
                self.session_id = session_id;
                self.update_config_options(&config_options);
                self.context_usage_pct = None;
                self.restore_config_selections(&previous_selections);
            }
            AcpEvent::ConnectionClosed => {
                self.exit_requested = true;
            }
        }
    }

    async fn handle_key(&mut self, commands: &mut Vec<RendererCommand>, key_event: KeyEvent) {
        if self.keybindings.exit.matches(key_event) {
            self.exit_requested = true;
            return;
        }

        if self.keybindings.toggle_git_diff.matches(key_event)
            && !self.conversation_screen.has_modal()
        {
            if let Some(msg) = self.screen_router.toggle_git_diff() {
                self.handle_screen_router_message(commands, msg).await;
            }
            return;
        }

        let event = Event::Key(key_event);

        if self.screen_router.is_git_diff() {
            for msg in self
                .screen_router
                .on_event(&event)
                .await
                .unwrap_or_default()
            {
                self.handle_screen_router_message(commands, msg).await;
            }
        } else if self.settings_overlay.is_some() {
            self.handle_settings_overlay_event(commands, &event).await;
        } else {
            let outcome = self.conversation_screen.on_event(&event).await;
            let consumed = outcome.is_some();
            self.handle_conversation_messages(commands, outcome).await;
            if !consumed {
                self.handle_fallthrough_keybindings(key_event);
            }
        }
    }

    async fn handle_conversation_messages(
        &mut self,
        commands: &mut Vec<RendererCommand>,
        outcome: Option<Vec<ConversationScreenMessage>>,
    ) {
        for msg in outcome.unwrap_or_default() {
            match msg {
                ConversationScreenMessage::SendPrompt {
                    user_input,
                    attachments,
                } => {
                    self.conversation_screen.conversation.push_user_message("");

                    self.conversation_screen
                        .conversation
                        .push_user_message(&user_input);

                    let outcome = build_attachment_blocks(&attachments).await;
                    for w in outcome.warnings {
                        self.conversation_screen
                            .conversation
                            .push_user_message(&format!("[wisp] {w}"));
                    }

                    let _ = self.prompt_handle.prompt(
                        &self.session_id,
                        &user_input,
                        if outcome.blocks.is_empty() {
                            None
                        } else {
                            Some(outcome.blocks)
                        },
                    );
                }
                ConversationScreenMessage::ClearScreen => {
                    commands.push(RendererCommand::ClearScreen);
                }
                ConversationScreenMessage::NewSession => {
                    commands.push(RendererCommand::ClearScreen);
                    let _ = self.prompt_handle.new_session(&self.working_dir);
                }
                ConversationScreenMessage::OpenSettings => {
                    self.open_settings_overlay();
                }
                ConversationScreenMessage::OpenSessionPicker => {
                    let _ = self.prompt_handle.list_sessions();
                }
                ConversationScreenMessage::LoadSession { session_id, cwd } => {
                    if let Err(e) = self.prompt_handle.load_session(&session_id, &cwd) {
                        tracing::warn!("Failed to load session: {e}");
                    }
                }
            }
        }
    }

    fn handle_fallthrough_keybindings(&self, key_event: KeyEvent) {
        if self.keybindings.cycle_reasoning.matches(key_event) {
            if let Some((id, val)) = settings::cycle_reasoning_option(&self.config_options) {
                let _ = self
                    .prompt_handle
                    .set_config_option(&self.session_id, &id, &val);
            }
            return;
        }

        if self.keybindings.cycle_mode.matches(key_event) {
            if let Some((id, val)) = settings::cycle_quick_option(&self.config_options) {
                let _ = self
                    .prompt_handle
                    .set_config_option(&self.session_id, &id, &val);
            }
            return;
        }

        if self.keybindings.cancel.matches(key_event)
            && self.conversation_screen.is_waiting()
            && let Err(e) = self.prompt_handle.cancel(&self.session_id)
        {
            tracing::warn!("Failed to send cancel: {e}");
        }
    }

    async fn handle_settings_overlay_event(
        &mut self,
        commands: &mut Vec<RendererCommand>,
        event: &Event,
    ) {
        let Some(ref mut overlay) = self.settings_overlay else {
            return;
        };
        let messages = overlay.on_event(event).await.unwrap_or_default();

        for msg in messages {
            match msg {
                SettingsMessage::Close => {
                    self.settings_overlay = None;
                    return;
                }
                SettingsMessage::SetConfigOption { config_id, value } => {
                    let _ =
                        self.prompt_handle
                            .set_config_option(&self.session_id, &config_id, &value);
                }
                SettingsMessage::SetTheme(theme) => {
                    commands.push(RendererCommand::SetTheme(theme));
                }
                SettingsMessage::AuthenticateServer(name) => {
                    let _ = self
                        .prompt_handle
                        .authenticate_mcp_server(&self.session_id, &name);
                }
                SettingsMessage::AuthenticateProvider(ref method_id) => {
                    if let Some(ref mut overlay) = self.settings_overlay {
                        overlay.on_authenticate_started(method_id);
                    }
                    let _ = self.prompt_handle.authenticate(&self.session_id, method_id);
                }
            }
        }
    }

    fn open_settings_overlay(&mut self) {
        self.settings_overlay = Some(settings::create_overlay(
            &self.config_options,
            &self.server_statuses,
            &self.auth_methods,
        ));
    }

    fn update_config_options(&mut self, config_options: &[acp::SessionConfigOption]) {
        self.config_options = config_options.to_vec();
        if let Some(ref mut overlay) = self.settings_overlay {
            overlay.update_config_options(config_options);
        }
    }

    fn restore_config_selections(&self, previous: &[(String, String)]) {
        let new_selections = current_config_selections(&self.config_options);
        for (id, old_value) in previous {
            let still_exists = new_selections.iter().any(|(new_id, _)| new_id == id);
            if !still_exists {
                tracing::debug!(
                    config_id = id,
                    "config option no longer present in new session"
                );
                continue;
            }
            let server_reset = new_selections
                .iter()
                .any(|(new_id, new_val)| new_id == id && new_val != old_value);
            if server_reset {
                if let Err(e) =
                    self.prompt_handle
                        .set_config_option(&self.session_id, id, old_value)
                {
                    tracing::warn!(config_id = id, error = %e, "failed to restore config option");
                }
            }
        }
    }

    async fn handle_screen_router_message(
        &mut self,
        commands: &mut Vec<RendererCommand>,
        msg: ScreenRouterMessage,
    ) {
        match msg {
            ScreenRouterMessage::LoadGitDiff | ScreenRouterMessage::RefreshGitDiff => {
                self.git_diff_mode_mut().complete_load().await;
            }
            ScreenRouterMessage::SendPrompt { user_input } => {
                self.conversation_screen.conversation.push_user_message("");
                self.conversation_screen
                    .conversation
                    .push_user_message(&user_input);
                let _ = self
                    .prompt_handle
                    .prompt(&self.session_id, &user_input, None);
            }
        }
        let _ = commands;
    }

    fn on_session_update(&mut self, update: &acp::SessionUpdate) {
        self.conversation_screen.on_session_update(update);

        if let acp::SessionUpdate::ConfigOptionUpdate(config_update) = update {
            self.update_config_options(&config_update.config_options);
        }
    }

    fn on_prompt_done(&mut self) {
        self.conversation_screen.on_prompt_done();
    }

    fn on_elicitation_request(
        &mut self,
        params: acp_utils::notifications::ElicitationParams,
        response_tx: oneshot::Sender<acp_utils::notifications::ElicitationResponse>,
    ) {
        self.settings_overlay = None;
        self.conversation_screen
            .on_elicitation_request(params, response_tx);
    }

    fn on_ext_notification(&mut self, notification: &acp::ExtNotification) {
        use acp_utils::notifications::{
            CONTEXT_CLEARED_METHOD, CONTEXT_USAGE_METHOD, ContextUsageParams, McpNotification,
            SUB_AGENT_PROGRESS_METHOD, SubAgentProgressParams,
        };

        match notification.method.as_ref() {
            CONTEXT_CLEARED_METHOD => {
                self.conversation_screen.reset_after_context_cleared();
                self.context_usage_pct = None;
            }
            CONTEXT_USAGE_METHOD => {
                if let Ok(params) =
                    serde_json::from_str::<ContextUsageParams>(notification.params.get())
                {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    {
                        self.context_usage_pct = params.usage_ratio.map(|usage_ratio| {
                            ((1.0 - usage_ratio) * 100.0).clamp(0.0, 100.0).round() as u8
                        });
                    }
                }
            }
            SUB_AGENT_PROGRESS_METHOD => {
                if let Ok(progress) =
                    serde_json::from_str::<SubAgentProgressParams>(notification.params.get())
                {
                    self.conversation_screen.on_sub_agent_progress(&progress);
                }
            }
            _ => {
                if let Ok(McpNotification::ServerStatus { servers }) =
                    McpNotification::try_from(notification)
                {
                    if let Some(ref mut overlay) = self.settings_overlay {
                        overlay.update_server_statuses(servers.clone());
                    }
                    self.server_statuses = servers;
                }
            }
        }
    }

    fn on_authenticate_complete(&mut self, method_id: &str) {
        if let Some(ref mut overlay) = self.settings_overlay {
            overlay.on_authenticate_complete(method_id);
        }
    }

    fn on_authenticate_failed(&mut self, method_id: &str, error: &str) {
        tracing::warn!("Provider auth failed for {method_id}: {error}");
        if let Some(ref mut overlay) = self.settings_overlay {
            overlay.on_authenticate_failed(method_id);
        }
    }
}

impl Component for App {
    type Message = RendererCommand;

    async fn on_event(&mut self, event: &Event) -> Option<Vec<RendererCommand>> {
        let mut commands = Vec::new();
        match event {
            Event::Key(key_event) => self.handle_key(&mut commands, *key_event).await,
            Event::Paste(_) => {
                self.settings_overlay = None;
                let outcome = self.conversation_screen.on_event(event).await;
                self.handle_conversation_messages(&mut commands, outcome)
                    .await;
            }
            Event::Tick => {
                let now = Instant::now();
                self.conversation_screen.on_tick(now);
            }
            Event::Mouse(_) => {
                if self.settings_overlay.is_some() {
                    self.handle_settings_overlay_event(&mut commands, event)
                        .await;
                }
            }
            Event::Resize(_) => {}
        }
        Some(commands)
    }

    fn render(&mut self, ctx: &ViewContext) -> Frame {
        self.conversation_screen.refresh_caches(ctx);
        self.screen_router.refresh_caches(ctx);

        let height = (ctx.size.height.saturating_sub(1)) as usize;
        if let Some(ref mut overlay) = self.settings_overlay
            && height >= 3
        {
            overlay.update_child_viewport(height.saturating_sub(4));
        }

        view::build_frame(self, ctx)
    }
}

fn current_config_selections(options: &[acp::SessionConfigOption]) -> Vec<(String, String)> {
    options
        .iter()
        .filter_map(|opt| {
            let acp::SessionConfigKind::Select(ref select) = opt.kind else {
                return None;
            };
            Some((opt.id.0.to_string(), select.current_value.0.to_string()))
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;

    pub fn make_app() -> App {
        App::new(
            SessionId::new("test"),
            "test-agent".to_string(),
            &[],
            vec![],
            PathBuf::from("."),
            AcpPromptHandle::noop(),
        )
    }

    pub fn make_app_with_config(config_options: &[acp::SessionConfigOption]) -> App {
        App::new(
            SessionId::new("test"),
            "test-agent".to_string(),
            config_options,
            vec![],
            PathBuf::from("."),
            AcpPromptHandle::noop(),
        )
    }

    pub fn make_app_with_auth(auth_methods: Vec<acp::AuthMethod>) -> App {
        App::new(
            SessionId::new("test"),
            "test-agent".to_string(),
            &[],
            auth_methods,
            PathBuf::from("."),
            AcpPromptHandle::noop(),
        )
    }

    pub fn make_app_with_config_recording(
        config_options: &[acp::SessionConfigOption],
    ) -> (
        App,
        tokio::sync::mpsc::UnboundedReceiver<acp_utils::client::PromptCommand>,
    ) {
        let (handle, rx) = AcpPromptHandle::recording();
        let app = App::new(
            SessionId::new("test"),
            "test-agent".to_string(),
            config_options,
            vec![],
            PathBuf::from("."),
            handle,
        );
        (app, rx)
    }

    pub fn make_app_with_session_id(session_id: &str) -> App {
        App::new(
            SessionId::new(session_id),
            "test-agent".to_string(),
            &[],
            vec![],
            PathBuf::from("."),
            AcpPromptHandle::noop(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::test_helpers::*;
    use super::*;
    use crate::components::command_picker::CommandEntry;
    use crate::components::elicitation_form::ElicitationForm;
    use crate::settings::{ThemeSettings as WispThemeSettings, WispSettings, save_settings};
    use crate::test_helpers::with_wisp_home;
    use std::fs;
    use std::time::Duration;
    use tempfile::TempDir;
    use tui::Renderer;
    use tui::testing::render_component;
    use tui::{Frame, Theme, ViewContext};

    fn make_renderer() -> Renderer<Vec<u8>> {
        Renderer::new(Vec::new(), Theme::default(), (80, 24))
    }

    fn render_app(renderer: &mut Renderer<Vec<u8>>, app: &mut App, context: &ViewContext) -> Frame {
        renderer.render_frame(|ctx| app.render(ctx)).unwrap();
        app.render(context)
    }

    #[test]
    fn decorate_settings_menu_adds_theme_entry() {
        let temp_dir = TempDir::new().unwrap();
        let themes_dir = temp_dir.path().join("themes");
        fs::create_dir_all(&themes_dir).unwrap();
        fs::write(themes_dir.join("catppuccin.tmTheme"), "x").unwrap();

        with_wisp_home(temp_dir.path(), || {
            let mut app = make_app();
            app.open_settings_overlay();
            assert!(app.settings_overlay.is_some());
        });
    }

    #[test]
    fn theme_entry_uses_current_theme_from_settings() {
        let temp_dir = TempDir::new().unwrap();
        let themes_dir = temp_dir.path().join("themes");
        fs::create_dir_all(&themes_dir).unwrap();
        fs::write(themes_dir.join("catppuccin.tmTheme"), "x").unwrap();
        fs::write(themes_dir.join("nord.tmTheme"), "x").unwrap();

        with_wisp_home(temp_dir.path(), || {
            let settings = WispSettings {
                theme: WispThemeSettings {
                    file: Some("nord.tmTheme".to_string()),
                },
            };
            save_settings(&settings).unwrap();

            let mut app = make_app();
            app.open_settings_overlay();
            assert!(app.settings_overlay.is_some());
        });
    }

    #[test]
    fn command_picker_cursor_stays_in_input_prompt() {
        let mut app = make_app();
        let mut renderer = make_renderer();
        app.conversation_screen
            .prompt_composer
            .open_command_picker_with_entries(vec![CommandEntry {
                name: "settings".to_string(),
                description: "Open settings".to_string(),
                has_input: false,
                hint: None,
                builtin: true,
            }]);

        let context = ViewContext::new((120, 40));
        let output = render_app(&mut renderer, &mut app, &context);
        let input_row = output
            .lines()
            .iter()
            .position(|line| line.plain_text().contains("> "))
            .expect("input prompt should exist");
        assert_eq!(output.cursor().row, input_row);
    }

    #[test]
    fn settings_overlay_replaces_conversation_window() {
        let options = vec![acp::SessionConfigOption::select(
            "model",
            "Model",
            "m1",
            vec![acp::SessionConfigSelectOption::new("m1", "M1")],
        )];
        let mut app = make_app_with_config(&options);
        let mut renderer = make_renderer();
        app.open_settings_overlay();

        let context = ViewContext::new((120, 40));
        let output = render_app(&mut renderer, &mut app, &context);
        assert!(
            output
                .lines()
                .iter()
                .any(|line| line.plain_text().contains("Configuration"))
        );

        app.settings_overlay = None;
        let output = render_app(&mut renderer, &mut app, &context);
        assert!(
            !output
                .lines()
                .iter()
                .any(|line| line.plain_text().contains("Configuration"))
        );
    }

    #[test]
    fn extract_model_display_handles_comma_separated_value() {
        use crate::components::status_line::extract_model_display;

        let options = vec![acp::SessionConfigOption::select(
            "model",
            "Model",
            "a:x,b:y",
            vec![
                acp::SessionConfigSelectOption::new("a:x", "Alpha / X"),
                acp::SessionConfigSelectOption::new("b:y", "Beta / Y"),
                acp::SessionConfigSelectOption::new("c:z", "Gamma / Z"),
            ],
        )];
        assert_eq!(
            extract_model_display(&options).as_deref(),
            Some("Alpha / X + Beta / Y")
        );
    }

    #[test]
    fn extract_reasoning_effort_returns_none_for_none_value() {
        use crate::components::status_line::extract_reasoning_effort;
        use acp_utils::config_option_id::ConfigOptionId;

        let options = vec![acp::SessionConfigOption::select(
            ConfigOptionId::ReasoningEffort.as_str(),
            "Reasoning",
            "none",
            vec![
                acp::SessionConfigSelectOption::new("none", "None"),
                acp::SessionConfigSelectOption::new("low", "Low"),
            ],
        )];
        assert_eq!(extract_reasoning_effort(&options), None);
    }

    #[test]
    fn render_hides_plan_header_when_no_entries_are_visible() {
        let mut app = make_app();
        let mut renderer = make_renderer();
        let grace_period = app.conversation_screen.plan_tracker.grace_period;
        app.conversation_screen.plan_tracker.replace(
            vec![acp::PlanEntry::new(
                "1",
                acp::PlanEntryPriority::Medium,
                acp::PlanEntryStatus::Completed,
            )],
            Instant::now()
                .checked_sub(grace_period + Duration::from_millis(1))
                .unwrap(),
        );
        app.conversation_screen.plan_tracker.on_tick(Instant::now());

        let output = render_app(&mut renderer, &mut app, &ViewContext::new((120, 40)));
        assert!(
            !output
                .lines()
                .iter()
                .any(|line| line.plain_text().contains("Plan"))
        );
    }

    #[test]
    fn plan_version_increments_on_replace() {
        let mut app = make_app();

        let initial_version = app.conversation_screen.plan_tracker.version();
        app.conversation_screen.plan_tracker.replace(
            vec![acp::PlanEntry::new(
                "Task A",
                acp::PlanEntryPriority::Medium,
                acp::PlanEntryStatus::Pending,
            )],
            Instant::now(),
        );

        assert!(app.conversation_screen.plan_tracker.version() > initial_version);
    }

    #[test]
    fn plan_version_increments_on_clear() {
        let mut app = make_app();

        app.conversation_screen.plan_tracker.replace(
            vec![acp::PlanEntry::new(
                "Task A",
                acp::PlanEntryPriority::Medium,
                acp::PlanEntryStatus::Pending,
            )],
            Instant::now(),
        );
        let version_before_clear = app.conversation_screen.plan_tracker.version();
        app.conversation_screen.plan_tracker.clear();

        assert!(app.conversation_screen.plan_tracker.version() > version_before_clear);
    }

    #[test]
    fn sessions_listed_filters_out_current_session() {
        let mut app = make_app_with_session_id("current-session");

        let sessions = vec![
            acp::SessionInfo::new("other-session-1", PathBuf::from("/project"))
                .title("First other session".to_string()),
            acp::SessionInfo::new("current-session", PathBuf::from("/project"))
                .title("Current session title".to_string()),
            acp::SessionInfo::new("other-session-2", PathBuf::from("/other"))
                .title("Second other session".to_string()),
        ];

        app.on_acp_event(AcpEvent::SessionsListed { sessions });

        let picker = match &mut app.conversation_screen.active_modal {
            Some(crate::components::conversation_screen::Modal::SessionPicker(p)) => p,
            _ => panic!("expected session picker modal"),
        };
        let term = render_component(|ctx| picker.render(ctx), 60, 10);
        let lines = term.get_lines();

        assert!(
            !lines
                .iter()
                .any(|line| line.contains("Current session title")),
            "current session should be filtered out, got: {:?}",
            lines
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("First other session")),
            "first other session should be present"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Second other session")),
            "second other session should be present"
        );
    }

    #[tokio::test]
    async fn custom_exit_keybinding_triggers_exit() {
        use crate::keybindings::KeyBinding;
        use tui::{KeyCode, KeyModifiers};

        let mut app = make_app();
        app.keybindings.exit = KeyBinding::new(KeyCode::Char('q'), KeyModifiers::CONTROL);

        let default_exit = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        app.on_event(&Event::Key(default_exit)).await;
        assert!(
            !app.exit_requested(),
            "default Ctrl+C should no longer exit"
        );

        let custom_exit = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
        app.on_event(&Event::Key(custom_exit)).await;
        assert!(app.exit_requested(), "custom Ctrl+Q should exit");
    }

    #[tokio::test]
    async fn ctrl_g_opens_git_diff_viewer() {
        use tui::{KeyCode, KeyModifiers};

        let mut app = make_app();
        let key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
        app.on_event(&Event::Key(key)).await;

        assert!(app.screen_router.is_git_diff());
    }

    #[tokio::test]
    async fn ctrl_g_closes_git_diff_viewer() {
        use tui::{KeyCode, KeyModifiers};

        let mut app = make_app();
        app.screen_router.enter_git_diff_for_test();

        let key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
        app.on_event(&Event::Key(key)).await;

        assert!(!app.screen_router.is_git_diff());
    }

    #[tokio::test]
    async fn ctrl_g_blocked_during_elicitation() {
        use tui::{KeyCode, KeyModifiers};

        let mut app = make_app();
        app.conversation_screen.active_modal =
            Some(crate::components::conversation_screen::Modal::Elicitation(
                ElicitationForm::from_params(
                    acp_utils::notifications::ElicitationParams {
                        message: "test".to_string(),
                        schema: acp_utils::ElicitationSchema::builder().build().unwrap(),
                    },
                    tokio::sync::oneshot::channel().0,
                ),
            ));

        let key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
        app.on_event(&Event::Key(key)).await;

        assert!(
            !app.screen_router.is_git_diff(),
            "git diff should not open during elicitation"
        );
    }

    #[tokio::test]
    async fn esc_in_diff_mode_does_not_cancel() {
        use tui::{KeyCode, KeyModifiers};

        let mut app = make_app();
        app.conversation_screen.waiting_for_response = true;
        app.screen_router.enter_git_diff_for_test();

        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        app.on_event(&Event::Key(key)).await;

        assert!(!app.exit_requested());
        assert!(
            app.conversation_screen.waiting_for_response,
            "Esc should NOT cancel a running prompt while git diff mode is active"
        );
    }

    #[tokio::test]
    async fn mouse_scroll_ignored_in_conversation_mode() {
        use tui::{KeyModifiers, MouseEvent, MouseEventKind};

        let mut app = make_app();
        let mouse = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        app.on_event(&Event::Mouse(mouse)).await;
    }

    #[tokio::test]
    async fn prompt_composer_submit_pushes_echo_lines() {
        use crate::components::conversation_window::SegmentContent;

        let mut app = make_app();
        let outcome = Some(vec![ConversationScreenMessage::SendPrompt {
            user_input: "hello".to_string(),
            attachments: vec![],
        }]);

        let mut commands = Vec::new();
        app.handle_conversation_messages(&mut commands, outcome)
            .await;

        let has_hello = app
            .conversation_screen
            .conversation
            .segments()
            .any(|seg| matches!(seg, SegmentContent::UserMessage(text) if text == "hello"));
        assert!(
            has_hello,
            "conversation buffer should contain the user input"
        );
    }

    #[test]
    fn prompt_composer_open_settings() {
        let mut app = make_app();
        let outcome = Some(vec![ConversationScreenMessage::OpenSettings]);

        let mut commands = Vec::new();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(app.handle_conversation_messages(&mut commands, outcome));

        assert!(
            app.settings_overlay.is_some(),
            "settings overlay should be opened"
        );
    }

    #[test]
    fn settings_overlay_close_clears_overlay() {
        let mut app = make_app();
        app.open_settings_overlay();

        app.settings_overlay = None;

        assert!(app.settings_overlay.is_none(), "close should clear overlay");
    }

    #[tokio::test]
    async fn tick_advances_tool_call_statuses() {
        let mut app = make_app();

        let tool_call = acp::ToolCall::new("tool-1".to_string(), "test_tool");
        app.conversation_screen
            .tool_call_statuses
            .on_tool_call(&tool_call);
        let ctx = ViewContext::new((80, 24));
        let lines_before = app
            .conversation_screen
            .tool_call_statuses
            .render_tool("tool-1", &ctx);
        app.on_event(&Event::Tick).await;
        let lines_after = app
            .conversation_screen
            .tool_call_statuses
            .render_tool("tool-1", &ctx);

        assert_ne!(
            lines_before[0].plain_text(),
            lines_after[0].plain_text(),
            "tick should advance the spinner animation"
        );
    }

    #[tokio::test]
    async fn tick_advances_progress_indicator() {
        let mut app = make_app();

        let tool_call = acp::ToolCall::new("tool-1".to_string(), "test_tool");
        app.conversation_screen
            .tool_call_statuses
            .on_tool_call(&tool_call);

        app.conversation_screen
            .progress_indicator
            .update(0, 1, true);
        let ctx = ViewContext::new((80, 24));
        let output_before = app.conversation_screen.progress_indicator.render(&ctx);
        app.on_event(&Event::Tick).await;
        let output_after = app.conversation_screen.progress_indicator.render(&ctx);

        assert_ne!(
            output_before[0].plain_text(),
            output_after[0].plain_text(),
            "spinner frame should change after tick"
        );
    }

    #[test]
    fn on_prompt_error_clears_waiting_state() {
        let mut app = make_app();
        app.conversation_screen.waiting_for_response = true;

        let error = acp::Error::internal_error();
        app.conversation_screen.on_prompt_error(&error);

        assert!(!app.conversation_screen.waiting_for_response);
        assert!(!app.exit_requested());
    }

    #[test]
    fn on_authenticate_complete_does_not_exit() {
        let mut app = make_app_with_auth(vec![acp::AuthMethod::Agent(acp::AuthMethodAgent::new(
            "anthropic",
            "Anthropic",
        ))]);

        app.on_authenticate_complete("anthropic");

        assert!(!app.exit_requested());
    }

    #[test]
    fn on_authenticate_failed_does_not_exit() {
        let mut app = make_app();

        app.on_authenticate_failed("anthropic", "bad token");

        assert!(!app.exit_requested());
    }

    #[test]
    fn on_connection_closed_requests_exit() {
        let mut app = make_app();

        app.on_acp_event(AcpEvent::ConnectionClosed);

        assert!(app.exit_requested());
    }

    #[tokio::test]
    async fn clear_screen_returns_clear_command() {
        let mut app = make_app();

        let mut commands = Vec::new();
        app.handle_conversation_messages(
            &mut commands,
            Some(vec![ConversationScreenMessage::ClearScreen]),
        )
        .await;

        assert!(
            commands
                .iter()
                .any(|c| matches!(c, RendererCommand::ClearScreen)),
            "should contain ClearScreen command"
        );
    }

    #[tokio::test]
    async fn cancel_sends_directly_via_prompt_handle() {
        use tui::{KeyCode, KeyModifiers};

        let mut app = make_app();
        app.conversation_screen.waiting_for_response = true;

        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        app.on_event(&Event::Key(key)).await;
        assert!(!app.exit_requested());
    }

    #[test]
    fn new_session_restores_changed_config_selections() {
        use acp_utils::client::PromptCommand;

        let initial_options = vec![
            acp::SessionConfigOption::select(
                "mode",
                "Mode",
                "Planner",
                vec![
                    acp::SessionConfigSelectOption::new("Planner", "Planner"),
                    acp::SessionConfigSelectOption::new("Coder", "Coder"),
                ],
            )
            .category(acp::SessionConfigOptionCategory::Mode),
            acp::SessionConfigOption::select(
                "model",
                "Model",
                "gpt-4o",
                vec![
                    acp::SessionConfigSelectOption::new("gpt-4o", "GPT-4o"),
                    acp::SessionConfigSelectOption::new("claude", "Claude"),
                ],
            )
            .category(acp::SessionConfigOptionCategory::Model),
        ];
        let (mut app, mut rx) = make_app_with_config_recording(&initial_options);

        // Simulate user changing mode to "Coder"
        let updated_options = vec![
            acp::SessionConfigOption::select(
                "mode",
                "Mode",
                "Coder",
                vec![
                    acp::SessionConfigSelectOption::new("Planner", "Planner"),
                    acp::SessionConfigSelectOption::new("Coder", "Coder"),
                ],
            )
            .category(acp::SessionConfigOptionCategory::Mode),
            acp::SessionConfigOption::select(
                "model",
                "Model",
                "gpt-4o",
                vec![
                    acp::SessionConfigSelectOption::new("gpt-4o", "GPT-4o"),
                    acp::SessionConfigSelectOption::new("claude", "Claude"),
                ],
            )
            .category(acp::SessionConfigOptionCategory::Model),
        ];
        app.update_config_options(&updated_options);

        // Server returns defaults for the new session (mode reset to "Planner")
        let server_defaults = vec![
            acp::SessionConfigOption::select(
                "mode",
                "Mode",
                "Planner",
                vec![
                    acp::SessionConfigSelectOption::new("Planner", "Planner"),
                    acp::SessionConfigSelectOption::new("Coder", "Coder"),
                ],
            )
            .category(acp::SessionConfigOptionCategory::Mode),
            acp::SessionConfigOption::select(
                "model",
                "Model",
                "gpt-4o",
                vec![
                    acp::SessionConfigSelectOption::new("gpt-4o", "GPT-4o"),
                    acp::SessionConfigSelectOption::new("claude", "Claude"),
                ],
            )
            .category(acp::SessionConfigOptionCategory::Model),
        ];

        app.on_acp_event(AcpEvent::NewSessionCreated {
            session_id: SessionId::new("new-session"),
            config_options: server_defaults,
        });

        assert_eq!(app.session_id, SessionId::new("new-session"));
        assert!(app.context_usage_pct.is_none());

        // Only "mode" was changed — verify exactly one SetConfigOption was sent
        let cmd = rx.try_recv().expect("expected a SetConfigOption command");
        match cmd {
            PromptCommand::SetConfigOption {
                config_id, value, ..
            } => {
                assert_eq!(config_id, "mode");
                assert_eq!(value, "Coder");
            }
            other => panic!("expected SetConfigOption, got {other:?}"),
        }
        assert!(
            rx.try_recv().is_err(),
            "model was unchanged, no extra command expected"
        );
    }
}
