//! Application state types for the desktop app.
//!
//! These types represent the UI state and are independent of the
//! underlying agent protocol (ACP).

use crate::acp_agent::AgentHandle;
use agent_client_protocol::{AvailableCommand, AvailableCommandInput, SessionId, ToolCall};
use std::collections::HashMap;

#[derive(Clone, PartialEq, Debug)]
pub enum AgentStatus {
    Idle,
    Running,
    Error(String),
}

#[derive(Clone, PartialEq, Debug)]
pub enum Role {
    User,
    Assistant,
    System,
}

#[derive(Clone, PartialEq, Debug)]
pub enum ToolCallStatus {
    Pending,
    Completed,
    Failed,
}

#[derive(Clone, PartialEq, Debug)]
pub enum MessageKind {
    Text,
    ToolCall {
        name: String,
        status: ToolCallStatus,
        result: Option<String>,
    },
}

#[derive(Clone, PartialEq, Debug)]
pub struct Message {
    pub id: String,
    pub role: Role,
    pub content: String,
    pub kind: MessageKind,
    pub timestamp: String,
    pub is_streaming: bool,
}

impl Message {
    pub fn user_text(content: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: Role::User,
            content: content.into(),
            kind: MessageKind::Text,
            timestamp: now_iso(),
            is_streaming: false,
        }
    }
}

/// Configuration for creating a new agent session.
#[derive(Clone, PartialEq, Debug)]
pub struct AgentConfig {
    /// Full command line for the agent (e.g., "aether-acp --model anthropic:claude-sonnet-4")
    pub command_line: String,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            command_line:
                "aether-acp --model anthropic:claude-sonnet-4-20250514 --mcp-config mcp.json"
                    .to_string(),
        }
    }
}

/// A slash command available for this agent session.
///
/// This is a UI-friendly wrapper around `AvailableCommand` that extracts
/// the input hint for easier display.
#[derive(Clone, PartialEq, Debug)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
    pub input_hint: Option<String>,
}

impl From<AvailableCommand> for SlashCommand {
    fn from(cmd: AvailableCommand) -> Self {
        let input_hint = cmd.input.and_then(|input| match input {
            AvailableCommandInput::Unstructured { hint } => Some(hint),
        });
        Self {
            name: cmd.name,
            description: cmd.description,
            input_hint,
        }
    }
}

/// Represents an active agent session in the UI.
///
/// This struct holds UI state only (messages, status, name, config).
/// Runtime handles (child process, tasks, command channel) are stored
/// separately in `AgentHandles`.
#[derive(Clone, PartialEq, Debug)]
pub struct AgentSession {
    /// Unique identifier for this agent (UUIDv4) - used for UI routing and state
    pub id: String,
    /// ACP session ID - used only for protocol communication with the child process
    pub acp_session_id: SessionId,
    /// Display name
    pub name: String,
    /// Configuration used to create this session
    pub config: AgentConfig,
    /// Current status
    pub status: AgentStatus,
    /// Message history
    pub messages: Vec<Message>,
    /// Tracks in-flight tool calls for correlating ToolCall → ToolCallUpdate
    pub tool_calls: HashMap<String, ToolCall>,
    /// Available slash commands for this agent
    pub available_commands: Vec<SlashCommand>,
}

/// Error returned when attempting to send to a disconnected agent.
#[derive(Debug)]
pub enum SendError {
    NotConnected,
    ChannelClosed,
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendError::NotConnected => write!(f, "Agent not connected"),
            SendError::ChannelClosed => write!(f, "Agent channel closed"),
        }
    }
}

impl std::error::Error for SendError {}

impl AgentSession {
    /// Create a new agent session.
    ///
    /// The `id` is a locally-generated UUID for UI routing/state.
    /// The `acp_session_id` is the session ID from the ACP protocol.
    pub fn new(
        id: String,
        acp_session_id: SessionId,
        config: AgentConfig,
        initial_message: String,
    ) -> Self {
        Self {
            id,
            acp_session_id,
            name: generate_agent_name(),
            config,
            status: AgentStatus::Running,
            messages: vec![Message::user_text(initial_message)],
            tool_calls: HashMap::new(),
            available_commands: Vec::new(),
        }
    }
}

pub fn now_iso() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

static AGENT_NAMES: &[&str] = &[
    "Atlas", "Nova", "Cipher", "Echo", "Flux", "Helix", "Iris", "Jade", "Kite", "Luna", "Mist",
    "Nexus", "Onyx", "Pulse", "Quark", "Raven", "Sage", "Terra", "Unity", "Vortex", "Wave",
    "Xenon", "Zephyr", "Aura",
];

static AGENT_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn generate_agent_name() -> String {
    let count = AGENT_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let name = AGENT_NAMES[count % AGENT_NAMES.len()];
    if count >= AGENT_NAMES.len() {
        format!("{} {}", name, count / AGENT_NAMES.len() + 1)
    } else {
        name.to_string()
    }
}

/// Collection of agent runtime handles.
///
/// Stores the actual agent handles (child process, tasks, command channel)
/// separately from the UI state. This allows `AgentSession` to remain
/// `Clone` and `PartialEq` while keeping runtime resources properly managed.
///
/// This is used inside a `GlobalSignal<AgentHandles>`, so mutability comes
/// from the signal's `write()` method rather than internal `RefCell`.
pub struct AgentHandles {
    /// Maps agent UUID to its runtime handle
    handles: HashMap<String, AgentHandle>,
}

impl AgentHandles {
    pub fn new() -> Self {
        Self {
            handles: HashMap::new(),
        }
    }

    /// Insert a new agent handle, keyed by its UUID.
    pub fn insert(&mut self, handle: AgentHandle) {
        self.handles.insert(handle.id.clone(), handle);
    }

    /// Send a prompt to an agent by its UUID.
    pub fn send_prompt(&self, agent_id: &str, message: String) -> Result<(), SendError> {
        match self.handles.get(agent_id) {
            Some(handle) => handle.send_prompt(message),
            None => Err(SendError::NotConnected),
        }
    }

    /// Remove an agent handle by its UUID.
    pub fn remove(&mut self, agent_id: &str) -> Option<AgentHandle> {
        self.handles.remove(agent_id)
    }
}

impl Default for AgentHandles {
    fn default() -> Self {
        Self::new()
    }
}
