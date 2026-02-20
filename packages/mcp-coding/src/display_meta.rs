//! Display metadata for tool responses.
//!
//! This module provides types for generating human-readable display metadata
//! that can be sent alongside tool results via the MCP `_meta` field.

use serde::{Deserialize, Serialize};
use serde_json::json;

/// Display metadata for bash/command tool results.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandDisplayMeta {
    /// The command that was executed
    pub command: String,
    /// Human-readable description of what the command does
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The exit code of the command
    pub exit_code: i32,
    /// Whether the command was killed due to timeout
    #[serde(skip_serializing_if = "Option::is_none")]
    pub killed: Option<bool>,
}

/// Display metadata for file read operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReadFileDisplayMeta {
    /// The file path that was read
    pub file_path: String,
    /// The size of the file in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<usize>,
    /// Number of lines read
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines: Option<usize>,
}

/// Display metadata for file write operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WriteFileDisplayMeta {
    /// The file path that was written
    pub file_path: String,
    /// The size of the data written in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<usize>,
}

/// Display metadata for file edit operations (abbreviated diff).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EditFileDisplayMeta {
    /// The file path that was edited
    pub file_path: String,
    /// The text that was replaced (abbreviated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_text: Option<String>,
    /// The new text that was inserted (abbreviated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_text: Option<String>,
}

/// Display metadata for todo/task list operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TodoDisplayMeta {
    /// The todo list items with their status
    pub items: Vec<TodoItemMeta>,
}

/// A single todo item with its status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TodoItemMeta {
    /// The content of the todo item
    pub content: String,
    /// Whether the item is completed
    pub completed: bool,
    /// Optional active form (e.g., "Creating file" for "create file")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_form: Option<String>,
}

impl TodoItemMeta {
    pub fn new(content: impl Into<String>, completed: bool, active_form: Option<String>) -> Self {
        Self {
            content: content.into(),
            completed,
            active_form,
        }
    }
}

/// Display metadata for `list_files` operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ListFilesDisplayMeta {
    /// The directory that was listed
    pub path: String,
    /// Number of items found
    pub count: usize,
}

/// Display metadata for grep operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GrepDisplayMeta {
    /// The pattern that was searched
    pub pattern: String,
    /// The path that was searched
    pub path: String,
    /// Number of matches found
    pub match_count: usize,
}

/// Display metadata for find operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FindDisplayMeta {
    /// The glob pattern that was used
    pub pattern: String,
    /// The base path used for the search
    pub path: String,
    /// Number of files found
    pub count: usize,
}

/// Display metadata for `web_fetch` operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WebFetchDisplayMeta {
    /// The URL that was fetched
    pub url: String,
    /// The title of the page (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Size of the content in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

/// Display metadata for `web_search` operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchDisplayMeta {
    /// The search query
    pub query: String,
    /// Number of results returned
    pub result_count: usize,
}

/// Display metadata for LSP symbol operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LspSymbolDisplayMeta {
    /// The symbol name
    pub symbol: String,
    /// The operation performed
    pub operation: String,
    /// Number of results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_count: Option<usize>,
}

/// Display metadata for `spawn_subagent` operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpawnSubAgentDisplayMeta {
    /// The agent name being spawned
    pub agent_name: String,
    /// The prompt/task sent to the sub-agent
    pub prompt: String,
    /// Number of tasks in the batch
    pub task_count: usize,
    /// Current task index (1-based)
    pub task_index: usize,
}

/// Union of all display metadata types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum ToolDisplayMeta {
    Command(CommandDisplayMeta),
    ReadFile(ReadFileDisplayMeta),
    WriteFile(WriteFileDisplayMeta),
    EditFile(EditFileDisplayMeta),
    Todo(TodoDisplayMeta),
    ListFiles(ListFilesDisplayMeta),
    Grep(GrepDisplayMeta),
    Find(FindDisplayMeta),
    WebFetch(WebFetchDisplayMeta),
    WebSearch(WebSearchDisplayMeta),
    LspSymbol(LspSymbolDisplayMeta),
    SpawnSubAgent(SpawnSubAgentDisplayMeta),
}

impl ToolDisplayMeta {
    /// Create command display metadata from bash input and result.
    pub fn command(
        command: String,
        description: Option<String>,
        exit_code: i32,
        killed: Option<bool>,
    ) -> Self {
        ToolDisplayMeta::Command(CommandDisplayMeta {
            command,
            description,
            exit_code,
            killed,
        })
    }

    /// Create `read_file` display metadata.
    pub fn read_file(file_path: String, size: Option<usize>, lines: Option<usize>) -> Self {
        ToolDisplayMeta::ReadFile(ReadFileDisplayMeta {
            file_path,
            size,
            lines,
        })
    }

    /// Create `write_file` display metadata.
    pub fn write_file(file_path: String, size: Option<usize>) -> Self {
        ToolDisplayMeta::WriteFile(WriteFileDisplayMeta { file_path, size })
    }

    /// Create `edit_file` display metadata.
    pub fn edit_file(
        file_path: String,
        old_text: Option<String>,
        new_text: Option<String>,
    ) -> Self {
        ToolDisplayMeta::EditFile(EditFileDisplayMeta {
            file_path,
            old_text,
            new_text,
        })
    }

    /// Create todo display metadata.
    pub fn todo(items: Vec<TodoItemMeta>) -> Self {
        ToolDisplayMeta::Todo(TodoDisplayMeta { items })
    }

    /// Create todo display metadata for a single item.
    pub fn todo_single(
        content: impl Into<String>,
        completed: bool,
        active_form: Option<String>,
    ) -> Self {
        ToolDisplayMeta::todo(vec![TodoItemMeta::new(content, completed, active_form)])
    }

    /// Create `list_files` display metadata.
    pub fn list_files(path: String, count: usize) -> Self {
        ToolDisplayMeta::ListFiles(ListFilesDisplayMeta { path, count })
    }

    /// Create grep display metadata.
    pub fn grep(pattern: String, path: String, match_count: usize) -> Self {
        ToolDisplayMeta::Grep(GrepDisplayMeta {
            pattern,
            path,
            match_count,
        })
    }

    /// Create find display metadata.
    pub fn find(pattern: String, path: String, count: usize) -> Self {
        ToolDisplayMeta::Find(FindDisplayMeta {
            pattern,
            path,
            count,
        })
    }

    /// Create `web_fetch` display metadata.
    pub fn web_fetch(url: String, title: Option<String>, size: Option<u64>) -> Self {
        ToolDisplayMeta::WebFetch(WebFetchDisplayMeta { url, title, size })
    }

    /// Create `web_search` display metadata.
    pub fn web_search(query: String, result_count: usize) -> Self {
        ToolDisplayMeta::WebSearch(WebSearchDisplayMeta {
            query,
            result_count,
        })
    }

    /// Create `lsp_symbol` display metadata.
    pub fn lsp_symbol(symbol: String, operation: String, result_count: Option<usize>) -> Self {
        ToolDisplayMeta::LspSymbol(LspSymbolDisplayMeta {
            symbol,
            operation,
            result_count,
        })
    }

    /// Create `spawn_subagent` display metadata.
    pub fn spawn_subagent(
        agent_name: String,
        prompt: String,
        task_count: usize,
        task_index: usize,
    ) -> Self {
        ToolDisplayMeta::SpawnSubAgent(SpawnSubAgentDisplayMeta {
            agent_name,
            prompt,
            task_count,
            task_index,
        })
    }

    /// Convert this display metadata to a JSON object suitable for the `_meta` field.
    pub fn to_meta(&self) -> serde_json::Value {
        json!({
            "display": self
        })
    }

    /// Convert this display metadata into an optional `_meta` value.
    pub fn into_meta(self) -> Option<serde_json::Value> {
        Some(self.to_meta())
    }
}

/// Helper to truncate a string for display purposes.
///
/// Truncates the string to `max_length` characters, adding "..." if truncated.
pub fn truncate(s: &str, max_length: usize) -> String {
    if s.len() <= max_length {
        s.to_string()
    } else {
        let mut truncated = s
            .chars()
            .take(max_length.saturating_sub(3))
            .collect::<String>();
        truncated.push_str("...");
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_display_meta() {
        let meta = ToolDisplayMeta::command(
            "cargo check".to_string(),
            Some("Check compilation without building".to_string()),
            0,
            None,
        );
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["type"], "Command");
        assert_eq!(json["command"], "cargo check");
        assert_eq!(json["description"], "Check compilation without building");
        assert_eq!(json["exitCode"], 0);
    }

    #[test]
    fn test_read_file_display_meta() {
        let meta = ToolDisplayMeta::read_file("/path/to/file.rs".to_string(), Some(1024), Some(50));
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["type"], "ReadFile");
        assert_eq!(json["filePath"], "/path/to/file.rs");
        assert_eq!(json["size"], 1024);
        assert_eq!(json["lines"], 50);
    }

    #[test]
    fn test_todo_single_meta() {
        let meta = ToolDisplayMeta::todo_single("One task", false, None);
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["type"], "Todo");
        assert_eq!(json["items"][0]["content"], "One task");
        assert_eq!(json["items"][0]["completed"], false);
    }

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("short", 10), "short");
    }

    #[test]
    fn test_truncate_long() {
        let long = "cargo check --message-format=json --locked";
        let truncated = truncate(long, 20);
        assert!(truncated.len() <= 20);
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_to_meta() {
        let meta = ToolDisplayMeta::command("ls -la".to_string(), None, 0, None);
        let meta_json = meta.to_meta();
        assert!(meta_json.is_object());
        assert!(meta_json.get("display").is_some());
    }

    #[test]
    fn test_into_meta() {
        let meta = ToolDisplayMeta::command("ls -la".to_string(), None, 0, None);
        let meta_json = meta.into_meta().expect("meta should be present");
        assert!(meta_json.get("display").is_some());
    }
}
