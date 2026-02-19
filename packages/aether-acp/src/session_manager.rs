use aether::events::AgentMessage;
use agent_client_protocol::{
    self as acp, Agent, AgentCapabilities, AuthenticateRequest, AuthenticateResponse,
    AvailableCommandsUpdate, Implementation, InitializeRequest, InitializeResponse,
    LoadSessionRequest, LoadSessionResponse, NewSessionRequest, NewSessionResponse,
    PromptCapabilities, PromptResponse, ProtocolVersion, SessionConfigOption,
    SessionConfigOptionCategory, SessionConfigSelectOption, SessionNotification, SessionUpdate,
    SetSessionConfigOptionRequest, SetSessionConfigOptionResponse, SetSessionModeRequest,
    SetSessionModeResponse,
};
use llm::catalog::{self, LlmModel};
use llm::parser::ModelProviderParser;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::spawn;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

use crate::mappers::{
    map_acp_mcp_servers, map_agent_message_to_session_notification,
    map_agent_message_to_stop_reason, try_into_ext_notification,
};
use crate::session::Session;
use acp_utils::content::map_content_blocks_to_text;
use acp_utils::server::AcpActorHandle;

/// Per-session state including active and staged model selections.
struct SessionState {
    session: Session,
    active_model: String,
    pending_model: Option<String>,
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

    async fn send_notification(
        &self,
        notification: acp::SessionNotification,
    ) -> Result<(), acp::Error> {
        self.actor_handle
            .send_session_notification(notification)
            .await
            .map_err(|_| acp::Error::internal_error())?;
        Ok(())
    }
}

/// Format provider:model_id string for an LlmModel
fn provider_model_str(m: &LlmModel) -> String {
    format!("{}:{}", m.provider(), m.model_id())
}

/// Map lowercase provider identifiers to human-readable display names
fn provider_display_name(provider: &str) -> &str {
    match provider {
        "anthropic" => "Anthropic",
        "deepseek" => "DeepSeek",
        "gemini" => "Gemini",
        "moonshot" => "Moonshot",
        "openrouter" => "OpenRouter",
        "zai" => "ZAI",
        "ollama" => "Ollama",
        "llamacpp" => "LlamaCpp",
        other => other,
    }
}

/// Extract distinct provider identifiers from available models in stable order
fn unique_providers(available: &[LlmModel]) -> Vec<&'static str> {
    let mut seen = Vec::new();
    for m in available {
        let p = m.provider();
        if !seen.contains(&p) {
            seen.push(p);
        }
    }
    seen
}

fn provider_required_env_var(models: &[LlmModel], provider: &str) -> Option<&'static str> {
    models
        .iter()
        .find(|m| m.provider() == provider)
        .and_then(|m| m.required_env_var())
}

fn unavailable_reason(model: &LlmModel) -> String {
    model
        .required_env_var()
        .map(|var| format!("Unavailable: set {var}"))
        .unwrap_or_else(|| "Unavailable: provider is not configured".to_string())
}

/// Extract the provider portion from a "provider:model_id" string
fn provider_from_model_str(model_str: &str) -> &str {
    model_str.split(':').next().unwrap_or(model_str)
}

fn model_exists(available: &[LlmModel], model_str: &str) -> bool {
    available.iter().any(|m| provider_model_str(m) == model_str)
}

fn effective_model<'a>(active_model: &'a str, pending_model: Option<&'a str>) -> &'a str {
    pending_model.unwrap_or(active_model)
}

/// Build the "Provider" select config option
fn build_provider_config_option(
    available: &[LlmModel],
    current_provider: &str,
) -> SessionConfigOption {
    let all_models = catalog::LlmModel::all();
    let providers = unique_providers(all_models);
    let available_providers: HashSet<&str> = available.iter().map(|m| m.provider()).collect();
    let options: Vec<SessionConfigSelectOption> = providers
        .iter()
        .map(|p| {
            let is_available = available_providers.contains(p);
            let display_name = provider_display_name(p);
            let name = if is_available {
                display_name.to_string()
            } else {
                format!("{display_name} (unavailable)")
            };

            let option = SessionConfigSelectOption::new(p.to_string(), name);
            if is_available {
                option
            } else {
                let description = provider_required_env_var(all_models, p)
                    .map(|var| format!("Unavailable: set {var}"))
                    .unwrap_or_else(|| "Unavailable: provider is not configured".to_string());
                option.description(description)
            }
        })
        .collect();

    SessionConfigOption::select(
        "provider",
        "Provider",
        current_provider.to_string(),
        options,
    )
    .category(SessionConfigOptionCategory::Model)
}

/// Build the "Model" select config option, filtered to only models from the given provider
fn build_model_config_option(
    available: &[LlmModel],
    current_provider: &str,
    current_model: &str,
) -> SessionConfigOption {
    let all_models = catalog::LlmModel::all();
    let available_models: HashSet<String> = available.iter().map(provider_model_str).collect();
    let options: Vec<acp::SessionConfigSelectOption> = all_models
        .iter()
        .filter(|m| m.provider() == current_provider)
        .map(|m| {
            let value = provider_model_str(m);
            let is_available = available_models.contains(&value);
            let name = if is_available {
                m.display_name().to_string()
            } else {
                format!("{} (unavailable)", m.display_name())
            };
            let option = acp::SessionConfigSelectOption::new(value, name);
            if is_available {
                option
            } else {
                option.description(unavailable_reason(m))
            }
        })
        .collect();

    SessionConfigOption::select("model", "Model", current_model.to_string(), options)
        .category(SessionConfigOptionCategory::Model)
}

/// Build both Provider and Model config options for the given state
fn build_config_options(available: &[LlmModel], current_model: &str) -> Vec<SessionConfigOption> {
    let current_provider = provider_from_model_str(current_model);
    vec![
        build_provider_config_option(available, current_provider),
        build_model_config_option(available, current_provider, current_model),
    ]
}

/// Pick a default model from the available list.
/// Prefers Claude Sonnet 4.5 (latest alias), then first available.
fn pick_default_model(available: &[LlmModel]) -> Option<&LlmModel> {
    // Prefer claude-sonnet-4-5 (latest alias)
    available
        .iter()
        .find(|m| m.model_id() == "claude-sonnet-4-5")
        .or_else(|| available.first())
}

/// Resolve MCP config path from the session's CWD.
/// Returns Some if `cwd/mcp.json` exists.
fn resolve_mcp_config(cwd: &Path) -> Option<PathBuf> {
    let path = cwd.join("mcp.json");
    path.exists().then_some(path)
}

#[async_trait::async_trait(?Send)]
impl Agent for SessionManager {
    async fn initialize(&self, args: InitializeRequest) -> Result<InitializeResponse, acp::Error> {
        info!("Received initialize request: {:?}", args);
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
            ))
    }

    async fn authenticate(
        &self,
        args: AuthenticateRequest,
    ) -> Result<AuthenticateResponse, acp::Error> {
        info!("Received authenticate request: {:?}", args);
        // No authentication required
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

        let model_str = provider_model_str(default_model);

        let parser = ModelProviderParser::default();
        let (llm, _) = parser.parse(&model_str).map_err(|e| {
            error!("Failed to create provider for '{}': {}", model_str, e);
            acp::Error::internal_error()
        })?;

        let mcp_config_path = resolve_mcp_config(&args.cwd);

        let session = Session::new(
            session_id.clone(),
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

        let state = SessionState {
            session,
            active_model: model_str.clone(),
            pending_model: None,
        };

        let mut sessions = self.sessions.lock().await;
        sessions.insert(session_id.clone(), state);

        info!("Session {} created successfully", session_id);

        let config_options = build_config_options(&available, &model_str);
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
        let session_id = args.session_id.clone();
        let mut sessions = self.sessions.lock().await;
        let state = sessions.get_mut(&session_id_str).ok_or_else(|| {
            error!("Session not found: {}", session_id_str);
            acp::Error::invalid_params()
        })?;

        let mut prompt_text = map_content_blocks_to_text(args.prompt);
        debug!("Prompt text: {}", prompt_text);

        if let Some(pending_model) = state.pending_model.clone() {
            if pending_model != state.active_model {
                state
                    .session
                    .switch_model(&pending_model)
                    .await
                    .map_err(|e| {
                        error!(
                            "Failed to switch model for session {}: {}",
                            session_id_str, e
                        );
                        acp::Error::internal_error()
                    })?;
                state.active_model = pending_model;
            }
            state.pending_model = None;
        }

        if let Some(slash_command_text) = prompt_text.strip_prefix('/') {
            info!("Detected slash command in prompt");

            let (command_name, args_text) =
                if let Some(space_idx) = slash_command_text.find(char::is_whitespace) {
                    let (cmd, args) = slash_command_text.split_at(space_idx);
                    (cmd, args.trim())
                } else {
                    (slash_command_text, "")
                };

            match state
                .session
                .expand_slash_command(command_name, args_text)
                .await
            {
                Ok(expanded_prompt) => {
                    info!(
                        "Expanded slash command '{}' -> {} chars",
                        command_name,
                        expanded_prompt.len()
                    );
                    prompt_text = expanded_prompt;
                }
                Err(e) => {
                    error!("Failed to expand slash command '{}': {}", command_name, e);
                    // Continue with original prompt text rather than failing
                    // This allows graceful degradation if command expansion fails
                }
            }
        }

        state.session.send_prompt(prompt_text).await.map_err(|e| {
            error!("Failed to send prompt: {}", e);
            acp::Error::internal_error()
        })?;

        let final_stop_reason;
        loop {
            match state.session.recv().await {
                Some(msg) => {
                    info!("Received agent message: {:?}", &msg);

                    if let Some(notification) =
                        map_agent_message_to_session_notification(session_id.clone(), &msg)
                    {
                        info!("Sending session notification");
                        self.send_notification(notification).await?;
                    } else if let Some(ext_notification) = try_into_ext_notification(&msg) {
                        info!("Sending ext notification: {}", ext_notification.method);
                        self.actor_handle
                            .send_ext_notification(ext_notification)
                            .await
                            .map_err(|_| acp::Error::internal_error())?;
                    } else {
                        info!("No notification generated for this message");
                    }

                    match &msg {
                        AgentMessage::Done
                        | AgentMessage::Cancelled { .. }
                        | AgentMessage::Error { .. } => {
                            final_stop_reason = map_agent_message_to_stop_reason(&msg);
                            info!(
                                "Terminal message received, stop reason: {:?}",
                                final_stop_reason
                            );
                            break;
                        }
                        _ => {
                            // Continue processing messages
                        }
                    }
                }
                None => {
                    error!("Agent channel closed unexpectedly");
                    return Err(acp::Error::internal_error());
                }
            }
        }

        info!("Prompt completed with stop reason: {:?}", final_stop_reason);

        Ok(PromptResponse::new(final_stop_reason))
    }

    async fn cancel(&self, args: acp::CancelNotification) -> Result<(), acp::Error> {
        info!("Received cancel for session: {:?}", args.session_id);
        let session_id_str = args.session_id.0.to_string();
        let sessions = self.sessions.lock().await;
        let state = sessions.get(&session_id_str).ok_or_else(|| {
            error!("Session not found for cancel: {}", session_id_str);
            acp::Error::invalid_params()
        })?;

        state.session.cancel().await.map_err(|e| {
            error!("Failed to cancel session: {}", e);
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

        match config_id.as_str() {
            "provider" => {
                // Find the first model for the new provider
                let first_model = available
                    .iter()
                    .find(|m| m.provider() == value)
                    .ok_or_else(|| {
                        error!("No models found for provider: {}", value);
                        acp::Error::invalid_params()
                    })?;
                let new_model_str = provider_model_str(first_model);
                let mut sessions = self.sessions.lock().await;
                let state = sessions.get_mut(&session_id_str).ok_or_else(|| {
                    error!("Session not found: {}", session_id_str);
                    acp::Error::invalid_params()
                })?;

                if state.active_model == new_model_str {
                    state.pending_model = None;
                } else {
                    state.pending_model = Some(new_model_str.clone());
                }

                let options = build_config_options(
                    &available,
                    effective_model(&state.active_model, state.pending_model.as_deref()),
                );
                Ok(SetSessionConfigOptionResponse::new(options))
            }
            "model" => {
                if !model_exists(&available, &value) {
                    error!("Unknown model in set_session_config_option: {}", value);
                    return Err(acp::Error::invalid_params());
                }

                let mut sessions = self.sessions.lock().await;
                let state = sessions.get_mut(&session_id_str).ok_or_else(|| {
                    error!("Session not found: {}", session_id_str);
                    acp::Error::invalid_params()
                })?;

                if state.active_model == value {
                    state.pending_model = None;
                } else {
                    state.pending_model = Some(value);
                }

                let options = build_config_options(
                    &available,
                    effective_model(&state.active_model, state.pending_model.as_deref()),
                );
                Ok(SetSessionConfigOptionResponse::new(options))
            }
            _ => {
                error!("Unknown config option: {}", config_id);
                Err(acp::Error::invalid_params())
            }
        }
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
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::{SessionConfigKind, SessionConfigSelectOptions};
    use llm::catalog::{AnthropicModel, DeepSeekModel, GeminiModel};

    fn test_models() -> Vec<LlmModel> {
        vec![
            LlmModel::Anthropic(AnthropicModel::ClaudeSonnet45),
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus46),
            LlmModel::DeepSeek(DeepSeekModel::DeepseekChat),
            LlmModel::Gemini(GeminiModel::Gemini25Pro),
        ]
    }

    #[test]
    fn provider_display_name_maps_known_providers() {
        assert_eq!(provider_display_name("anthropic"), "Anthropic");
        assert_eq!(provider_display_name("deepseek"), "DeepSeek");
        assert_eq!(provider_display_name("gemini"), "Gemini");
        assert_eq!(provider_display_name("openrouter"), "OpenRouter");
    }

    #[test]
    fn provider_display_name_returns_input_for_unknown() {
        assert_eq!(provider_display_name("mystery"), "mystery");
    }

    #[test]
    fn unique_providers_preserves_order() {
        let models = test_models();
        let providers = unique_providers(&models);
        assert_eq!(providers, vec!["anthropic", "deepseek", "gemini"]);
    }

    #[test]
    fn unique_providers_deduplicates() {
        let models = test_models();
        let providers = unique_providers(&models);
        assert_eq!(providers.iter().filter(|&&p| p == "anthropic").count(), 1);
    }

    #[test]
    fn provider_from_model_str_splits_correctly() {
        assert_eq!(
            provider_from_model_str("anthropic:claude-opus-4-6"),
            "anthropic"
        );
        assert_eq!(
            provider_from_model_str("deepseek:deepseek-chat"),
            "deepseek"
        );
    }

    #[test]
    fn provider_from_model_str_handles_no_colon() {
        assert_eq!(provider_from_model_str("justaprovider"), "justaprovider");
    }

    #[test]
    fn build_provider_config_option_has_correct_structure() {
        let models = test_models();
        let opt = build_provider_config_option(&models, "anthropic");

        assert_eq!(opt.id.0.as_ref(), "provider");
        assert_eq!(opt.name, "Provider");

        let SessionConfigKind::Select(ref select) = opt.kind else {
            panic!("Expected Select kind");
        };
        assert_eq!(select.current_value.0.as_ref(), "anthropic");

        let SessionConfigSelectOptions::Ungrouped(ref options) = select.options else {
            panic!("Expected Ungrouped options");
        };
        assert!(
            options
                .iter()
                .any(|o| o.value.0.as_ref() == "anthropic" && o.name == "Anthropic")
        );
        assert!(
            options
                .iter()
                .any(|o| o.value.0.as_ref() == "deepseek" && o.name == "DeepSeek")
        );
        assert!(
            options
                .iter()
                .any(|o| o.value.0.as_ref() == "gemini" && o.name == "Gemini")
        );
    }

    #[test]
    fn build_provider_config_option_marks_unavailable_providers() {
        let models = test_models();
        let opt = build_provider_config_option(&models, "anthropic");

        let SessionConfigKind::Select(ref select) = opt.kind else {
            panic!("Expected Select kind");
        };
        let SessionConfigSelectOptions::Ungrouped(ref options) = select.options else {
            panic!("Expected Ungrouped options");
        };

        let openrouter = options
            .iter()
            .find(|o| o.value.0.as_ref() == "openrouter")
            .expect("expected openrouter provider option");
        assert!(openrouter.name.contains("unavailable"));
        assert!(
            openrouter
                .description
                .as_deref()
                .is_some_and(|d| d.starts_with("Unavailable:"))
        );
    }

    #[test]
    fn build_model_config_option_filters_by_provider() {
        let models = test_models();
        let opt = build_model_config_option(&models, "anthropic", "anthropic:claude-sonnet-4-5");

        assert_eq!(opt.id.0.as_ref(), "model");

        let SessionConfigKind::Select(ref select) = opt.kind else {
            panic!("Expected Select kind");
        };

        let SessionConfigSelectOptions::Ungrouped(ref options) = select.options else {
            panic!("Expected Ungrouped options");
        };

        // Only anthropic models should be listed
        assert!(options.len() >= 2);
        for o in options {
            assert!(
                o.value.0.starts_with("anthropic:"),
                "Expected anthropic model, got: {}",
                o.value.0
            );
        }
    }

    #[test]
    fn build_model_config_option_marks_unavailable_models_with_reason() {
        let models = test_models();
        let opt = build_model_config_option(&models, "openrouter", "openrouter:openai/gpt-4o");

        let SessionConfigKind::Select(ref select) = opt.kind else {
            panic!("Expected Select kind");
        };
        let SessionConfigSelectOptions::Ungrouped(ref options) = select.options else {
            panic!("Expected Ungrouped options");
        };
        assert!(!options.is_empty());
        let first = &options[0];
        assert!(first.name.contains("unavailable"));
        assert!(
            first
                .description
                .as_deref()
                .is_some_and(|d| d.starts_with("Unavailable:"))
        );
    }

    #[test]
    fn build_config_options_returns_provider_and_model() {
        let models = test_models();
        let opts = build_config_options(&models, "deepseek:deepseek-chat");

        assert_eq!(opts.len(), 2);
        assert_eq!(opts[0].id.0.as_ref(), "provider");
        assert_eq!(opts[1].id.0.as_ref(), "model");

        // Provider should be set to deepseek
        let SessionConfigKind::Select(ref provider_select) = opts[0].kind else {
            panic!("Expected Select kind");
        };
        assert_eq!(provider_select.current_value.0.as_ref(), "deepseek");

        // Model list should only contain deepseek models
        let SessionConfigKind::Select(ref model_select) = opts[1].kind else {
            panic!("Expected Select kind");
        };
        let SessionConfigSelectOptions::Ungrouped(ref model_options) = model_select.options else {
            panic!("Expected Ungrouped options");
        };
        assert!(!model_options.is_empty());
        assert!(
            model_options
                .iter()
                .all(|o| o.value.0.starts_with("deepseek:"))
        );
        assert!(
            model_options
                .iter()
                .any(|o| o.value.0.as_ref() == "deepseek:deepseek-chat")
        );
    }

    #[test]
    fn model_exists_accepts_known_model() {
        let models = test_models();
        assert!(model_exists(&models, "anthropic:claude-sonnet-4-5"));
        assert!(model_exists(&models, "deepseek:deepseek-chat"));
    }

    #[test]
    fn model_exists_rejects_unknown_model() {
        let models = test_models();
        assert!(!model_exists(&models, "anthropic:not-real"));
        assert!(!model_exists(&models, "mystery:some-model"));
    }

    #[test]
    fn effective_model_prefers_pending() {
        assert_eq!(
            effective_model(
                "anthropic:claude-sonnet-4-5",
                Some("deepseek:deepseek-chat")
            ),
            "deepseek:deepseek-chat"
        );
    }

    #[test]
    fn effective_model_falls_back_to_active() {
        assert_eq!(
            effective_model("anthropic:claude-sonnet-4-5", None),
            "anthropic:claude-sonnet-4-5"
        );
    }
}
