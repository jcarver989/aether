use acp_utils::config_option_id::ConfigOptionId;
use acp_utils::notifications::McpRequest;
use agent_client_protocol::{
    self as acp, Agent, AgentCapabilities, AuthMethod, AuthenticateRequest, AuthenticateResponse,
    AvailableCommandsUpdate, ConfigOptionUpdate, Implementation, InitializeRequest,
    InitializeResponse, LoadSessionRequest, LoadSessionResponse, NewSessionRequest,
    NewSessionResponse, PromptCapabilities, PromptResponse, ProtocolVersion, SessionId,
    SessionNotification, SessionUpdate, SetSessionConfigOptionRequest,
    SetSessionConfigOptionResponse, SetSessionModeRequest, SetSessionModeResponse,
};
use llm::ReasoningEffort;
use llm::catalog::{self, LlmModel};
use llm::oauth::OAuthCredentialStore;
use llm::parser::ModelProviderParser;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::spawn;
use tokio::sync::oneshot;
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

use super::config_setting::ConfigSetting;
use super::mappers::map_acp_mcp_servers;
use super::model_config::{
    build_config_options, effective_model, mode_name_for_state, model_exists, pick_default_model,
    resolve_mode, validated_modes,
};
use super::relay::{RelayHandle, SessionCommand, spawn_relay};
use super::session::Session;
use super::settings::{AetherCliSettings, load_or_create_settings};
use acp_utils::content::map_content_blocks_to_text;
use acp_utils::server::AcpActorHandle;

/// Mutable per-session config state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct SessionConfigState {
    active_model: String,
    pending_model: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
    selected_mode: Option<String>,
}

impl SessionConfigState {
    fn new(active_model: String) -> Self {
        Self {
            active_model,
            pending_model: None,
            reasoning_effort: None,
            selected_mode: None,
        }
    }

    fn apply_config_change(
        &mut self,
        settings: &AetherCliSettings,
        available: &[LlmModel],
        setting: &ConfigSetting,
    ) -> Result<(), acp::Error> {
        match setting {
            ConfigSetting::Mode(value) => {
                let Some((mode_model, mode_reasoning_effort)) =
                    resolve_mode(settings, available, value)
                else {
                    error!("Unknown or invalid mode: {}", value);
                    return Err(acp::Error::invalid_params());
                };

                self.pending_model = (self.active_model != mode_model).then_some(mode_model);
                self.reasoning_effort = mode_reasoning_effort;
                self.selected_mode = Some(value.clone());
            }
            ConfigSetting::Model(value) => {
                if !model_exists(available, value) {
                    error!("Unknown model in set_session_config_option: {}", value);
                    return Err(acp::Error::invalid_params());
                }
                self.pending_model = (self.active_model != *value).then_some(value.clone());
            }
            ConfigSetting::ReasoningEffort(effort) => {
                self.reasoning_effort = *effort;
            }
        }

        let effective = effective_model(&self.active_model, self.pending_model.as_deref());
        if setting.config_id() == ConfigOptionId::Model {
            self.selected_mode =
                mode_name_for_state(settings, available, effective, self.reasoning_effort);
        }

        Ok(())
    }
}

/// Per-session state including active and staged model selections.
struct SessionState {
    relay_tx: mpsc::Sender<SessionCommand>,
    mcp_request_tx: mpsc::Sender<McpRequest>,
    #[allow(dead_code)]
    _relay_handle: JoinHandle<()>,
    config: SessionConfigState,
}

/// Manages ACP sessions, each session has its own agent and state
pub struct SessionManager {
    system_prompt: Option<String>,
    settings: AetherCliSettings,
    sessions: Arc<Mutex<HashMap<String, SessionState>>>,
    next_session_id: Arc<Mutex<u64>>,
    actor_handle: AcpActorHandle,
}

impl SessionManager {
    pub fn new(system_prompt: Option<String>, actor_handle: AcpActorHandle) -> Self {
        info!("Creating SessionManager");
        Self {
            system_prompt,
            settings: load_or_create_settings(),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            next_session_id: Arc::new(Mutex::new(0)),
            actor_handle,
        }
    }

    async fn generate_session_id(&self) -> String {
        let mut id = self.next_session_id.lock().await;
        let session_id = format!("session-{}", *id);
        *id += 1;
        session_id
    }
}

/// Resolve MCP config path from the session's CWD.
/// Returns Some if `cwd/mcp.json` exists.
fn resolve_mcp_config(cwd: &Path) -> Option<PathBuf> {
    let path = cwd.join("mcp.json");
    path.exists().then_some(path)
}

/// Build auth methods for OAuth providers that lack credentials.
fn build_auth_methods() -> Vec<AuthMethod> {
    let credential_ids = OAuthCredentialStore::credential_ids_sync();
    let mut seen = HashSet::new();
    LlmModel::all()
        .iter()
        .filter_map(LlmModel::oauth_provider_id)
        .filter(|id| !credential_ids.contains(*id) && seen.insert(*id))
        .map(|id| {
            let display = LlmModel::all()
                .iter()
                .find(|m| m.oauth_provider_id() == Some(id))
                .map_or(id, |m| m.provider_display_name());
            AuthMethod::new(id, display)
        })
        .collect()
}

fn select_initial_mode(
    settings: &AetherCliSettings,
    available: &[LlmModel],
) -> (Option<String>, Option<(String, Option<ReasoningEffort>)>) {
    validated_modes(settings, available)
        .into_iter()
        .next()
        .map_or((None, None), |mode| {
            (Some(mode.name), Some((mode.model, mode.reasoning_effort)))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::config_setting::ConfigSetting;
    use crate::acp::settings::Mode;

    fn available_models() -> Vec<LlmModel> {
        vec![
            "anthropic:claude-sonnet-4-5".parse().expect("valid model"),
            "anthropic:claude-opus-4-6".parse().expect("valid model"),
            "deepseek:deepseek-chat".parse().expect("valid model"),
        ]
    }

    fn settings_with_modes() -> AetherCliSettings {
        let mut settings = AetherCliSettings::default();
        settings.modes.insert(
            "Planner".to_string(),
            Mode {
                model: "anthropic:claude-sonnet-4-5".to_string(),
                reasoning_effort: Some("high".to_string()),
            },
        );
        settings.modes.insert(
            "Coder".to_string(),
            Mode {
                model: "deepseek:deepseek-chat".to_string(),
                reasoning_effort: None,
            },
        );
        settings
    }

    fn fake_config_state(active_model: &str) -> SessionConfigState {
        SessionConfigState::new(active_model.to_string())
    }

    #[test]
    fn session_config_state_new_initializes_defaults() {
        let state = SessionConfigState::new("deepseek:deepseek-chat".to_string());

        assert_eq!(state.active_model, "deepseek:deepseek-chat");
        assert_eq!(state.pending_model, None);
        assert_eq!(state.reasoning_effort, None);
        assert_eq!(state.selected_mode, None);
    }

    #[test]
    fn mode_selection_updates_pending_model_and_reasoning() {
        let settings = settings_with_modes();
        let available = available_models();
        let mut state = fake_config_state("deepseek:deepseek-chat");
        let setting = ConfigSetting::Mode("Planner".to_string());

        let result = state.apply_config_change(&settings, &available, &setting);

        assert!(result.is_ok());
        assert_eq!(
            state.pending_model.as_deref(),
            Some("anthropic:claude-sonnet-4-5")
        );
        assert_eq!(state.reasoning_effort, Some(ReasoningEffort::High));
        assert_eq!(state.selected_mode.as_deref(), Some("Planner"));
    }

    #[test]
    fn unknown_mode_is_rejected() {
        let settings = settings_with_modes();
        let available = available_models();
        let mut state = fake_config_state("deepseek:deepseek-chat");
        let setting = ConfigSetting::Mode("Unknown".to_string());

        let result = state.apply_config_change(&settings, &available, &setting);

        assert!(result.is_err());
    }

    #[test]
    fn reasoning_effort_change_preserves_selected_mode() {
        let settings = settings_with_modes();
        let available = available_models();
        let mut state = fake_config_state("anthropic:claude-sonnet-4-5");
        state.reasoning_effort = Some(ReasoningEffort::High);
        state.selected_mode = Some("Planner".to_string());

        let setting = ConfigSetting::ReasoningEffort(Some(ReasoningEffort::Low));
        let result = state.apply_config_change(&settings, &available, &setting);

        assert!(result.is_ok());
        assert_eq!(state.reasoning_effort, Some(ReasoningEffort::Low));
        assert_eq!(state.selected_mode.as_deref(), Some("Planner"));
    }

    #[test]
    fn manual_model_change_clears_mode_selection_when_no_tuple_match() {
        let settings = settings_with_modes();
        let available = available_models();
        let mut state = fake_config_state("anthropic:claude-sonnet-4-5");
        state.reasoning_effort = Some(ReasoningEffort::Medium);
        state.selected_mode = Some("Planner".to_string());
        let setting = ConfigSetting::Model("deepseek:deepseek-chat".to_string());

        let result = state.apply_config_change(&settings, &available, &setting);

        assert!(result.is_ok());
        assert_eq!(
            state.pending_model.as_deref(),
            Some("deepseek:deepseek-chat")
        );
        assert!(state.selected_mode.is_none());
    }
}

#[async_trait::async_trait(?Send)]
impl Agent for SessionManager {
    async fn initialize(&self, args: InitializeRequest) -> Result<InitializeResponse, acp::Error> {
        info!("Received initialize request: {:?}", args);
        let auth_methods = build_auth_methods();
        Ok(InitializeResponse::new(ProtocolVersion::V1)
            .agent_info(Implementation::new("Aether", "0.1.0"))
            .agent_capabilities(
                AgentCapabilities::new()
                    .load_session(false)
                    .prompt_capabilities(
                        PromptCapabilities::new()
                            .embedded_context(true)
                            .image(false)
                            .audio(false),
                    ),
            )
            .auth_methods(auth_methods))
    }

    async fn authenticate(
        &self,
        args: AuthenticateRequest,
    ) -> Result<AuthenticateResponse, acp::Error> {
        info!("Received authenticate request: {:?}", args);
        let method_id = args.method_id.0.as_ref();
        match method_id {
            "codex" => {
                llm::perform_codex_oauth_flow().await.map_err(|e| {
                    error!("OAuth flow failed for {method_id}: {e}");
                    acp::Error::internal_error()
                })?;
            }
            _ => return Err(acp::Error::invalid_params()),
        }

        // Broadcast updated config options to all active sessions
        let available = catalog::available_models();
        let sessions = self.sessions.lock().await;
        for (id, state) in sessions.iter() {
            let model = effective_model(
                &state.config.active_model,
                state.config.pending_model.as_deref(),
            );
            let options = build_config_options(
                &self.settings,
                &available,
                state.config.selected_mode.as_deref(),
                model,
                state.config.reasoning_effort,
            );
            let notification = SessionNotification::new(
                SessionId::new(id.clone()),
                SessionUpdate::ConfigOptionUpdate(ConfigOptionUpdate::new(options)),
            );
            let _ = self
                .actor_handle
                .send_session_notification(notification)
                .await;
        }

        Ok(AuthenticateResponse::default())
    }

    async fn new_session(&self, args: NewSessionRequest) -> Result<NewSessionResponse, acp::Error> {
        info!("Creating new session with cwd: {:?}", args.cwd);
        let session_id = self.generate_session_id().await;
        let acp_session_id = acp::SessionId::new(session_id.clone());

        let available = catalog::available_models();
        let default_model = pick_default_model(&available).ok_or_else(|| {
            error!("No models available — set an API key env var (e.g. ANTHROPIC_API_KEY)");
            acp::Error::internal_error()
        })?;

        let mut initial_model = default_model.to_string();
        let mut initial_reasoning_effort = None;
        let mut initial_selected_mode = None;

        if !self.settings.modes.is_empty() {
            let (mode_name, mode_config) = select_initial_mode(&self.settings, &available);
            if let Some((model, reasoning_effort)) = mode_config {
                initial_model = model;
                initial_reasoning_effort = reasoning_effort;
                initial_selected_mode = mode_name;
            }
        }

        let model_str = initial_model;

        let parser = ModelProviderParser::default();
        let (llm, _) = parser.parse(&model_str).map_err(|e| {
            error!("Failed to create provider for '{}': {}", model_str, e);
            acp::Error::internal_error()
        })?;

        let mcp_config_path = resolve_mcp_config(&args.cwd);

        let session = Session::new(
            llm,
            self.system_prompt.clone(),
            mcp_config_path.clone(),
            args.cwd.clone(),
            map_acp_mcp_servers(args.mcp_servers),
        )
        .await
        .map_err(|e| {
            error!("Failed to create session: {}", e);
            acp::Error::internal_error()
        })?;

        let available_commands = session.list_available_commands().await.map_err(|e| {
            error!("Failed to list available commands: {}", e);
            acp::Error::internal_error()
        })?;

        let RelayHandle {
            cmd_tx,
            mcp_request_tx,
            join_handle,
        } = spawn_relay(session, self.actor_handle.clone(), acp_session_id.clone());

        let mut config = SessionConfigState::new(model_str.clone());
        config.reasoning_effort = initial_reasoning_effort;
        config.selected_mode.clone_from(&initial_selected_mode);

        let state = SessionState {
            relay_tx: cmd_tx,
            mcp_request_tx,
            _relay_handle: join_handle,
            config,
        };

        let mut sessions = self.sessions.lock().await;
        sessions.insert(session_id.clone(), state);

        info!("Session {} created successfully", session_id);

        let config_options = build_config_options(
            &self.settings,
            &available,
            initial_selected_mode.as_deref(),
            &model_str,
            initial_reasoning_effort,
        );
        let response =
            NewSessionResponse::new(acp_session_id.clone()).config_options(config_options);

        // Send available commands update notification asynchronously (don't await)
        // This allows the response to be sent first, then the notification follows
        if !available_commands.is_empty() {
            let command_count = available_commands.len();
            let notification = SessionNotification::new(
                acp_session_id,
                SessionUpdate::AvailableCommandsUpdate(AvailableCommandsUpdate::new(
                    available_commands,
                )),
            );

            let actor_handle = self.actor_handle.clone();
            let session_id_log = session_id.clone();
            spawn(async move {
                if let Err(e) = actor_handle.send_session_notification(notification).await {
                    error!("Failed to send available commands notification: {:?}", e);
                } else {
                    info!(
                        "Sent available commands update for session {} ({} commands)",
                        session_id_log, command_count
                    );
                }
            });
        }

        Ok(response)
    }

    async fn load_session(
        &self,
        args: LoadSessionRequest,
    ) -> Result<LoadSessionResponse, acp::Error> {
        info!("Received load_session request: {:?}", args);
        // Not supported yet
        Err(acp::Error::method_not_found())
    }

    async fn prompt(&self, args: acp::PromptRequest) -> Result<acp::PromptResponse, acp::Error> {
        info!("Received prompt for session: {:?}", args.session_id);
        let session_id_str = args.session_id.0.to_string();

        let (relay_tx, prompt_text, switch_model, reasoning_effort) = {
            let mut sessions = self.sessions.lock().await;
            let state = sessions.get_mut(&session_id_str).ok_or_else(|| {
                error!("Session not found: {}", session_id_str);
                acp::Error::invalid_params()
            })?;

            let prompt_text = map_content_blocks_to_text(args.prompt);
            debug!("Prompt text: {}", prompt_text);

            let switch_model = state.config.pending_model.take().and_then(|pending| {
                if pending == state.config.active_model {
                    None
                } else {
                    state.config.active_model.clone_from(&pending);
                    Some(pending)
                }
            });

            let reasoning_effort = state.config.reasoning_effort;

            (
                state.relay_tx.clone(),
                prompt_text,
                switch_model,
                reasoning_effort,
            )
        };

        let (result_tx, result_rx) = oneshot::channel();
        relay_tx
            .send(SessionCommand::Prompt {
                text: prompt_text,
                switch_model,
                reasoning_effort,
                result_tx,
            })
            .await
            .map_err(|_| {
                error!("Relay channel closed for session {}", session_id_str);
                acp::Error::internal_error()
            })?;

        let stop_reason = result_rx
            .await
            .map_err(|_| {
                error!(
                    "Relay dropped result channel for session {}",
                    session_id_str
                );
                acp::Error::internal_error()
            })?
            .map_err(|e| {
                error!("Relay error for session {}: {}", session_id_str, e);
                acp::Error::internal_error()
            })?;

        info!("Prompt completed with stop reason: {:?}", stop_reason);
        Ok(PromptResponse::new(stop_reason))
    }

    async fn cancel(&self, args: acp::CancelNotification) -> Result<(), acp::Error> {
        info!("Received cancel for session: {:?}", args.session_id);
        let session_id_str = args.session_id.0.to_string();
        let relay_tx = {
            let sessions = self.sessions.lock().await;
            sessions
                .get(&session_id_str)
                .ok_or_else(|| {
                    error!("Session not found for cancel: {}", session_id_str);
                    acp::Error::invalid_params()
                })?
                .relay_tx
                .clone()
        };

        relay_tx.send(SessionCommand::Cancel).await.map_err(|_| {
            error!("Relay channel closed for cancel: {}", session_id_str);
            acp::Error::internal_error()
        })?;

        Ok(())
    }

    async fn set_session_config_option(
        &self,
        args: SetSessionConfigOptionRequest,
    ) -> Result<SetSessionConfigOptionResponse, acp::Error> {
        let session_id_str = args.session_id.0.to_string();
        let config_id = args.config_id.0.to_string();
        let value = args.value.0.to_string();

        info!(
            "set_session_config_option: session={}, config={}, value={}",
            session_id_str, config_id, value
        );

        let setting = ConfigSetting::parse(&config_id, &value).map_err(|e| {
            error!("{e}");
            acp::Error::invalid_params()
        })?;

        let available = catalog::available_models();

        let mut sessions = self.sessions.lock().await;
        let state = sessions.get_mut(&session_id_str).ok_or_else(|| {
            error!("Session not found: {}", session_id_str);
            acp::Error::invalid_params()
        })?;

        state
            .config
            .apply_config_change(&self.settings, &available, &setting)?;

        let effective_model = effective_model(
            &state.config.active_model,
            state.config.pending_model.as_deref(),
        );
        let options = build_config_options(
            &self.settings,
            &available,
            state.config.selected_mode.as_deref(),
            effective_model,
            state.config.reasoning_effort,
        );
        Ok(SetSessionConfigOptionResponse::new(options))
    }

    async fn set_session_mode(
        &self,
        args: SetSessionModeRequest,
    ) -> Result<SetSessionModeResponse, acp::Error> {
        info!("Received set_session_mode request: {:?}", args);
        Err(acp::Error::method_not_found())
    }

    async fn ext_method(&self, args: acp::ExtRequest) -> Result<acp::ExtResponse, acp::Error> {
        info!(
            "Received extension method: {}, params: {:?}",
            args.method, args.params
        );
        let null_value: Arc<serde_json::value::RawValue> =
            serde_json::from_str("null").expect("null is valid JSON");
        Ok(null_value.into())
    }

    async fn ext_notification(&self, args: acp::ExtNotification) -> Result<(), acp::Error> {
        info!(
            "Received extension notification: {}, params: {:?}",
            args.method, args.params
        );

        if let Ok(msg) = McpRequest::try_from(&args) {
            match msg {
                McpRequest::Authenticate {
                    session_id,
                    server_name,
                } => {
                    let mcp_request_tx = {
                        let sessions = self.sessions.lock().await;
                        let session = sessions.get(&session_id);
                        session
                            .ok_or_else(|| {
                                error!(
                                    "Session not found for authenticate_mcp_server: {}",
                                    session_id
                                );
                                acp::Error::invalid_params()
                            })?
                            .mcp_request_tx
                            .clone()
                    };

                    mcp_request_tx
                        .send(McpRequest::Authenticate {
                            session_id,
                            server_name,
                        })
                        .await
                        .map_err(|_| {
                            error!("MCP request channel closed for session");
                            acp::Error::internal_error()
                        })?;
                }
            }
        }

        Ok(())
    }
}
