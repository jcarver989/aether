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

use super::mappers::map_acp_mcp_servers;
use super::model_config::{
    build_config_options, effective_model, model_exists, pick_default_model,
};
use super::relay::{RelayHandle, SessionCommand, spawn_relay};
use super::session::Session;
use acp_utils::content::map_content_blocks_to_text;
use acp_utils::server::AcpActorHandle;

/// Per-session state including active and staged model selections.
struct SessionState {
    relay_tx: mpsc::Sender<SessionCommand>,
    mcp_request_tx: mpsc::Sender<McpRequest>,
    #[allow(dead_code)]
    _relay_handle: JoinHandle<()>,
    active_model: String,
    pending_model: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
}

/// Manages ACP sessions, each session has its own agent and state
pub struct SessionManager {
    system_prompt: Option<String>,
    sessions: Arc<Mutex<HashMap<String, SessionState>>>,
    next_session_id: Arc<Mutex<u64>>,
    actor_handle: AcpActorHandle,
}

impl SessionManager {
    pub fn new(system_prompt: Option<String>, actor_handle: AcpActorHandle) -> Self {
        info!("Creating SessionManager");
        Self {
            system_prompt,
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
            let model = effective_model(&state.active_model, state.pending_model.as_deref());
            let options = build_config_options(&available, model, state.reasoning_effort);
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

        let model_str = default_model.to_string();

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

        let state = SessionState {
            relay_tx: cmd_tx,
            mcp_request_tx,
            _relay_handle: join_handle,
            active_model: model_str.clone(),
            pending_model: None,
            reasoning_effort: None,
        };

        let mut sessions = self.sessions.lock().await;
        sessions.insert(session_id.clone(), state);

        info!("Session {} created successfully", session_id);

        let config_options = build_config_options(&available, &model_str, None);
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

            let switch_model = state.pending_model.take().and_then(|pending| {
                if pending == state.active_model {
                    None
                } else {
                    state.active_model.clone_from(&pending);
                    Some(pending)
                }
            });

            let reasoning_effort = state.reasoning_effort;

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

        let available = catalog::available_models();

        let mut sessions = self.sessions.lock().await;
        let state = sessions.get_mut(&session_id_str).ok_or_else(|| {
            error!("Session not found: {}", session_id_str);
            acp::Error::invalid_params()
        })?;

        match config_id.as_str() {
            "model" => {
                if !model_exists(&available, &value) {
                    error!("Unknown model in set_session_config_option: {}", value);
                    return Err(acp::Error::invalid_params());
                }
                state.pending_model = (state.active_model != value).then_some(value);
            }
            "reasoning_effort" => {
                state.reasoning_effort = ReasoningEffort::parse(&value).map_err(|e| {
                    error!("{e}");
                    acp::Error::invalid_params()
                })?;
            }
            _ => {
                error!("Unknown config option: {}", config_id);
                return Err(acp::Error::invalid_params());
            }
        }

        let options = build_config_options(
            &available,
            effective_model(&state.active_model, state.pending_model.as_deref()),
            state.reasoning_effort,
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
