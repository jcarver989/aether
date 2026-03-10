use crate::cli::Cli;
use crate::error::WispError;
use crate::settings::load_or_create_settings;
use crate::tui::Theme;
use acp_utils::client::{AcpEvent, AcpPromptHandle, AutoApproveClient, spawn_acp_session};
use agent_client_protocol::{
    self as acp, Implementation, InitializeRequest, NewSessionRequest, ProtocolVersion,
};
use std::env::current_dir;
use tokio::sync::mpsc;

pub struct RuntimeState {
    pub session_id: acp::SessionId,
    pub agent_name: String,
    pub config_options: Vec<acp::SessionConfigOption>,
    pub auth_methods: Vec<acp::AuthMethod>,
    pub theme: Theme,
    pub event_rx: mpsc::UnboundedReceiver<AcpEvent>,
    pub prompt_handle: AcpPromptHandle,
    pub working_dir: std::path::PathBuf,
}

impl RuntimeState {
    pub async fn from_cli(cli: &Cli) -> Result<Self, WispError> {
        let cwd = current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let new_session_request = NewSessionRequest::new(cwd.clone());
        let init_request = InitializeRequest::new(ProtocolVersion::LATEST)
            .client_info(Implementation::new("wisp", env!("CARGO_PKG_VERSION")));

        let session = spawn_acp_session(
            &cli.agent,
            init_request,
            new_session_request,
            AutoApproveClient::new,
        )
        .await
        .map_err(WispError::Acp)?;

        let settings = load_or_create_settings();

        Ok(Self {
            session_id: session.session_id,
            agent_name: session.agent_name,
            config_options: session.config_options,
            auth_methods: session.auth_methods,
            theme: crate::settings::load_theme(&settings),
            event_rx: session.event_rx,
            prompt_handle: session.prompt_handle,
            working_dir: cwd,
        })
    }
}
