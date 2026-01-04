//! Agent events for UI updates.
//!
//! These events are emitted by agents (real or fake) and consumed by the UI
//! to update state. This module is platform-agnostic.

use crate::platform::oneshot;
use crate::state::{AgentStatus, DiffState, TerminalStream};
use agent_client_protocol::{
    AvailableCommand, RequestPermissionRequest, RequestPermissionResponse, ToolCall,
    ToolCallUpdateFields,
};

/// Events emitted by an agent for UI consumption.
#[derive(Debug)]
pub enum AgentEvent {
    /// Append text chunk to the current streaming message
    MessageChunk { agent_id: String, text: String },
    /// Mark current streaming message as complete
    MessageComplete { agent_id: String },
    /// A new tool call started
    ToolCallStarted {
        agent_id: String,
        tool_id: String,
        tool_call: ToolCall,
    },
    /// Tool call fields updated (but not completed/failed)
    ToolCallUpdated {
        agent_id: String,
        tool_id: String,
        fields: ToolCallUpdateFields,
    },
    /// Tool call completed successfully
    ToolCallCompleted {
        agent_id: String,
        tool_id: String,
        result: String,
    },
    /// Tool call failed
    ToolCallFailed {
        agent_id: String,
        tool_id: String,
        error: String,
    },
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
    /// Available slash commands updated
    AvailableCommandsUpdate {
        agent_id: String,
        commands: Vec<AvailableCommand>,
    },
    /// Git diff state updated
    DiffUpdate {
        agent_id: String,
        diff_state: DiffState,
    },
    /// Terminal output chunk received from a spawned process
    TerminalOutput {
        agent_id: String,
        terminal_id: String,
        output: String,
        stream: TerminalStream,
    },
    /// Context usage updated
    ContextUsageUpdate {
        agent_id: String,
        usage_ratio: f64,
        tokens_used: u32,
        context_limit: u32,
    },
}

impl AgentEvent {
    /// Extract the agent_id from this event.
    pub fn agent_id(&self) -> &str {
        match self {
            AgentEvent::MessageChunk { agent_id, .. } => agent_id,
            AgentEvent::MessageComplete { agent_id } => agent_id,
            AgentEvent::ToolCallStarted { agent_id, .. } => agent_id,
            AgentEvent::ToolCallUpdated { agent_id, .. } => agent_id,
            AgentEvent::ToolCallCompleted { agent_id, .. } => agent_id,
            AgentEvent::ToolCallFailed { agent_id, .. } => agent_id,
            AgentEvent::StatusChange { agent_id, .. } => agent_id,
            AgentEvent::PermissionRequest { agent_id, .. } => agent_id,
            AgentEvent::Disconnected { agent_id } => agent_id,
            AgentEvent::Error { agent_id, .. } => agent_id,
            AgentEvent::AvailableCommandsUpdate { agent_id, .. } => agent_id,
            AgentEvent::DiffUpdate { agent_id, .. } => agent_id,
            AgentEvent::TerminalOutput { agent_id, .. } => agent_id,
            AgentEvent::ContextUsageUpdate { agent_id, .. } => agent_id,
        }
    }
}
