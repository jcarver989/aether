use agent_client_protocol as acp;

/// Events forwarded from the ACP connection to the main event loop.
pub enum AcpEvent {
    SessionUpdate(Box<acp::SessionUpdate>),
    PromptDone(acp::StopReason),
    PromptError(acp::Error),
    ConnectionClosed,
}
