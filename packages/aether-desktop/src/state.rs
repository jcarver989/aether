//! Application state types for the desktop app.
//!
//! These types represent the UI state and are independent of the
//! underlying agent protocol (ACP).

use crate::error::AetherDesktopError;
use crate::platform::{AgentHandle, DockerProgress};

use agent_client_protocol::{
    AvailableCommand, AvailableCommandInput, ContentBlock, SessionId, ToolCall,
};
use dioxus::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, PartialEq, Debug)]
pub enum AgentStatus {
    Idle,
    Starting(DockerProgress),
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

/// Identifies which output stream a terminal line came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TerminalStream {
    Stdout,
    Stderr,
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

/// Execution mode for an agent session.
#[derive(Clone, PartialEq, Debug, Default)]
pub enum ExecutionMode {
    /// Run agent as a local process.
    #[default]
    Local,
    /// Run agent in a Docker container with copy-on-write isolation.
    Docker {
        /// Path to the Dockerfile used for this session.
        dockerfile_path: std::path::PathBuf,
    },
}

impl ExecutionMode {
    /// Returns true if this is Docker execution mode.
    pub fn is_docker(&self) -> bool {
        matches!(self, ExecutionMode::Docker { .. })
    }
}

/// Configuration for creating a new agent session.
#[derive(Clone, PartialEq, Debug)]
pub struct AgentConfig {
    /// Display name for the agent (e.g., "Claude", "Aether", or command basename)
    pub name: String,
    /// Full command line for the agent (e.g., "aether-acp --model anthropic:claude-sonnet-4")
    pub command_line: String,
    /// Execution mode - local or Docker.
    pub execution_mode: ExecutionMode,
    /// Project directory path (used as workspace root for MCP servers)
    pub project_path: PathBuf,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            name: "Aether".to_string(),
            command_line:
                "aether-acp --model anthropic:claude-sonnet-4-20250514 --mcp-config mcp.json"
                    .to_string(),
            execution_mode: ExecutionMode::Local,
            project_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
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
        let input_hint = cmd.input.map(|input| match input {
            AvailableCommandInput::Unstructured { hint } => hint,
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
    /// Working directory for this agent
    pub cwd: PathBuf,
    /// Git diff state for this agent
    pub diff_state: DiffState,
    /// Maps terminal_id → tool_id for correlating streaming terminal output
    pub terminal_to_tool: HashMap<String, String>,
    /// Current context usage ratio (0.0 - 1.0)
    pub context_usage: f64,
    /// Tokens currently used in context
    pub tokens_used: u32,
    /// Maximum context limit for this agent
    pub context_limit: u32,
}

impl AgentSession {
    /// Create a new agent session.
    ///
    /// The `id` is a locally-generated UUID for UI routing/state.
    /// The `acp_session_id` is the session ID from the ACP protocol (can be empty for pending sessions).
    pub fn new(
        id: String,
        acp_session_id: SessionId,
        config: AgentConfig,
        initial_message: String,
        cwd: PathBuf,
        status: AgentStatus,
    ) -> Self {
        let name = config.name.clone();
        Self {
            id,
            acp_session_id,
            name,
            config,
            status,
            messages: vec![Message::user_text(initial_message)],
            tool_calls: HashMap::new(),
            available_commands: Vec::new(),
            cwd,
            diff_state: DiffState::default(),
            terminal_to_tool: HashMap::new(),
            context_usage: 0.0,
            tokens_used: 0,
            context_limit: 0,
        }
    }

    /// Get the first user message content, if any.
    pub fn first_user_message(&self) -> Option<&str> {
        self.messages.first().map(|m| m.content.as_str())
    }
}

pub fn now_iso() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
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
    ///
    /// The prompt is a vector of ContentBlocks which can include text, resources, or resource links.
    pub fn send_prompt(
        &self,
        agent_id: &str,
        prompt: Vec<ContentBlock>,
    ) -> Result<(), AetherDesktopError> {
        match self.handles.get(agent_id) {
            Some(handle) => handle.send_prompt(prompt),
            None => Err(AetherDesktopError::SendNotConnected),
        }
    }

    /// Check if an agent supports embedded context in prompts.
    pub fn supports_embedded_context(&self, agent_id: &str) -> bool {
        self.handles
            .get(agent_id)
            .map(|h| h.agent_capabilities.prompt_capabilities.embedded_context)
            .unwrap_or(false)
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

/// Status of a file in the git diff.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
}

/// Unique key for identifying a comment location.
/// Tuple of (file_path, line_number).
pub type CommentKey = (String, u32);

/// A comment on a diff line.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DiffComment {
    /// Path to the file being commented on
    pub file_path: String,
    /// Line number in the new version (or old version for deletions)
    pub line_number: u32,
    /// The type of line being commented on
    pub line_origin: LineOrigin,
    /// The content of the comment
    pub content: String,
    /// The original line content for context
    pub line_content: String,
    /// Timestamp when the comment was created
    pub created_at: String,
}

impl DiffComment {
    pub fn new(
        file_path: String,
        line_number: u32,
        line_origin: LineOrigin,
        content: String,
        line_content: String,
    ) -> Self {
        Self {
            file_path,
            line_number,
            line_origin,
            content,
            line_content,
            created_at: now_iso(),
        }
    }

    /// Returns the comment key for this comment.
    pub fn key(&self) -> CommentKey {
        (self.file_path.clone(), self.line_number)
    }
}

/// Origin/type of a diff line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineOrigin {
    Context,
    Addition,
    Deletion,
}

/// A single line in a diff hunk.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DiffLine {
    pub origin: LineOrigin,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    pub content: String,
}

/// A contiguous section of changes in a file.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

/// Diff information for a single file.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FileDiff {
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub hunks: Vec<DiffHunk>,
}

/// State for the git diff view.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DiffState {
    pub files: Vec<FileDiff>,
    pub selected_file: Option<String>,
    pub loading: bool,
    pub error: Option<String>,
    /// Comments on diff lines, keyed by (file_path, line_number)
    pub comments: HashMap<CommentKey, DiffComment>,
    /// Whether changes are ephemeral (Docker mode - not persisted to host)
    pub is_ephemeral: bool,
}

impl DiffState {
    /// Add a comment to the diff.
    pub fn add_comment(&mut self, comment: DiffComment) {
        let key = comment.key();
        self.comments.insert(key, comment);
    }

    /// Remove a comment from the diff.
    pub fn remove_comment(&mut self, key: &CommentKey) {
        self.comments.remove(key);
    }

    /// Update a comment's content in place. Returns true if the comment was found and updated.
    pub fn update_comment(&mut self, key: &CommentKey, new_content: String) -> bool {
        if let Some(comment) = self.comments.get_mut(key) {
            comment.content = new_content;
            true
        } else {
            false
        }
    }

    /// Clear all comments from the diff.
    pub fn clear_comments(&mut self) {
        self.comments.clear();
    }

    /// Get comments for a specific file.
    pub fn comments_for_file(&self, file_path: &str) -> Vec<&DiffComment> {
        self.comments
            .values()
            .filter(|c| c.file_path == file_path)
            .collect()
    }

    /// Generate a prompt from all comments.
    ///
    /// Groups comments by file and formats them in a way that's
    /// easy for an agent to understand and act on.
    pub fn generate_prompt(&self) -> String {
        generate_comments_prompt(&self.comments)
    }
}

/// Generate a prompt from a collection of comments.
///
/// Groups comments by file and formats them in a way that's
/// easy for an agent to understand and act on.
pub fn generate_comments_prompt(comments: &HashMap<CommentKey, DiffComment>) -> String {
    if comments.is_empty() {
        return String::new();
    }

    // Group comments by file
    let mut by_file: HashMap<&str, Vec<&DiffComment>> = HashMap::new();
    for comment in comments.values() {
        by_file.entry(&comment.file_path).or_default().push(comment);
    }

    // Sort files for consistent ordering
    let mut files: Vec<_> = by_file.keys().cloned().collect();
    files.sort();

    let mut prompt = String::from("Please make the following changes:\n\n");
    let mut index = 1;

    for file_path in files {
        let Some(comments) = by_file.get_mut(file_path) else {
            continue;
        };
        // Sort comments by line number
        comments.sort_by_key(|c| c.line_number);

        for comment in comments.iter() {
            let origin_marker = match comment.line_origin {
                LineOrigin::Addition => "+",
                LineOrigin::Deletion => "-",
                LineOrigin::Context => " ",
            };
            let line_content = comment.line_content.trim();

            prompt.push_str(&format!(
                "{}. In `{}` at line {}:\n",
                index, comment.file_path, comment.line_number
            ));
            prompt.push_str(&format!("   > {}{}\n", origin_marker, line_content));
            prompt.push_str(&format!("   Comment: {}\n\n", comment.content));
            index += 1;
        }
    }

    prompt
}

/// Indexed registry for agent sessions.
///
/// Uses HashMap for O(1) lookups while maintaining insertion order via Vec
/// for UI display purposes (sidebar listing).
#[derive(Clone, Default)]
pub struct AgentRegistry {
    /// Maps agent ID to its session
    agents: HashMap<String, AgentSession>,
    /// Maintains insertion order for UI display
    order: Vec<String>,
}

impl std::fmt::Debug for AgentRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRegistry")
            .field("agent_ids", &self.order)
            .finish()
    }
}

impl AgentRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a reference to a specific agent - O(1).
    pub fn get(&self, id: &str) -> Option<&AgentSession> {
        self.agents.get(id)
    }

    /// Get a mutable reference to a specific agent - O(1).
    pub fn get_mut(&mut self, id: &str) -> Option<&mut AgentSession> {
        self.agents.get_mut(id)
    }

    /// Iterate over agent sessions in insertion order.
    pub fn iter_ordered(&self) -> impl Iterator<Item = &AgentSession> + '_ {
        self.order.iter().filter_map(|id| self.agents.get(id))
    }

    /// Insert a new agent session.
    pub fn insert(&mut self, session: AgentSession) {
        let id = session.id.clone();
        self.order.push(id.clone());
        self.agents.insert(id, session);
    }

    /// Remove an agent by ID.
    pub fn remove(&mut self, id: &str) -> Option<AgentSession> {
        self.order.retain(|x| x != id);
        self.agents.remove(id)
    }

    /// Get the number of agents.
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_comment(file_path: &str, line_number: u32, content: &str) -> DiffComment {
        DiffComment {
            file_path: file_path.to_string(),
            line_number,
            line_origin: LineOrigin::Addition,
            content: content.to_string(),
            line_content: format!("let x = {};", line_number),
            created_at: "2025-01-01T00:00:00.000Z".to_string(),
        }
    }

    #[test]
    fn test_add_comment() {
        let mut state = DiffState::default();
        let comment = make_comment("src/main.rs", 42, "This needs a type annotation");

        state.add_comment(comment.clone());

        assert_eq!(state.comments.len(), 1);
        let key = ("src/main.rs".to_string(), 42);
        assert!(state.comments.contains_key(&key));
        assert_eq!(
            state.comments.get(&key).unwrap().content,
            "This needs a type annotation"
        );
    }

    #[test]
    fn test_remove_comment() {
        let mut state = DiffState::default();
        let comment = make_comment("src/main.rs", 42, "Remove this");

        state.add_comment(comment);
        assert_eq!(state.comments.len(), 1);

        let key = ("src/main.rs".to_string(), 42);
        state.remove_comment(&key);

        assert!(state.comments.is_empty());
    }

    #[test]
    fn test_clear_comments() {
        let mut state = DiffState::default();
        state.add_comment(make_comment("src/main.rs", 10, "Comment 1"));
        state.add_comment(make_comment("src/main.rs", 20, "Comment 2"));
        state.add_comment(make_comment("src/lib.rs", 5, "Comment 3"));

        assert_eq!(state.comments.len(), 3);

        state.clear_comments();

        assert!(state.comments.is_empty());
    }

    #[test]
    fn test_comments_for_file() {
        let mut state = DiffState::default();
        state.add_comment(make_comment("src/main.rs", 10, "Main comment 1"));
        state.add_comment(make_comment("src/main.rs", 20, "Main comment 2"));
        state.add_comment(make_comment("src/lib.rs", 5, "Lib comment"));

        let main_comments = state.comments_for_file("src/main.rs");
        assert_eq!(main_comments.len(), 2);

        let lib_comments = state.comments_for_file("src/lib.rs");
        assert_eq!(lib_comments.len(), 1);

        let other_comments = state.comments_for_file("src/other.rs");
        assert!(other_comments.is_empty());
    }

    #[test]
    fn test_generate_prompt_empty() {
        let state = DiffState::default();
        let prompt = state.generate_prompt();
        assert!(prompt.is_empty());
    }

    #[test]
    fn test_generate_prompt_single_comment() {
        let mut state = DiffState::default();
        state.add_comment(make_comment("src/main.rs", 42, "Add error handling"));

        let prompt = state.generate_prompt();

        assert!(prompt.contains("Please make the following changes:"));
        assert!(prompt.contains("1. In `src/main.rs` at line 42:"));
        assert!(prompt.contains("Comment: Add error handling"));
    }

    #[test]
    fn test_generate_prompt_multiple_files() {
        let mut state = DiffState::default();
        state.add_comment(make_comment("src/main.rs", 10, "Fix main"));
        state.add_comment(make_comment("src/lib.rs", 5, "Fix lib"));
        state.add_comment(make_comment("src/main.rs", 20, "Another main fix"));

        let prompt = state.generate_prompt();

        // Should contain all comments
        assert!(prompt.contains("Fix main"));
        assert!(prompt.contains("Fix lib"));
        assert!(prompt.contains("Another main fix"));

        // Comments should be grouped by file and sorted
        // lib.rs comes before main.rs alphabetically
        let lib_pos = prompt.find("src/lib.rs").unwrap();
        let main_pos = prompt.find("src/main.rs").unwrap();
        assert!(lib_pos < main_pos);
    }

    #[test]
    fn test_comment_key_uniqueness() {
        let mut state = DiffState::default();
        state.add_comment(make_comment("src/main.rs", 42, "First comment"));
        state.add_comment(make_comment("src/main.rs", 42, "Second comment"));

        // Same key overwrites the previous comment
        assert_eq!(state.comments.len(), 1);
        let key = ("src/main.rs".to_string(), 42);
        assert_eq!(state.comments.get(&key).unwrap().content, "Second comment");
    }

    #[test]
    fn test_diff_comment_key() {
        let comment = make_comment("src/foo.rs", 100, "Test");
        let key = comment.key();

        assert_eq!(key.0, "src/foo.rs");
        assert_eq!(key.1, 100);
    }

    #[test]
    fn test_update_comment() {
        let mut state = DiffState::default();
        state.add_comment(make_comment("src/main.rs", 42, "Original content"));

        let key = ("src/main.rs".to_string(), 42);
        let updated = state.update_comment(&key, "Updated content".to_string());

        assert!(updated);
        assert_eq!(state.comments.get(&key).unwrap().content, "Updated content");
    }

    #[test]
    fn test_update_comment_missing_key() {
        let mut state = DiffState::default();
        state.add_comment(make_comment("src/main.rs", 42, "Some content"));

        let missing_key = ("src/other.rs".to_string(), 99);
        let updated = state.update_comment(&missing_key, "New content".to_string());

        assert!(!updated);
        // Original comment should be unchanged
        let key = ("src/main.rs".to_string(), 42);
        assert_eq!(state.comments.get(&key).unwrap().content, "Some content");
    }
}
