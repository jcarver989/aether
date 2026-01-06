//! Agent events for UI updates.
//!
//! These events are emitted by agents (real or fake) and consumed by the UI
//! to update state. This module is platform-agnostic.

use crate::components::tool_display::SubAgentStreamMessage;
use crate::platform::oneshot;
use crate::state::{AgentStatus, DiffState, McpServerStatus};
use aether_acp_client::transform::AcpEvent;
use agent_client_protocol::{
    RequestPermissionRequest, RequestPermissionResponse,
};

/// Top-level application events.
#[derive(Debug)]
pub enum AppEvent {
    /// Agent-related event
    Agent(AgentEvent),
    /// MCP server-related event
    Mcp(McpEvent),
}

impl From<AgentEvent> for AppEvent {
    fn from(event: AgentEvent) -> Self {
        AppEvent::Agent(event)
    }
}

impl From<McpEvent> for AppEvent {
    fn from(event: McpEvent) -> Self {
        AppEvent::Mcp(event)
    }
}

/// Events related to MCP server connections.
#[derive(Debug, Clone)]
pub enum McpEvent {
    /// MCP server status changed
    StatusChanged {
        server_name: String,
        status: McpServerStatus,
    },
    /// Start OAuth flow for an MCP server
    StartOAuthFlow {
        server_name: String,
        base_url: String,
    },
    /// OAuth flow completed successfully
    OAuthFlowCompleted { server_name: String },
    /// OAuth flow failed
    OAuthFlowFailed { server_name: String, error: String },
}

/// Events emitted by an agent for UI consumption.
#[derive(Debug)]
pub enum AgentEvent {
    /// Protocol events from ACP, wrapped with agent_id for routing
    Protocol { agent_id: String, event: AcpEvent },
    /// Agent status changed
    StatusChange {
        agent_id: String,
        status: AgentStatus,
    },
    /// Permission request needs user response
    PermissionRequest {
        #[allow(dead_code)]
        agent_id: String,
        request: RequestPermissionRequest,
        response_tx: oneshot::Sender<RequestPermissionResponse>,
    },
    /// Agent disconnected
    Disconnected { agent_id: String },
    /// Error occurred
    #[allow(dead_code)]
    Error { agent_id: String, error: String },
    /// Git diff state updated
    DiffUpdate {
        agent_id: String,
        diff_state: DiffState,
    },
    /// Progress update from a streaming sub-agent
    SubAgentProgress {
        agent_id: String,
        parent_tool_id: String,
        sub_agent_id: String,
        agent_name: String,
        message: SubAgentStreamMessage,
    },
}

impl AgentEvent {
    /// Extract the agent_id from this event.
    pub fn agent_id(&self) -> &str {
        match self {
            AgentEvent::Protocol { agent_id, .. } => agent_id,
            AgentEvent::StatusChange { agent_id, .. } => agent_id,
            AgentEvent::PermissionRequest { agent_id, .. } => agent_id,
            AgentEvent::Disconnected { agent_id } => agent_id,
            AgentEvent::Error { agent_id, .. } => agent_id,
            AgentEvent::DiffUpdate { agent_id, .. } => agent_id,
            AgentEvent::SubAgentProgress { agent_id, .. } => agent_id,
        }
    }
}
