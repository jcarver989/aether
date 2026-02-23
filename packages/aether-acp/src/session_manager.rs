use agent_client_protocol::{
    self as acp, Agent, AgentCapabilities, AuthenticateRequest, AuthenticateResponse,
    AvailableCommandsUpdate, Implementation, InitializeRequest, InitializeResponse,
    LoadSessionRequest, LoadSessionResponse, NewSessionRequest, NewSessionResponse,
    PromptCapabilities, PromptResponse, ProtocolVersion, SessionConfigOption,
    SessionConfigOptionCategory, SessionNotification, SessionUpdate, SetSessionConfigOptionRequest,
    SetSessionConfigOptionResponse, SetSessionModeRequest, SetSessionModeResponse,
};
use llm::catalog::{self, LlmModel};
use llm::parser::ModelProviderParser;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::spawn;
use tokio::sync::Mutex;
use tokio::sync::oneshot;
use tracing::{debug, error, info};

use crate::mappers::map_acp_mcp_servers;
use crate::relay::{SessionCommand, spawn_relay};
use crate::session::Session;
use acp_utils::content::map_content_blocks_to_text;
use acp_utils::server::AcpActorHandle;

/// Per-session state including active and staged model selections.
struct SessionState {
    relay_tx: tokio::sync::mpsc::Sender<SessionCommand>,
    #[allow(dead_code)]
    _relay_handle: tokio::task::JoinHandle<()>,
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
}

/// Format `provider:model_id` string for an `LlmModel`
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

fn unavailable_reason(model: &LlmModel) -> String {
    model.required_env_var().map_or_else(
        || "Unavailable: provider is not configured".to_string(),
        |var| format!("Unavailable: set {var}"),
    )
}

fn model_exists(available: &[LlmModel], model_str: &str) -> bool {
    available.iter().any(|m| provider_model_str(m) == model_str)
}

fn effective_model<'a>(active_model: &'a str, pending_model: Option<&'a str>) -> &'a str {
    pending_model.unwrap_or(active_model)
}

/// Build the "Model" select config option with all models from all providers.
/// Display names use "Provider / `ModelName`" format.
/// Fully-unavailable providers are collapsed into a single summary line.
fn build_model_config_option(available: &[LlmModel], current_model: &str) -> SessionConfigOption {
    let all_models = catalog::LlmModel::all();
    let available_models: HashSet<String> = available.iter().map(provider_model_str).collect();

    // Phase 1: Group models by provider, counting available models per provider
    struct ProviderGroup<'a> {
        models: Vec<&'a LlmModel>,
        available_count: usize,
    }

    let mut groups: BTreeMap<&str, ProviderGroup<'_>> = BTreeMap::new();
    for m in all_models {
        let value = provider_model_str(m);
        let is_available = available_models.contains(&value);
        let group = groups.entry(m.provider()).or_insert_with(|| ProviderGroup {
            models: Vec::new(),
            available_count: 0,
        });
        group.models.push(m);
        if is_available {
            group.available_count += 1;
        }
    }

    // Phase 2: Emit options per group
    let mut options: Vec<acp::SessionConfigSelectOption> = Vec::new();
    for (provider_key, group) in &groups {
        if group.available_count == 0 {
            // Fully unavailable — emit one collapsed entry
            let display = provider_display_name(provider_key);
            let count = group.models.len();
            let noun = if count == 1 { "model" } else { "models" };
            let name = format!("{display} ({count} {noun})");
            let value = format!("__unavailable:{provider_key}");
            let reason = group.models[0].required_env_var().map_or_else(
                || "Unavailable: provider is not configured".to_string(),
                |var| format!("Unavailable: set {var}"),
            );
            options.push(acp::SessionConfigSelectOption::new(value, name).description(reason));
        } else {
            // Mixed or fully available — list each model individually
            for m in &group.models {
                let value = provider_model_str(m);
                let is_available = available_models.contains(&value);
                let display = provider_display_name(provider_key);
                let name = if is_available {
                    format!("{display} / {}", m.display_name())
                } else {
                    format!("{display} / {} (unavailable)", m.display_name())
                };
                let option = acp::SessionConfigSelectOption::new(value, name);
                if is_available {
                    options.push(option);
                } else {
                    options.push(option.description(unavailable_reason(m)));
                }
            }
        }
    }

    SessionConfigOption::select("model", "Model", current_model.to_string(), options)
        .category(SessionConfigOptionCategory::Model)
}

/// Build config options for the given state
fn build_config_options(available: &[LlmModel], current_model: &str) -> Vec<SessionConfigOption> {
    vec![build_model_config_option(available, current_model)]
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

        let (relay_tx, relay_handle) =
            spawn_relay(session, self.actor_handle.clone(), acp_session_id.clone());

        let state = SessionState {
            relay_tx,
            _relay_handle: relay_handle,
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

        let (relay_tx, prompt_text, switch_model) = {
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

            (state.relay_tx.clone(), prompt_text, switch_model)
        };

        let (result_tx, result_rx) = oneshot::channel();
        relay_tx
            .send(SessionCommand::Prompt {
                text: prompt_text,
                switch_model,
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

        if config_id.as_str() != "model" {
            error!("Unknown config option: {}", config_id);
            return Err(acp::Error::invalid_params());
        }

        if !model_exists(&available, &value) {
            error!("Unknown model in set_session_config_option: {}", value);
            return Err(acp::Error::invalid_params());
        }

        let mut sessions = self.sessions.lock().await;
        let state = sessions.get_mut(&session_id_str).ok_or_else(|| {
            error!("Session not found: {}", session_id_str);
            acp::Error::invalid_params()
        })?;

        state.pending_model = (state.active_model != value).then_some(value);

        let options = build_config_options(
            &available,
            effective_model(&state.active_model, state.pending_model.as_deref()),
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
    fn build_model_config_option_includes_all_providers() {
        let models = test_models();
        let opt = build_model_config_option(&models, "anthropic:claude-sonnet-4-5");

        assert_eq!(opt.id.0.as_ref(), "model");

        let SessionConfigKind::Select(ref select) = opt.kind else {
            panic!("Expected Select kind");
        };

        let SessionConfigSelectOptions::Ungrouped(ref options) = select.options else {
            panic!("Expected Ungrouped options");
        };

        // Available providers list models individually
        assert!(options.iter().any(|o| o.value.0.starts_with("anthropic:")));
        assert!(options.iter().any(|o| o.value.0.starts_with("deepseek:")));
        assert!(options.iter().any(|o| o.value.0.starts_with("gemini:")));

        // Fully-unavailable providers are collapsed into sentinel entries
        assert!(options
            .iter()
            .any(|o| o.value.0.as_ref() == "__unavailable:moonshot"));
        assert!(options
            .iter()
            .any(|o| o.value.0.as_ref() == "__unavailable:openrouter"));
        assert!(options
            .iter()
            .any(|o| o.value.0.as_ref() == "__unavailable:zai"));
    }

    #[test]
    fn build_model_config_option_uses_provider_slash_model_display_names() {
        let models = test_models();
        let opt = build_model_config_option(&models, "anthropic:claude-sonnet-4-5");

        let SessionConfigKind::Select(ref select) = opt.kind else {
            panic!("Expected Select kind");
        };
        let SessionConfigSelectOptions::Ungrouped(ref options) = select.options else {
            panic!("Expected Ungrouped options");
        };

        // Available models should have "Provider / Model" format
        let sonnet = options
            .iter()
            .find(|o| o.value.0.as_ref() == "anthropic:claude-sonnet-4-5")
            .expect("expected anthropic sonnet option");
        assert!(
            sonnet.name.starts_with("Anthropic / "),
            "Expected 'Anthropic / ...' display name, got: {}",
            sonnet.name
        );
    }

    #[test]
    fn build_model_config_option_marks_unavailable_models_with_reason() {
        let models = test_models();
        let opt = build_model_config_option(&models, "anthropic:claude-sonnet-4-5");

        let SessionConfigKind::Select(ref select) = opt.kind else {
            panic!("Expected Select kind");
        };
        let SessionConfigSelectOptions::Ungrouped(ref options) = select.options else {
            panic!("Expected Ungrouped options");
        };

        let unavailable = options
            .iter()
            .find(|o| o.name.contains("unavailable"))
            .expect("expected at least one unavailable model option");
        assert!(unavailable.name.contains(" / "));
        assert!(
            unavailable
                .description
                .as_deref()
                .is_some_and(|d| d.starts_with("Unavailable:"))
        );
    }

    #[test]
    fn build_config_options_returns_single_model_option() {
        let models = test_models();
        let opts = build_config_options(&models, "deepseek:deepseek-chat");

        assert_eq!(opts.len(), 1);
        assert_eq!(opts[0].id.0.as_ref(), "model");

        let SessionConfigKind::Select(ref model_select) = opts[0].kind else {
            panic!("Expected Select kind");
        };
        assert_eq!(
            model_select.current_value.0.as_ref(),
            "deepseek:deepseek-chat"
        );

        // Should include models from all providers
        let SessionConfigSelectOptions::Ungrouped(ref model_options) = model_select.options else {
            panic!("Expected Ungrouped options");
        };
        assert!(
            model_options
                .iter()
                .any(|o| o.value.0.starts_with("anthropic:"))
        );
        assert!(
            model_options
                .iter()
                .any(|o| o.value.0.starts_with("deepseek:"))
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

    #[test]
    fn collapsed_entry_for_fully_unavailable_provider() {
        // test_models() has no Moonshot models available
        let models = test_models();
        let opt = build_model_config_option(&models, "anthropic:claude-sonnet-4-5");

        let SessionConfigKind::Select(ref select) = opt.kind else {
            panic!("Expected Select kind");
        };
        let SessionConfigSelectOptions::Ungrouped(ref options) = select.options else {
            panic!("Expected Ungrouped options");
        };

        let moonshot = options
            .iter()
            .find(|o| o.value.0.as_ref() == "__unavailable:moonshot")
            .expect("expected collapsed moonshot entry");

        // Name should be "Moonshot (N models)"
        assert!(
            moonshot.name.starts_with("Moonshot ("),
            "Expected 'Moonshot (N models)', got: {}",
            moonshot.name
        );
        assert!(moonshot.name.ends_with("models)"));

        // Description triggers is_disabled in TUI
        assert!(moonshot
            .description
            .as_deref()
            .is_some_and(|d| d.starts_with("Unavailable:")));
    }

    #[test]
    fn mixed_provider_lists_models_individually() {
        // test_models() has Gemini25Pro available, so Gemini is "mixed"
        let models = test_models();
        let opt = build_model_config_option(&models, "anthropic:claude-sonnet-4-5");

        let SessionConfigKind::Select(ref select) = opt.kind else {
            panic!("Expected Select kind");
        };
        let SessionConfigSelectOptions::Ungrouped(ref options) = select.options else {
            panic!("Expected Ungrouped options");
        };

        // Should NOT have a collapsed entry for gemini
        assert!(
            !options
                .iter()
                .any(|o| o.value.0.as_ref() == "__unavailable:gemini"),
            "Gemini should not be collapsed when it has available models"
        );

        // Individual gemini models should still be listed
        assert!(options
            .iter()
            .any(|o| o.value.0.starts_with("gemini:") && !o.name.contains("unavailable")));
        assert!(options
            .iter()
            .any(|o| o.value.0.starts_with("gemini:") && o.name.contains("unavailable")));
    }
}
