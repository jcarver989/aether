use acp::{Error, ExtNotification, SessionUpdate, StopReason};
use agent_client_protocol as acp;
use tokio::sync::oneshot;

use crate::notifications::{ElicitationParams, ElicitationResponse};

/// Events forwarded from the ACP connection to the main event loop.
pub enum AcpEvent {
    SessionUpdate(Box<SessionUpdate>),
    ExtNotification(ExtNotification),
    ElicitationRequest {
        params: ElicitationParams,
        response_tx: oneshot::Sender<ElicitationResponse>,
    },
    PromptDone(StopReason),
    PromptError(Error),
    ConnectionClosed,
}
