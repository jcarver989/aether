use acp_utils::client::{
    AcpEvent, AcpPromptHandle, AutoApproveClient, SpawnConfig, spawn_acp_session,
};
use agent_client_protocol as acp;
use tokio::sync::mpsc;

use crate::cli::Cli;
use crate::error::WispError;

pub struct AppState {
    pub session_id: acp::SessionId,
    pub agent_name: String,
    pub config_options: Vec<acp::SessionConfigOption>,
    pub event_rx: mpsc::UnboundedReceiver<AcpEvent>,
    pub prompt_handle: AcpPromptHandle,
}

impl AppState {
    pub async fn from_cli(cli: &Cli) -> Result<Self, WispError> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

        let config = SpawnConfig {
            agent_command: cli.agent.clone(),
            client_name: "wisp".to_string(),
            client_version: env!("CARGO_PKG_VERSION").to_string(),
            cwd,
        };

        let session = spawn_acp_session(config, AutoApproveClient::new)
            .await
            .map_err(WispError::Acp)?;

        Ok(Self {
            session_id: session.session_id,
            agent_name: session.agent_name,
            config_options: session.config_options,
            event_rx: session.event_rx,
            prompt_handle: session.prompt_handle,
        })
    }
}
