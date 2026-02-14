//! ACP session management.
//!
//! This module provides types and functions for establishing ACP sessions
//! with agents.

use crate::client::AcpClient;
use agent_client_protocol::{
    Agent, AgentCapabilities, ClientSideConnection, InitializeRequest, NewSessionRequest,
    ProtocolVersion, SessionId,
};
use std::path::PathBuf;

/// Error type for session operations.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("Initialization failed: {0}")]
    InitFailed(String),
    #[error("Session creation failed: {0}")]
    SessionFailed(String),
}

/// Information returned from session initialization.
///
/// Contains both the session ID and agent capabilities needed by the client.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// ACP session ID for protocol communication.
    pub session_id: SessionId,
    /// Agent capabilities returned from initialization.
    pub agent_capabilities: AgentCapabilities,
}

/// Initialize a session with an ACP agent.
///
/// Performs the ACP handshake:
/// 1. Sends `InitializeRequest` with client capabilities
/// 2. Receives agent capabilities
/// 3. Sends `NewSessionRequest` with working directory
/// 4. Returns session ID and capabilities
///
/// # Arguments
/// * `conn` - The established connection to the agent
/// * `cwd` - Working directory for the session
///
/// # Returns
/// `SessionInfo` containing the session ID and agent capabilities
pub async fn start_session(
    conn: &ClientSideConnection,
    cwd: PathBuf,
) -> Result<SessionInfo, SessionError> {
    let init_req = InitializeRequest::new(ProtocolVersion::LATEST)
        .client_capabilities(AcpClient::capabilities());

    let init_response = conn
        .initialize(init_req)
        .await
        .map_err(|e| SessionError::InitFailed(e.to_string()))?;

    let session_req = NewSessionRequest::new(cwd);

    let session_response = conn
        .new_session(session_req)
        .await
        .map_err(|e| SessionError::SessionFailed(e.to_string()))?;

    Ok(SessionInfo {
        session_id: session_response.session_id,
        agent_capabilities: init_response.agent_capabilities,
    })
}
