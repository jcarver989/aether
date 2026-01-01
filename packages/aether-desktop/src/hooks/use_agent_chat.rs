//! Agent chat hook that manages all chat-related state and business logic.
//!
//! Separates business logic from the view, making the component cleaner
//! and the logic more testable.

use std::fmt::Debug;
use std::path::Path;
use std::sync::Arc;

use agent_client_protocol::{
    ContentBlock, EmbeddedResource, EmbeddedResourceResource, ResourceLink, TextResourceContents,
};
use dioxus::prelude::*;
use regex::Regex;
use std::sync::LazyLock;
use tokio::fs::read_to_string;
use tokio::sync::Mutex;

use crate::file_search::{FileMatch, FileSearcher};
use crate::state::{AgentStatus, Message, MessageKind, Role, SlashCommand, now_iso};
use crate::{AGENTS, FILE_SEARCHERS, HANDLES, with_agent_mut};

use super::use_autocomplete::AutocompleteController;

/// Matches `@query` at start of string or after whitespace, capturing the query.
static FILE_MENTION_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|\s)@(\S*)$").unwrap());

/// Input mode for the chat - determines which autocomplete (if any) is active.
#[derive(Clone, PartialEq)]
pub enum InputMode {
    /// Normal text input, no autocomplete
    Normal,
    /// Slash command autocomplete active
    SlashCommand(AutocompleteController<SlashCommand>),
    /// File mention autocomplete active
    FileMention(AutocompleteController<FileMatch>),
}

impl Debug for InputMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputMode::Normal => write!(f, "Normal"),
            InputMode::SlashCommand(ctrl) => {
                write!(f, "SlashCommand({} items)", ctrl.items().len())
            }
            InputMode::FileMention(ctrl) => {
                write!(f, "FileMention({} items)", ctrl.items().len())
            }
        }
    }
}

/// Controller for agent chat interactions.
///
/// Encapsulates all chat state and behavior, exposing a clean interface
/// for the view to consume.
///
/// This struct only contains `Signal` handles which are cheap to clone/copy.
/// We manually implement `Copy` because the derive macro requires inner types
/// to be `Copy`, but `Signal<T>` is always `Copy` regardless of `T`.
pub struct AgentChatController {
    agent_id: Signal<String>,

    /// Current input text
    pub input: Signal<String>,

    /// Files selected via @ mentions (pending attachment)
    pub pending_files: Signal<Vec<FileMatch>>,

    /// Whether file indexing is in progress
    pub files_loading: Signal<bool>,

    /// Current input mode (normal, slash command, or file mention)
    pub input_mode: Signal<InputMode>,

    /// File searcher instance
    file_searcher: Signal<Option<Arc<Mutex<FileSearcher>>>>,

    /// Available slash commands for this agent
    available_commands: Signal<Vec<SlashCommand>>,
}

impl AgentChatController {
    /// Get the agent ID.
    pub fn agent_id(&self) -> String {
        self.agent_id.read().clone()
    }

    /// Check if the agent is currently running.
    pub fn is_running(&self) -> bool {
        AGENTS
            .read()
            .get(&self.agent_id())
            .map(|a| matches!(a.status, AgentStatus::Running))
            .unwrap_or(false)
    }

    /// Get available slash commands.
    pub fn available_commands(&self) -> Vec<SlashCommand> {
        self.available_commands.read().clone()
    }

    /// Get current input value.
    pub fn input_value(&self) -> String {
        self.input.read().clone()
    }

    /// Get the current input mode.
    pub fn input_mode(&self) -> InputMode {
        self.input_mode.read().clone()
    }

    /// Handle text input changes.
    ///
    /// Detects `/` and `@` triggers for autocomplete.
    pub fn on_input_change(&mut self, value: String) {
        self.input.set(value.clone());

        let mode = if value.starts_with('/') && !value.contains(' ') {
            let filter = value.trim_start_matches('/').to_string();
            let filtered: Vec<SlashCommand> = self
                .available_commands
                .read()
                .iter()
                .filter(|cmd| {
                    filter.is_empty() || cmd.name.to_lowercase().contains(&filter.to_lowercase())
                })
                .cloned()
                .collect();

            let mut controller = AutocompleteController::new();
            controller.show(filter, filtered);
            InputMode::SlashCommand(controller)
        } else if let Some(caps) = FILE_MENTION_REGEX.captures(&value) {
            let query = caps.get(1).map_or("", |m| m.as_str()).to_string();
            let mut controller = AutocompleteController::new();

            controller.show(query.clone(), vec![]);

            if let Some(searcher) = self.file_searcher.read().clone() {
                let mut input_mode = self.input_mode;
                spawn(async move {
                    let mut searcher = searcher.lock().await;
                    let matches = searcher.search(&query, 10);
                    if let InputMode::FileMention(ref mut ctrl) = *input_mode.write() {
                        ctrl.set_items(matches);
                    }
                });
            }

            InputMode::FileMention(controller)
        } else {
            InputMode::Normal
        };

        self.input_mode.set(mode);
    }

    /// Remove a pending file by path.
    pub fn remove_pending_file(&mut self, path: &str) {
        self.pending_files.write().retain(|f| f.path != path);
    }

    /// Send the current message.
    pub fn send(&mut self) {
        let content = self.input.read().clone();
        let files = self.pending_files.read().clone();
        let agent_id = self.agent_id();

        if content.trim().is_empty() && files.is_empty() {
            return;
        }

        self.input_mode.set(InputMode::Normal);

        let display_content = if files.is_empty() {
            content.clone()
        } else {
            let file_list = files
                .iter()
                .map(|f| format!("@{}", f.path))
                .collect::<Vec<_>>()
                .join(" ");
            format!("{}\n\n{}", file_list, content)
        };

        with_agent_mut(&agent_id, |agent| {
            agent.messages.push(Message {
                id: uuid::Uuid::new_v4().to_string(),
                role: Role::User,
                content: display_content,
                kind: MessageKind::Text,
                timestamp: now_iso(),
                is_streaming: false,
            });
            agent.status = AgentStatus::Running;
        });

        self.input.set(String::new());
        self.pending_files.write().clear();

        spawn(async move {
            let supports_embedded = HANDLES.read().supports_embedded_context(&agent_id);
            let mut prompt: Vec<ContentBlock> = Vec::new();

            for file in &files {
                if supports_embedded {
                    match read_to_string(&file.absolute_path).await {
                        Ok(file_content) => {
                            prompt.push(file_to_embedded_resource(file, &file_content));
                        }
                        Err(e) => {
                            tracing::warn!("Failed to read file {}: {}", file.path, e);
                        }
                    }
                } else {
                    prompt.push(file_to_resource_link(file));
                }
            }

            if !content.trim().is_empty() {
                prompt.push(ContentBlock::from(content));
            }

            if let Err(e) = HANDLES.read().send_prompt(&agent_id, prompt) {
                tracing::error!("Failed to send message: {}", e);
                with_agent_mut(&agent_id, |agent| {
                    agent.status = AgentStatus::Error(e.to_string());
                });
            }
        });
    }
}

impl Clone for AgentChatController {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for AgentChatController {}

impl PartialEq for AgentChatController {
    fn eq(&self, other: &Self) -> bool {
        self.agent_id == other.agent_id
            && self.input == other.input
            && self.pending_files == other.pending_files
            && self.files_loading == other.files_loading
            && self.input_mode == other.input_mode
            && self.file_searcher == other.file_searcher
            && self.available_commands == other.available_commands
    }
}

impl Debug for AgentChatController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentChatController")
            .field("agent_id", &self.agent_id())
            .field("input", &self.input_value())
            .field("pending_files_count", &self.pending_files.read().len())
            .field("files_loading", &*self.files_loading.read())
            .field("input_mode", &self.input_mode())
            .finish_non_exhaustive()
    }
}

/// Create the agent chat controller hook.
///
/// Returns `None` if the agent doesn't exist.
pub fn use_agent_chat(agent_id: &str) -> Option<AgentChatController> {
    let agent_id_signal = use_signal(|| agent_id.to_string());

    // Check if agent exists and get initial data
    let (agent_cwd, initial_commands) = {
        let registry = AGENTS.read();
        let agent = registry.get(agent_id)?;
        (agent.cwd.clone(), agent.available_commands.clone())
    };

    let input = use_signal(String::new);
    let pending_files = use_signal(Vec::new);
    let files_loading = use_signal(|| false);
    let input_mode = use_signal(|| InputMode::Normal);

    let available_commands = use_signal(|| initial_commands);

    // Sync available_commands from global registry when it changes
    let agent_id_for_effect = agent_id.to_string();
    let mut available_commands_sync = available_commands;
    use_effect(move || {
        let registry = AGENTS.read();
        if let Some(agent) = registry.get(&agent_id_for_effect)
            && *available_commands_sync.read() != agent.available_commands
        {
            available_commands_sync.set(agent.available_commands.clone());
        }
    });

    // Get or create file searcher
    let file_searcher: Signal<Option<Arc<Mutex<FileSearcher>>>> =
        use_signal(|| Some(FILE_SEARCHERS.write().get_or_create(agent_cwd.clone())));

    // Index files on first render
    let mut files_loading_clone = files_loading;
    use_effect({
        let searcher = file_searcher.read().clone();
        move || {
            if let Some(searcher) = searcher.clone() {
                spawn(async move {
                    let mut searcher = searcher.lock().await;
                    if !searcher.is_indexed() {
                        files_loading_clone.set(true);
                        if let Err(e) = searcher.index_files() {
                            tracing::warn!("Failed to index files: {}", e);
                        }
                        files_loading_clone.set(false);
                    }
                });
            }
        }
    });

    Some(AgentChatController {
        agent_id: agent_id_signal,
        input,
        pending_files,
        files_loading,
        input_mode,
        file_searcher,
        available_commands,
    })
}

/// Build a ContentBlock::Resource with embedded file content.
fn file_to_embedded_resource(file: &FileMatch, content: &str) -> ContentBlock {
    ContentBlock::Resource(EmbeddedResource {
        annotations: None,
        resource: EmbeddedResourceResource::TextResourceContents(TextResourceContents {
            uri: format!("file://{}", file.absolute_path.display()),
            text: content.to_string(),
            mime_type: mime_from_path(&file.path),
            meta: None,
        }),
        meta: None,
    })
}

/// Build a ContentBlock::ResourceLink for a file reference.
fn file_to_resource_link(file: &FileMatch) -> ContentBlock {
    ContentBlock::ResourceLink(ResourceLink {
        uri: format!("file://{}", file.absolute_path.display()),
        name: file.path.clone(),
        size: Some(file.size as i64),
        mime_type: mime_from_path(&file.path),
        title: Some(file.path.clone()),
        description: None,
        annotations: None,
        meta: None,
    })
}

/// Infer MIME type from file extension.
fn mime_from_path(path: &str) -> Option<String> {
    let ext = Path::new(path).extension()?.to_str()?;
    let mime = match ext.to_lowercase().as_str() {
        "rs" => "text/x-rust",
        "py" => "text/x-python",
        "js" => "text/javascript",
        "ts" => "text/typescript",
        "tsx" => "text/typescript-jsx",
        "jsx" => "text/javascript-jsx",
        "json" => "application/json",
        "toml" => "text/x-toml",
        "yaml" | "yml" => "text/x-yaml",
        "md" => "text/markdown",
        "html" => "text/html",
        "css" => "text/css",
        "go" => "text/x-go",
        "java" => "text/x-java",
        "c" => "text/x-c",
        "cpp" | "cc" | "cxx" => "text/x-c++",
        "h" | "hpp" => "text/x-c-header",
        "sh" | "bash" => "text/x-shellscript",
        "sql" => "text/x-sql",
        "xml" => "text/xml",
        "txt" => "text/plain",
        _ => "text/plain",
    };
    Some(mime.to_string())
}
