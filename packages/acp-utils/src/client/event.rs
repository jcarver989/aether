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
    AuthenticateComplete {
        method_id: String,
    },
    AuthenticateFailed {
        method_id: String,
        error: String,
    },
    SessionsListed {
        sessions: Vec<acp::SessionInfo>,
    },
    SessionLoaded {
        session_id: acp::SessionId,
        config_options: Vec<acp::SessionConfigOption>,
    },
    ConnectionClosed,
}
