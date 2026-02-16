use agent_client_protocol as acp;
use tokio::sync::mpsc;

use crate::acp_connection::{AcpEvent, AcpPromptHandle, spawn_acp_session};
use crate::cli::Cli;
use crate::error::WispError;

pub struct AppState {
    pub session_id: acp::SessionId,
    pub event_rx: mpsc::UnboundedReceiver<AcpEvent>,
    pub prompt_handle: AcpPromptHandle,
}

impl AppState {
    pub async fn from_cli(cli: &Cli) -> Result<Self, WispError> {
        let session = spawn_acp_session(&cli.agent).await?;

        Ok(Self {
            session_id: session.session_id,
            event_rx: session.event_rx,
            prompt_handle: session.prompt_handle,
        })
    }
}
