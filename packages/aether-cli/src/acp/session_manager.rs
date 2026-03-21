use acp_utils::config_option_id::ConfigOptionId;
use acp_utils::notifications::{AuthMethodsUpdatedParams, McpRequest};
use agent_client_protocol::{
    self as acp, Agent, AgentCapabilities, AuthMethod, AuthenticateRequest, AuthenticateResponse,
    AvailableCommandsUpdate, ConfigOptionUpdate, ExtNotification, Implementation,
    InitializeRequest, InitializeResponse, ListSessionsRequest, ListSessionsResponse,
    LoadSessionRequest, LoadSessionResponse, McpCapabilities, NewSessionRequest,
    NewSessionResponse, PromptCapabilities, PromptResponse, ProtocolVersion, SessionId,
    SessionNotification, SessionUpdate, SetSessionConfigOptionRequest,
    SetSessionConfigOptionResponse, SetSessionModeRequest, SetSessionModeResponse,
};
use llm::ReasoningEffort;
use llm::catalog::{self, LlmModel, get_local_models};
use llm::oauth::OAuthCredentialStore;
use llm::types::IsoString;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use tokio::spawn;
use tokio::sync::oneshot;
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

use super::config_setting::ConfigSetting;
use super::mappers::{map_acp_mcp_servers, replay_to_client};
use super::model_config::{
    ValidatedMode, build_config_options_from_modes, effective_model,
    mode_name_for_state_from_modes, model_exists, pick_default_model, resolve_mode_from_modes,
    validated_modes_from_specs,
};
use super::relay::{RelayHandle, SessionCommand, spawn_relay};
use super::session::Session;
use super::session_store::{SessionMeta, SessionStore};
use acp_utils::content::map_content_blocks_to_text;
use acp_utils::server::AcpActorHandle;
use aether_core::context::ext::ContextExt;
use aether_project::{AgentCatalog, load_agent_catalog};
use llm::Context;

struct SessionModeCatalog {
    catalog: AgentCatalog,
    modes: Vec<ValidatedMode>,
}

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
        validated_modes: &[ValidatedMode],
        available: &[LlmModel],
        setting: &ConfigSetting,
    ) -> Result<(), acp::Error> {
        match setting {
            ConfigSetting::Mode(value) => {
                let Some((mode_model, mode_reasoning_effort)) =
                    resolve_mode_from_modes(validated_modes, value)
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
                mode_name_for_state_from_modes(validated_modes, effective, self.reasoning_effort);
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
    modes: Vec<ValidatedMode>,
}

/// Manages ACP sessions, each session has its own agent and state
pub struct SessionManager {
    sessions: Arc<Mutex<HashMap<String, SessionState>>>,
    actor_handle: AcpActorHandle,
    session_store: Arc<SessionStore>,
}

impl SessionManager {
    pub fn new(actor_handle: AcpActorHandle) -> Self {
        info!("Creating SessionManager");
        let session_store = SessionStore::new().map_or_else(
            |e| panic!("Failed to initialize session store: {e}"),
            Arc::new,
        );
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            actor_handle,
            session_store,
        }
    }

    async fn load_mode_catalog(cwd: &Path) -> Result<SessionModeCatalog, acp::Error> {
        let catalog = load_agent_catalog(cwd).map_err(|e| {
            error!("Failed to load agent catalog: {e}");
            acp::Error::invalid_params()
        })?;

        let available = get_local_models().await;
        let specs: Vec<_> = catalog.user_invocable().cloned().collect();
        let modes = validated_modes_from_specs(&specs, &available);

        Ok(SessionModeCatalog { catalog, modes })
    }

    #[allow(clippy::too_many_arguments, clippy::similar_names)]
    async fn register_session(
        &self,
        session: Session,
        session_id: &str,
        acp_session_id: &SessionId,
        model: &str,
        selected_mode: Option<String>,
        reasoning_effort: Option<ReasoningEffort>,
        modes: Vec<ValidatedMode>,
    ) -> Vec<acp::SessionConfigOption> {
        let RelayHandle {
            cmd_tx,
            mcp_request_tx,
            join_handle,
        } = spawn_relay(
            session,
            self.actor_handle.clone(),
            acp_session_id.clone(),
            self.session_store.clone(),
        );

        let mut config = SessionConfigState::new(model.to_string());
        config.reasoning_effort = reasoning_effort;
        config.selected_mode.clone_from(&selected_mode);

        let state = SessionState {
            relay_tx: cmd_tx,
            mcp_request_tx,
            _relay_handle: join_handle,
            config,
            modes: modes.clone(),
        };

        let mut sessions = self.sessions.lock().await;
        sessions.insert(session_id.to_string(), state);

        let available = get_local_models().await;
        let all_models = get_all_models(&available);
        build_config_options_from_modes(
            &modes,
            &available,
            selected_mode.as_deref(),
            model,
            reasoning_effort,
            &all_models,
        )
    }

    fn spawn_available_commands_notification(
        &self,
        available_commands: Vec<acp::AvailableCommand>,
        acp_session_id: SessionId,
        session_id: &str,
    ) {
        if available_commands.is_empty() {
            return;
        }
        let command_count = available_commands.len();
        let notification = SessionNotification::new(
            acp_session_id,
            SessionUpdate::AvailableCommandsUpdate(AvailableCommandsUpdate::new(
                available_commands,
            )),
        );
        let actor_handle = self.actor_handle.clone();
        let session_id_log = session_id.to_string();
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
}

/// Merge catalog `all()` with locally-discovered models for the `all_models`
/// parameter of `build_model_config_option`.
fn get_all_models(discovered: &[LlmModel]) -> Vec<LlmModel> {
    let mut all = LlmModel::all().to_vec();
    for m in discovered {
        if !all.contains(m) {
            all.push(m.clone());
        }
    }
    all
}

fn build_auth_methods() -> Vec<AuthMethod> {
    let mut seen = HashSet::new();
    LlmModel::all()
        .iter()
        .filter_map(LlmModel::oauth_provider_id)
        .filter(|id| seen.insert(*id))
        .map(|id| {
            let display = LlmModel::all()
                .iter()
                .find(|m| m.oauth_provider_id() == Some(id))
                .map_or(id, |m| m.provider_display_name());
            let mut method = acp::AuthMethodAgent::new(id, display);
            if OAuthCredentialStore::has_credential(id) {
                method = method.description("authenticated");
            }
            AuthMethod::Agent(method)
        })
        .collect()
}

fn select_initial_mode(
    validated_modes: &[ValidatedMode],
) -> (Option<String>, Option<(String, Option<ReasoningEffort>)>) {
    validated_modes.first().map_or((None, None), |mode| {
        (
            Some(mode.name.clone()),
            Some((mode.model.clone(), mode.reasoning_effort)),
        )
    })
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::acp::config_setting::ConfigSetting;
    use agent_client_protocol::{InitializeRequest, ProtocolVersion};

    fn available_models() -> Vec<LlmModel> {
        vec![
            "anthropic:claude-sonnet-4-5".parse().expect("valid model"),
            "anthropic:claude-opus-4-6".parse().expect("valid model"),
            "deepseek:deepseek-chat".parse().expect("valid model"),
        ]
    }

    fn validated_modes() -> Vec<ValidatedMode> {
        vec![
            ValidatedMode {
                name: "Planner".to_string(),
                model: "anthropic:claude-sonnet-4-5".to_string(),
                reasoning_effort: Some(ReasoningEffort::High),
            },
            ValidatedMode {
                name: "Coder".to_string(),
                model: "deepseek:deepseek-chat".to_string(),
                reasoning_effort: None,
            },
        ]
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
        let modes = validated_modes();
        let available = available_models();
        let mut state = fake_config_state("deepseek:deepseek-chat");
        let setting = ConfigSetting::Mode("Planner".to_string());

        let result = state.apply_config_change(&modes, &available, &setting);

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
        let modes = validated_modes();
        let available = available_models();
        let mut state = fake_config_state("deepseek:deepseek-chat");
        let setting = ConfigSetting::Mode("Unknown".to_string());

        let result = state.apply_config_change(&modes, &available, &setting);

        assert!(result.is_err());
    }

    #[test]
    fn reasoning_effort_change_preserves_selected_mode() {
        let modes = validated_modes();
        let available = available_models();
        let mut state = fake_config_state("anthropic:claude-sonnet-4-5");
        state.reasoning_effort = Some(ReasoningEffort::High);
        state.selected_mode = Some("Planner".to_string());

        let setting = ConfigSetting::ReasoningEffort(Some(ReasoningEffort::Low));
        let result = state.apply_config_change(&modes, &available, &setting);

        assert!(result.is_ok());
        assert_eq!(state.reasoning_effort, Some(ReasoningEffort::Low));
        assert_eq!(state.selected_mode.as_deref(), Some("Planner"));
    }

    #[test]
    fn manual_model_change_clears_mode_selection_when_no_tuple_match() {
        let modes = validated_modes();
        let available = available_models();
        let mut state = fake_config_state("anthropic:claude-sonnet-4-5");
        state.reasoning_effort = Some(ReasoningEffort::Medium);
        state.selected_mode = Some("Planner".to_string());
        let setting = ConfigSetting::Model("deepseek:deepseek-chat".to_string());

        let result = state.apply_config_change(&modes, &available, &setting);

        assert!(result.is_ok());
        assert_eq!(
            state.pending_model.as_deref(),
            Some("deepseek:deepseek-chat")
        );
        assert!(state.selected_mode.is_none());
    }

    #[tokio::test]
    async fn initialize_always_advertises_load_session_support() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let manager = SessionManager::new(AcpActorHandle::new(tx));

        let response = manager
            .initialize(InitializeRequest::new(ProtocolVersion::LATEST))
            .await
            .expect("initialize succeeds");

        let json = serde_json::to_string(&response).expect("response serializes");
        assert!(json.contains("\"loadSession\":true"));
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
                    .load_session(true)
                    .mcp_capabilities(McpCapabilities::new().http(true).sse(true))
                    .session_capabilities(
                        acp::SessionCapabilities::new().list(acp::SessionListCapabilities::new()),
                    )
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
        let auth_methods = build_auth_methods();
        let auth_methods_notification: ExtNotification =
            AuthMethodsUpdatedParams { auth_methods }.into();
        if let Err(e) = self
            .actor_handle
            .send_ext_notification(auth_methods_notification)
            .await
        {
            error!("Failed to send auth methods updated notification: {:?}", e);
        }

        // Broadcast updated config options to all active sessions
        let available = catalog::get_local_models().await;
        let all_models = get_all_models(&available);
        let sessions = self.sessions.lock().await;
        for (id, state) in sessions.iter() {
            let model = effective_model(
                &state.config.active_model,
                state.config.pending_model.as_deref(),
            );
            let options = build_config_options_from_modes(
                &state.modes,
                &available,
                state.config.selected_mode.as_deref(),
                model,
                state.config.reasoning_effort,
                &all_models,
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

    async fn new_session(
        &self,
        mut args: NewSessionRequest,
    ) -> Result<NewSessionResponse, acp::Error> {
        // Inside a sandbox container the client sends the *host* cwd, but the
        // project is mounted at the container's working directory.
        if std::env::var("AETHER_INSIDE_SANDBOX").is_ok() {
            let container_cwd = std::env::current_dir().unwrap_or_else(|_| "/workspace".into());
            info!(
                "Sandbox: remapping cwd {:?} -> {:?}",
                args.cwd, container_cwd
            );
            args.cwd = container_cwd;
        }

        info!("Creating new session with cwd: {:?}", args.cwd);
        let session_id = uuid::Uuid::new_v4().to_string();
        let acp_session_id = acp::SessionId::new(session_id.clone());

        let mode_catalog = Self::load_mode_catalog(&args.cwd).await?;
        let available = catalog::get_local_models().await;
        let default_model = pick_default_model(&available).ok_or_else(|| {
            error!("No models available — set an API key env var (e.g. ANTHROPIC_API_KEY)");
            acp::Error::internal_error()
        })?;

        let (initial_selected_mode, mode_config) = select_initial_mode(&mode_catalog.modes);

        let spec = if let Some(selected_mode) = initial_selected_mode.as_deref() {
            mode_catalog
                .catalog
                .resolve(selected_mode, &args.cwd)
                .map_err(|e| {
                    error!(
                        "Failed to resolve runtime inputs for mode '{}': {e}",
                        selected_mode
                    );
                    acp::Error::invalid_params()
                })?
        } else {
            mode_catalog
                .catalog
                .resolve_default(default_model, None, &args.cwd)
        };

        let model_str = spec.model.clone();
        let initial_reasoning_effort = mode_config.and_then(|(_, effort)| effort);

        let session = Session::new(
            spec,
            args.cwd.clone(),
            map_acp_mcp_servers(args.mcp_servers),
            None,
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

        let meta = SessionMeta {
            session_id: session_id.clone(),
            cwd: args.cwd.clone(),
            model: model_str.clone(),
            selected_mode: initial_selected_mode.clone(),
            created_at: IsoString::now().0,
        };
        if let Err(e) = self.session_store.append_meta(&session_id, &meta) {
            error!("Failed to write session meta: {e}");
        }

        let config_options = self
            .register_session(
                session,
                &session_id,
                &acp_session_id,
                &model_str,
                initial_selected_mode,
                initial_reasoning_effort,
                mode_catalog.modes,
            )
            .await;

        info!("Session {} created successfully", session_id);

        let response =
            NewSessionResponse::new(acp_session_id.clone()).config_options(config_options);

        self.spawn_available_commands_notification(available_commands, acp_session_id, &session_id);

        Ok(response)
    }

    async fn list_sessions(
        &self,
        args: ListSessionsRequest,
    ) -> Result<ListSessionsResponse, acp::Error> {
        info!("Listing sessions, cwd filter: {:?}", args.cwd);
        let mut summaries = self.session_store.list();

        if let Some(ref cwd) = args.cwd {
            summaries.retain(|s| s.meta.cwd == *cwd);
        }

        let sessions: Vec<acp::SessionInfo> = summaries
            .into_iter()
            .map(|s| {
                acp::SessionInfo::new(s.meta.session_id, s.meta.cwd)
                    .updated_at(s.meta.created_at)
                    .title(s.title)
            })
            .collect();

        info!("Found {} sessions", sessions.len());
        Ok(ListSessionsResponse::new(sessions))
    }

    async fn load_session(
        &self,
        args: LoadSessionRequest,
    ) -> Result<LoadSessionResponse, acp::Error> {
        let session_id = args.session_id.0.to_string();
        info!("Loading session: {session_id}");

        let (meta, events) = self.session_store.load(&session_id).ok_or_else(|| {
            error!("Session not found: {session_id}");
            acp::Error::invalid_params()
        })?;

        let context = Context::from_events(&events);
        let mode_catalog = Self::load_mode_catalog(&args.cwd).await?;

        let spec = if let Some(mode_name) = meta.selected_mode.as_deref() {
            mode_catalog
                .catalog
                .resolve(mode_name, &args.cwd)
                .map_err(|e| {
                    error!(
                        "Failed to resolve runtime inputs for mode '{}': {e}",
                        mode_name
                    );
                    acp::Error::invalid_params()
                })?
        } else {
            let parsed_model: LlmModel = meta.model.parse().map_err(|e: String| {
                error!("Failed to parse restored model '{}': {e}", meta.model);
                acp::Error::invalid_params()
            })?;
            mode_catalog
                .catalog
                .resolve_default(&parsed_model, None, &args.cwd)
        };

        let model = spec.model.clone();

        let restored_messages: Vec<_> = context
            .messages()
            .iter()
            .filter(|m| !m.is_system())
            .cloned()
            .collect();

        let session = Session::new(
            spec,
            args.cwd.clone(),
            map_acp_mcp_servers(args.mcp_servers),
            Some(restored_messages),
        )
        .await
        .map_err(|e| {
            error!("Failed to create session for load: {e}");
            acp::Error::internal_error()
        })?;

        let available_commands = session.list_available_commands().await.map_err(|e| {
            error!("Failed to list available commands: {e}");
            acp::Error::internal_error()
        })?;

        let acp_session_id = acp::SessionId::new(session_id.clone());

        let config_options = self
            .register_session(
                session,
                &session_id,
                &acp_session_id,
                &model,
                meta.selected_mode,
                None,
                mode_catalog.modes,
            )
            .await;

        info!("Session {session_id} loaded successfully");

        let response = LoadSessionResponse::new().config_options(config_options);

        let actor_handle = self.actor_handle.clone();
        let replay_session_id = acp_session_id.clone();
        spawn(async move {
            replay_to_client(&events, &actor_handle, &replay_session_id).await;
        });

        self.spawn_available_commands_notification(available_commands, acp_session_id, &session_id);

        Ok(response)
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

        let available = get_local_models().await;
        let all_models = get_all_models(&available);

        let mut sessions = self.sessions.lock().await;
        let state = sessions.get_mut(&session_id_str).ok_or_else(|| {
            error!("Session not found: {}", session_id_str);
            acp::Error::invalid_params()
        })?;

        state
            .config
            .apply_config_change(&state.modes, &available, &setting)?;

        let effective_model = effective_model(
            &state.config.active_model,
            state.config.pending_model.as_deref(),
        );
        let options = build_config_options_from_modes(
            &state.modes,
            &available,
            state.config.selected_mode.as_deref(),
            effective_model,
            state.config.reasoning_effort,
            &all_models,
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
