use agent_client_protocol as acp;
use agent_events::AgentMessage;
use llm::parser::ModelProviderParser;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

use crate::mappers::{
    map_agent_message_to_session_notification, map_agent_message_to_stop_reason,
    try_into_ext_notification,
};
use crate::session::Session;
use acp_utils::content::map_content_blocks_to_text;
use acp_utils::server::AcpActorHandle;

/// Managers ACP sessions, each session has its own agent and state
pub struct SessionManager {
    model_provider: String,
    system_prompt: Option<String>,
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    mcp_config_path: PathBuf,
    next_session_id: Arc<Mutex<u64>>,
    actor_handle: AcpActorHandle,
}

impl SessionManager {
    pub fn new(
        model_provider: String,
        system_prompt: Option<String>,
        mcp_config_path: PathBuf,
        actor_handle: AcpActorHandle,
    ) -> Self {
        info!(
            "Creating AetherAgent with model: {}, MCP config: {:?}",
            model_provider, mcp_config_path
        );
        Self {
            model_provider,
            system_prompt,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            mcp_config_path,
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

#[async_trait::async_trait(?Send)]
impl acp::Agent for SessionManager {
    async fn initialize(
        &self,
        args: acp::InitializeRequest,
    ) -> Result<acp::InitializeResponse, acp::Error> {
        info!("Received initialize request: {:?}", args);
        Ok(
            acp::InitializeResponse::new(acp::ProtocolVersion::V1).agent_capabilities(
                acp::AgentCapabilities::new()
                    .load_session(false)
                    .prompt_capabilities(
                        acp::PromptCapabilities::new()
                            .embedded_context(true)
                            .image(false)
                            .audio(false),
                    ),
            ),
        )
    }

    async fn authenticate(
        &self,
        args: acp::AuthenticateRequest,
    ) -> Result<acp::AuthenticateResponse, acp::Error> {
        info!("Received authenticate request: {:?}", args);
        // No authentication required
        Ok(acp::AuthenticateResponse::default())
    }

    async fn new_session(
        &self,
        args: acp::NewSessionRequest,
    ) -> Result<acp::NewSessionResponse, acp::Error> {
        info!("Creating new session with cwd: {:?}", args.cwd);

        let session_id = self.generate_session_id().await;
        let acp_session_id = acp::SessionId::new(session_id.clone());

        // Parse the model provider
        let parser = ModelProviderParser::default();
        let (llm, _) = parser.parse(&self.model_provider).map_err(|e| {
            error!(
                "Failed to create provider for '{}': {}",
                self.model_provider, e
            );
            acp::Error::internal_error()
        })?;

        let session = Session::new(
            session_id.clone(),
            llm,
            self.system_prompt.clone(),
            self.mcp_config_path.clone(),
            args.cwd,
        )
        .await
        .map_err(|e| {
            error!("Failed to create session: {}", e);
            acp::Error::internal_error()
        })?;

        // Get available commands from the session before inserting it
        let available_commands = session.list_available_commands().await.map_err(|e| {
            error!("Failed to list available commands: {}", e);
            acp::Error::internal_error()
        })?;

        let mut sessions = self.sessions.lock().await;
        sessions.insert(session_id.clone(), session);

        info!("Session {} created successfully", session_id);

        // Build model config option for the client
        let (provider_name, model_name) = self
            .model_provider
            .split_once(':')
            .unwrap_or(("unknown", &self.model_provider));

        let model_option = acp::SessionConfigOption::select(
            "model",
            "Model",
            self.model_provider.clone(),
            vec![
                acp::SessionConfigSelectOption::new(self.model_provider.clone(), model_name)
                    .description(format!("Provider: {provider_name}")),
            ],
        )
        .category(acp::SessionConfigOptionCategory::Model);

        // Prepare the response to return first
        let response =
            acp::NewSessionResponse::new(acp_session_id.clone()).config_options(vec![model_option]);

        // Send available commands update notification asynchronously (don't await)
        // This allows the response to be sent first, then the notification follows
        if !available_commands.is_empty() {
            let command_count = available_commands.len();
            let notification = acp::SessionNotification::new(
                acp_session_id,
                acp::SessionUpdate::AvailableCommandsUpdate(acp::AvailableCommandsUpdate::new(
                    available_commands,
                )),
            );

            // Spawn task to send notification after response is returned
            let actor_handle = self.actor_handle.clone();
            let session_id_log = session_id.clone();
            tokio::spawn(async move {
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
        args: acp::LoadSessionRequest,
    ) -> Result<acp::LoadSessionResponse, acp::Error> {
        info!("Received load_session request: {:?}", args);
        // Not supported yet
        Err(acp::Error::method_not_found())
    }

    async fn prompt(&self, args: acp::PromptRequest) -> Result<acp::PromptResponse, acp::Error> {
        info!("Received prompt for session: {:?}", args.session_id);

        let session_id_str = args.session_id.0.to_string();
        let session_id = args.session_id.clone();

        // Get the session
        let mut sessions = self.sessions.lock().await;
        let session = sessions.get_mut(&session_id_str).ok_or_else(|| {
            error!("Session not found: {}", session_id_str);
            acp::Error::invalid_params()
        })?;

        // Convert prompt to text
        let mut prompt_text = map_content_blocks_to_text(args.prompt);
        debug!("Prompt text: {}", prompt_text);

        // Check if this is a slash command and expand it if so
        if let Some(slash_command_text) = prompt_text.strip_prefix('/') {
            info!("Detected slash command in prompt");

            // Parse command name and arguments
            let (command_name, args_text) =
                if let Some(space_idx) = slash_command_text.find(char::is_whitespace) {
                    let (cmd, args) = slash_command_text.split_at(space_idx);
                    (cmd, args.trim())
                } else {
                    (slash_command_text, "")
                };

            // Expand the slash command
            match session.expand_slash_command(command_name, args_text).await {
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

        // Send the prompt to the agent (either expanded or original)
        session.send_prompt(prompt_text).await.map_err(|e| {
            error!("Failed to send prompt: {}", e);
            acp::Error::internal_error()
        })?;

        // Stream agent messages back as session updates
        let final_stop_reason;

        loop {
            match session.recv().await {
                Some(msg) => {
                    info!("Received agent message: {:?}", &msg);

                    // Send session update for non-terminal messages
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

                    // Check if this is a terminal message
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

        Ok(acp::PromptResponse::new(final_stop_reason))
    }

    async fn cancel(&self, args: acp::CancelNotification) -> Result<(), acp::Error> {
        info!("Received cancel for session: {:?}", args.session_id);

        let session_id_str = args.session_id.0.to_string();

        let sessions = self.sessions.lock().await;
        let session = sessions.get(&session_id_str).ok_or_else(|| {
            error!("Session not found for cancel: {}", session_id_str);
            acp::Error::invalid_params()
        })?;

        session.cancel().await.map_err(|e| {
            error!("Failed to cancel session: {}", e);
            acp::Error::internal_error()
        })?;

        Ok(())
    }

    async fn set_session_mode(
        &self,
        args: acp::SetSessionModeRequest,
    ) -> Result<acp::SetSessionModeResponse, acp::Error> {
        info!("Received set_session_mode request: {:?}", args);
        // Not supported yet
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
