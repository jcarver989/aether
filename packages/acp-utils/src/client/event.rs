use acp::{Error, ExtNotification, SessionUpdate, StopReason};
use agent_client_protocol as acp;

/// Events forwarded from the ACP connection to the main event loop.
pub enum AcpEvent {
    SessionUpdate(Box<SessionUpdate>),
    ExtNotification(ExtNotification),
    PromptDone(StopReason),
    PromptError(Error),
    ConnectionClosed,
}
