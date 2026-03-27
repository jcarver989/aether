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
use acp_utils::config_meta::SelectOptionMeta;
use acp_utils::config_option_id::ConfigOptionId;
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
    prompt_capabilities: acp::PromptCapabilities,
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
        prompt_capabilities: acp::PromptCapabilities,
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
            prompt_capabilities,
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

    pub fn needs_mouse_capture(&self) -> bool {
        self.settings_overlay.is_some() || self.screen_router.is_git_diff()
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
            AcpEvent::PromptDone(stop_reason) => self.on_prompt_done(stop_reason),
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

    async fn submit_prompt(&mut self, user_input: String, attachments: Vec<PromptAttachment>) {
        let outcome = build_attachment_blocks(&attachments).await;
        self.conversation_screen.conversation.push_user_message("");
        self.conversation_screen
            .conversation
            .push_user_message(&user_input);
        for placeholder in &outcome.transcript_placeholders {
            self.conversation_screen
                .conversation
                .push_user_message(placeholder);
        }
        for w in outcome.warnings {
            self.conversation_screen
                .conversation
                .push_user_message(&format!("[wisp] {w}"));
        }

        if let Some(message) = self.media_support_error(&outcome.blocks) {
            self.conversation_screen.reject_local_prompt(&message);
            return;
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
                    self.conversation_screen.waiting_for_response = true;
                    self.submit_prompt(user_input, attachments).await;
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

    fn update_auth_methods(&mut self, auth_methods: Vec<acp::AuthMethod>) {
        self.auth_methods = auth_methods;
        if let Some(ref mut overlay) = self.settings_overlay {
            overlay.update_auth_methods(self.auth_methods.clone());
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
            if server_reset
                && let Err(e) =
                    self.prompt_handle
                        .set_config_option(&self.session_id, id, old_value)
            {
                tracing::warn!(config_id = id, error = %e, "failed to restore config option");
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
                if self.conversation_screen.is_waiting() {
                    return;
                }

                self.conversation_screen.waiting_for_response = true;
                self.submit_prompt(user_input, Vec::new()).await;
                self.screen_router.close_git_diff();
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

    fn on_prompt_done(&mut self, stop_reason: acp::StopReason) {
        self.conversation_screen.on_prompt_done(stop_reason);
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
            AUTH_METHODS_UPDATED_METHOD, AuthMethodsUpdatedParams, CONTEXT_CLEARED_METHOD,
            CONTEXT_USAGE_METHOD, ContextUsageParams, McpNotification, SUB_AGENT_PROGRESS_METHOD,
            SubAgentProgressParams,
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
            AUTH_METHODS_UPDATED_METHOD => {
                if let Ok(params) = AuthMethodsUpdatedParams::try_from(notification) {
                    self.update_auth_methods(params.auth_methods);
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

    fn media_support_error(&self, blocks: &[acp::ContentBlock]) -> Option<String> {
        let requires_image = blocks
            .iter()
            .any(|block| matches!(block, acp::ContentBlock::Image(_)));
        let requires_audio = blocks
            .iter()
            .any(|block| matches!(block, acp::ContentBlock::Audio(_)));

        if !requires_image && !requires_audio {
            return None;
        }

        if requires_image && !self.prompt_capabilities.image {
            return Some("ACP agent does not support image input.".to_string());
        }
        if requires_audio && !self.prompt_capabilities.audio {
            return Some("ACP agent does not support audio input.".to_string());
        }

        let option = self
            .config_options
            .iter()
            .find(|option| option.id.0.as_ref() == ConfigOptionId::Model.as_str())?;
        let acp::SessionConfigKind::Select(select) = &option.kind else {
            return None;
        };

        let values: Vec<_> = select
            .current_value
            .0
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect();

        if values.is_empty() {
            return None;
        }

        let acp::SessionConfigSelectOptions::Ungrouped(options) = &select.options else {
            return None;
        };

        let selected_meta: Vec<_> = values
            .iter()
            .filter_map(|value| {
                options
                    .iter()
                    .find(|option| option.value.0.as_ref() == *value)
                    .map(|option| SelectOptionMeta::from_meta(option.meta.as_ref()))
            })
            .collect();

        if selected_meta.len() != values.len() {
            return Some("Current model selection is missing prompt capability metadata.".into());
        }

        if requires_image && selected_meta.iter().any(|meta| !meta.supports_image) {
            return Some("Current model selection does not support image input.".to_string());
        }
        if requires_audio && selected_meta.iter().any(|meta| !meta.supports_audio) {
            return Some("Current model selection does not support audio input.".to_string());
        }

        None
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
                if self.screen_router.is_git_diff() {
                    for msg in self.screen_router.on_event(event).await.unwrap_or_default() {
                        self.handle_screen_router_message(&mut commands, msg).await;
                    }
                } else if self.settings_overlay.is_some() {
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
            acp::PromptCapabilities::new(),
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
            acp::PromptCapabilities::new(),
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
            acp::PromptCapabilities::new(),
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
            acp::PromptCapabilities::new(),
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
            acp::PromptCapabilities::new(),
            &[],
            vec![],
            PathBuf::from("."),
            AcpPromptHandle::noop(),
        )
    }

    pub fn make_app_with_config_and_capabilities_recording(
        config_options: &[acp::SessionConfigOption],
        prompt_capabilities: acp::PromptCapabilities,
    ) -> (
        App,
        tokio::sync::mpsc::UnboundedReceiver<acp_utils::client::PromptCommand>,
    ) {
        let (handle, rx) = AcpPromptHandle::recording();
        let app = App::new(
            SessionId::new("test"),
            "test-agent".to_string(),
            prompt_capabilities,
            config_options,
            vec![],
            PathBuf::from("."),
            handle,
        );
        (app, rx)
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
    use tui::testing::render_component;
    use tui::{Frame, KeyCode, KeyModifiers, Renderer, Theme, ViewContext};

    fn make_renderer() -> Renderer<Vec<u8>> {
        Renderer::new(Vec::new(), Theme::default(), (80, 24))
    }

    fn render_app(renderer: &mut Renderer<Vec<u8>>, app: &mut App, context: &ViewContext) -> Frame {
        renderer.render_frame(|ctx| app.render(ctx)).unwrap();
        app.render(context)
    }

    fn frame_contains(output: &Frame, text: &str) -> bool {
        output
            .lines()
            .iter()
            .any(|line| line.plain_text().contains(text))
    }

    async fn send_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
        app.on_event(&Event::Key(KeyEvent::new(code, modifiers)))
            .await;
    }

    fn setup_themes_dir(files: &[&str]) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let themes_dir = temp_dir.path().join("themes");
        fs::create_dir_all(&themes_dir).unwrap();
        for f in files {
            fs::write(themes_dir.join(f), "x").unwrap();
        }
        temp_dir
    }

    fn make_plan_entry(name: &str, status: acp::PlanEntryStatus) -> acp::PlanEntry {
        acp::PlanEntry::new(name, acp::PlanEntryPriority::Medium, status)
    }

    fn mode_model_options(
        mode_val: impl Into<String>,
        model_val: impl Into<String>,
    ) -> Vec<acp::SessionConfigOption> {
        vec![
            acp::SessionConfigOption::select(
                "mode",
                "Mode",
                mode_val.into(),
                vec![
                    acp::SessionConfigSelectOption::new("Planner", "Planner"),
                    acp::SessionConfigSelectOption::new("Coder", "Coder"),
                ],
            )
            .category(acp::SessionConfigOptionCategory::Mode),
            acp::SessionConfigOption::select(
                "model",
                "Model",
                model_val.into(),
                vec![
                    acp::SessionConfigSelectOption::new("gpt-4o", "GPT-4o"),
                    acp::SessionConfigSelectOption::new("claude", "Claude"),
                ],
            )
            .category(acp::SessionConfigOptionCategory::Model),
        ]
    }

    fn image_model_options() -> Vec<acp::SessionConfigOption> {
        vec![
            acp::SessionConfigOption::select(
                "model",
                "Model",
                "anthropic:claude-sonnet-4-5",
                vec![
                    acp::SessionConfigSelectOption::new(
                        "anthropic:claude-sonnet-4-5",
                        "Claude Sonnet",
                    )
                    .meta(
                        SelectOptionMeta {
                            reasoning_levels: vec![],
                            supports_image: true,
                            supports_audio: false,
                        }
                        .into_meta(),
                    ),
                    acp::SessionConfigSelectOption::new("deepseek:deepseek-chat", "DeepSeek").meta(
                        SelectOptionMeta {
                            reasoning_levels: vec![],
                            supports_image: false,
                            supports_audio: false,
                        }
                        .into_meta(),
                    ),
                ],
            )
            .category(acp::SessionConfigOptionCategory::Model),
        ]
    }

    #[test]
    fn settings_overlay_with_themes() {
        let temp_dir = setup_themes_dir(&["catppuccin.tmTheme"]);
        with_wisp_home(temp_dir.path(), || {
            let mut app = make_app();
            app.open_settings_overlay();
            assert!(app.settings_overlay.is_some());
        });

        let temp_dir = setup_themes_dir(&["catppuccin.tmTheme", "nord.tmTheme"]);
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

        let ctx = ViewContext::new((120, 40));
        assert!(frame_contains(
            &render_app(&mut renderer, &mut app, &ctx),
            "Configuration"
        ));
        app.settings_overlay = None;
        assert!(!frame_contains(
            &render_app(&mut renderer, &mut app, &ctx),
            "Configuration"
        ));
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
            vec![make_plan_entry("1", acp::PlanEntryStatus::Completed)],
            Instant::now()
                .checked_sub(grace_period + Duration::from_millis(1))
                .unwrap(),
        );
        app.conversation_screen.plan_tracker.on_tick(Instant::now());

        let output = render_app(&mut renderer, &mut app, &ViewContext::new((120, 40)));
        assert!(!frame_contains(&output, "Plan"));
    }

    #[test]
    fn plan_version_increments_on_replace_and_clear() {
        let mut app = make_app();
        let v0 = app.conversation_screen.plan_tracker.version();

        app.conversation_screen.plan_tracker.replace(
            vec![make_plan_entry("Task A", acp::PlanEntryStatus::Pending)],
            Instant::now(),
        );
        let v1 = app.conversation_screen.plan_tracker.version();
        assert!(v1 > v0, "replace should increment version");

        app.conversation_screen.plan_tracker.clear();
        assert!(
            app.conversation_screen.plan_tracker.version() > v1,
            "clear should increment version"
        );
    }

    #[test]
    fn sessions_listed_filters_out_current_session() {
        let mut app = make_app_with_session_id("current-session");
        app.on_acp_event(AcpEvent::SessionsListed {
            sessions: vec![
                acp::SessionInfo::new("other-session-1", PathBuf::from("/project"))
                    .title("First other session".to_string()),
                acp::SessionInfo::new("current-session", PathBuf::from("/project"))
                    .title("Current session title".to_string()),
                acp::SessionInfo::new("other-session-2", PathBuf::from("/other"))
                    .title("Second other session".to_string()),
            ],
        });

        let picker = match &mut app.conversation_screen.active_modal {
            Some(crate::components::conversation_screen::Modal::SessionPicker(p)) => p,
            _ => panic!("expected session picker modal"),
        };
        let lines = render_component(|ctx| picker.render(ctx), 60, 10).get_lines();

        let has = |text: &str| lines.iter().any(|l| l.contains(text));
        assert!(
            !has("Current session title"),
            "current session should be filtered out"
        );
        assert!(
            has("First other session"),
            "first other session should be present"
        );
        assert!(
            has("Second other session"),
            "second other session should be present"
        );
    }

    #[tokio::test]
    async fn custom_exit_keybinding_triggers_exit() {
        use crate::keybindings::KeyBinding;
        let mut app = make_app();
        app.keybindings.exit = KeyBinding::new(KeyCode::Char('q'), KeyModifiers::CONTROL);

        send_key(&mut app, KeyCode::Char('c'), KeyModifiers::CONTROL).await;
        assert!(
            !app.exit_requested(),
            "default Ctrl+C should no longer exit"
        );

        send_key(&mut app, KeyCode::Char('q'), KeyModifiers::CONTROL).await;
        assert!(app.exit_requested(), "custom Ctrl+Q should exit");
    }

    #[tokio::test]
    async fn ctrl_g_toggles_git_diff_viewer() {
        let mut app = make_app();

        send_key(&mut app, KeyCode::Char('g'), KeyModifiers::CONTROL).await;
        assert!(app.screen_router.is_git_diff(), "should open git diff");

        send_key(&mut app, KeyCode::Char('g'), KeyModifiers::CONTROL).await;
        assert!(!app.screen_router.is_git_diff(), "should close git diff");
    }

    #[tokio::test]
    async fn needs_mouse_capture_in_git_diff() {
        let mut app = make_app();
        assert!(!app.needs_mouse_capture());

        send_key(&mut app, KeyCode::Char('g'), KeyModifiers::CONTROL).await;
        assert!(app.needs_mouse_capture());

        send_key(&mut app, KeyCode::Char('g'), KeyModifiers::CONTROL).await;
        assert!(!app.needs_mouse_capture());
    }

    #[tokio::test]
    async fn ctrl_g_blocked_during_elicitation() {
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

        send_key(&mut app, KeyCode::Char('g'), KeyModifiers::CONTROL).await;
        assert!(
            !app.screen_router.is_git_diff(),
            "git diff should not open during elicitation"
        );
    }

    #[tokio::test]
    async fn esc_in_diff_mode_does_not_cancel() {
        let mut app = make_app();
        app.conversation_screen.waiting_for_response = true;
        app.screen_router.enter_git_diff_for_test();

        send_key(&mut app, KeyCode::Esc, KeyModifiers::NONE).await;

        assert!(!app.exit_requested());
        assert!(
            app.conversation_screen.waiting_for_response,
            "Esc should NOT cancel a running prompt while git diff mode is active"
        );
    }

    #[tokio::test]
    async fn git_diff_submit_sends_prompt_and_closes_diff_when_idle() {
        use acp_utils::client::PromptCommand;

        let (mut app, mut rx) = make_app_with_config_recording(&[]);
        app.screen_router.enter_git_diff_for_test();

        let mut commands = Vec::new();
        app.handle_screen_router_message(
            &mut commands,
            ScreenRouterMessage::SendPrompt {
                user_input: "Looks good".to_string(),
            },
        )
        .await;

        assert!(
            !app.screen_router.is_git_diff(),
            "successful submit should exit git diff mode"
        );
        assert!(
            app.conversation_screen.waiting_for_response,
            "submit should transition into waiting state"
        );

        let cmd = rx.try_recv().expect("expected Prompt command to be sent");
        match cmd {
            PromptCommand::Prompt { text, .. } => {
                assert!(text.contains("Looks good"));
            }
            other => panic!("expected Prompt command, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn git_diff_submit_while_waiting_is_ignored_and_keeps_diff_open() {
        let (mut app, mut rx) = make_app_with_config_recording(&[]);
        app.conversation_screen.waiting_for_response = true;
        app.screen_router.enter_git_diff_for_test();

        let mut commands = Vec::new();
        app.handle_screen_router_message(
            &mut commands,
            ScreenRouterMessage::SendPrompt {
                user_input: "Needs follow-up".to_string(),
            },
        )
        .await;

        assert!(
            app.screen_router.is_git_diff(),
            "blocked submit should keep git diff mode open"
        );
        assert!(
            rx.try_recv().is_err(),
            "no prompt should be sent while waiting"
        );
    }

    #[tokio::test]
    async fn mouse_scroll_ignored_in_conversation_mode() {
        use tui::{MouseEvent, MouseEventKind};
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
        let mut commands = Vec::new();
        app.handle_conversation_messages(
            &mut commands,
            Some(vec![ConversationScreenMessage::SendPrompt {
                user_input: "hello".to_string(),
                attachments: vec![],
            }]),
        )
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

    #[tokio::test]
    async fn unsupported_media_is_blocked_locally() {
        let (mut app, mut rx) = make_app_with_config_and_capabilities_recording(
            &image_model_options(),
            acp::PromptCapabilities::new().image(true).audio(false),
        );
        let mut commands = Vec::new();
        let temp = tempfile::tempdir().unwrap();
        let audio_path = temp.path().join("clip.wav");
        std::fs::write(&audio_path, b"fake wav").unwrap();

        app.handle_conversation_messages(
            &mut commands,
            Some(vec![ConversationScreenMessage::SendPrompt {
                user_input: "listen".to_string(),
                attachments: vec![PromptAttachment {
                    path: audio_path,
                    display_name: "clip.wav".to_string(),
                }],
            }]),
        )
        .await;

        assert!(rx.try_recv().is_err(), "prompt should be blocked locally");
        assert!(!app.conversation_screen.waiting_for_response);
        let messages: Vec<_> = app
            .conversation_screen
            .conversation
            .segments()
            .filter_map(|segment| match segment {
                crate::components::conversation_window::SegmentContent::UserMessage(text) => {
                    Some(text.clone())
                }
                _ => None,
            })
            .collect();
        assert!(messages.iter().any(|text| text == "listen"));
        assert!(
            messages
                .iter()
                .any(|text| text == "[audio attachment: clip.wav]")
        );
        assert!(messages.iter().any(|text| {
            text == "[wisp] ACP agent does not support audio input."
                || text == "[wisp] Current model selection does not support audio input."
        }));
    }

    #[test]
    fn replayed_media_user_chunks_render_placeholders() {
        use crate::components::conversation_window::SegmentContent;
        let mut app = make_app();

        app.on_session_update(&acp::SessionUpdate::UserMessageChunk(
            acp::ContentChunk::new(acp::ContentBlock::Image(acp::ImageContent::new(
                "aW1n",
                "image/png",
            ))),
        ));
        app.on_session_update(&acp::SessionUpdate::UserMessageChunk(
            acp::ContentChunk::new(acp::ContentBlock::Audio(acp::AudioContent::new(
                "YXVkaW8=",
                "audio/wav",
            ))),
        ));

        let segments: Vec<_> = app.conversation_screen.conversation.segments().collect();
        assert!(matches!(
            segments[0],
            SegmentContent::UserMessage(text) if text == "[image attachment]"
        ));
        assert!(matches!(
            segments[1],
            SegmentContent::UserMessage(text) if text == "[audio attachment]"
        ));
    }

    #[test]
    fn prompt_composer_open_settings() {
        let mut app = make_app();
        let mut commands = Vec::new();
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(app.handle_conversation_messages(
                &mut commands,
                Some(vec![ConversationScreenMessage::OpenSettings]),
            ));
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
    async fn tick_advances_spinner_animations() {
        let mut app = make_app();
        let tool_call = acp::ToolCall::new("tool-1".to_string(), "test_tool");
        app.conversation_screen
            .tool_call_statuses
            .on_tool_call(&tool_call);
        app.conversation_screen
            .progress_indicator
            .update(0, 1, true);

        let ctx = ViewContext::new((80, 24));
        let tool_before = app
            .conversation_screen
            .tool_call_statuses
            .render_tool("tool-1", &ctx);
        let prog_before = app.conversation_screen.progress_indicator.render(&ctx);

        app.on_event(&Event::Tick).await;

        let tool_after = app
            .conversation_screen
            .tool_call_statuses
            .render_tool("tool-1", &ctx);
        let prog_after = app.conversation_screen.progress_indicator.render(&ctx);

        assert_ne!(
            tool_before[0].plain_text(),
            tool_after[0].plain_text(),
            "tick should advance tool spinner"
        );
        assert_ne!(
            prog_before[0].plain_text(),
            prog_after[0].plain_text(),
            "tick should advance progress spinner"
        );
    }

    #[test]
    fn on_prompt_error_clears_waiting_state() {
        let mut app = make_app();
        app.conversation_screen.waiting_for_response = true;
        app.conversation_screen
            .on_prompt_error(&acp::Error::internal_error());
        assert!(!app.conversation_screen.waiting_for_response);
        assert!(!app.exit_requested());
    }

    #[test]
    fn auth_events_and_connection_close_exit_behavior() {
        let mut app = make_app_with_auth(vec![acp::AuthMethod::Agent(acp::AuthMethodAgent::new(
            "anthropic",
            "Anthropic",
        ))]);
        app.on_authenticate_complete("anthropic");
        assert!(
            !app.exit_requested(),
            "authenticate_complete should not exit"
        );

        let mut app = make_app();
        app.on_authenticate_failed("anthropic", "bad token");
        assert!(!app.exit_requested(), "authenticate_failed should not exit");

        let mut app = make_app();
        app.on_acp_event(AcpEvent::ConnectionClosed);
        assert!(app.exit_requested(), "connection_closed should exit");
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
        let mut app = make_app();
        app.conversation_screen.waiting_for_response = true;
        send_key(&mut app, KeyCode::Esc, KeyModifiers::NONE).await;
        assert!(!app.exit_requested());
    }

    #[test]
    fn new_session_restores_changed_config_selections() {
        use acp_utils::client::PromptCommand;

        let (mut app, mut rx) =
            make_app_with_config_recording(&mode_model_options("Planner", "gpt-4o"));
        app.update_config_options(&mode_model_options("Coder", "gpt-4o"));

        app.on_acp_event(AcpEvent::NewSessionCreated {
            session_id: SessionId::new("new-session"),
            config_options: mode_model_options("Planner", "gpt-4o"),
        });

        assert_eq!(app.session_id, SessionId::new("new-session"));
        assert!(app.context_usage_pct.is_none());

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
