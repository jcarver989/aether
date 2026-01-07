//! Frontend types for tool display metadata.
//!
//! These types mirror the backend `ToolDisplayMeta` enum from mcp-lexicon
//! and are used to render human-friendly tool call displays in the UI.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Display metadata for bash/command tool results.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandDisplayMeta {
    /// The command that was executed
    pub command: String,
    /// Human-readable description of what the command does
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The exit code of the command
    pub exit_code: i32,
    /// Whether the command was killed due to timeout
    #[serde(default)]
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
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<usize>,
    /// Number of lines read
    #[serde(default)]
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
    #[serde(default)]
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
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_text: Option<String>,
    /// The new text that was inserted (abbreviated)
    #[serde(default)]
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
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_form: Option<String>,
}

impl TodoDisplayMeta {
    pub fn progress_label(&self) -> Option<String> {
        if self.items.is_empty() {
            return None;
        }
        let completed = self.items.iter().filter(|item| item.completed).count();
        Some(format!("({}/{})", completed, self.items.len()))
    }
}

/// Display metadata for list_files operations.
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

/// Display metadata for web_fetch operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WebFetchDisplayMeta {
    /// The URL that was fetched
    pub url: String,
    /// The title of the page (if available)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Size of the content in bytes
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

/// Display metadata for web_search operations.
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
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_count: Option<usize>,
}

/// Display metadata for spawn_subagent operations.
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
///
/// This enum matches the backend `ToolDisplayMeta` from mcp-lexicon
/// and is used to determine which specialized component to render.
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
    /// Parse display metadata from a tool result JSON value.
    ///
    /// Looks for `_meta.display` in the result and attempts to parse
    /// it into a `ToolDisplayMeta` enum.
    ///
    /// Returns `None` if:
    /// - The result is not valid JSON
    /// - The `_meta` field is missing
    /// - The `display` field is missing
    /// - Parsing fails
    pub fn from_result(result: &str) -> Option<Self> {
        let json: Value = serde_json::from_str(result).ok()?;
        let meta = json.get("_meta")?;
        let display = meta.get("display")?;
        serde_json::from_value(display.clone()).ok()
    }

    /// Try to extract display metadata from a result string.
    ///
    /// This is a convenience method that combines parsing and extraction.
    pub fn try_extract(result: Option<&str>) -> Option<Self> {
        result.and_then(Self::from_result)
    }

    /// Build a concise detail line for tool headers.
    pub fn detail_line(&self) -> Option<String> {
        match self {
            ToolDisplayMeta::Command(_) => None,
            ToolDisplayMeta::ReadFile(_) => None,
            ToolDisplayMeta::WriteFile(_) => None,
            ToolDisplayMeta::EditFile(_) => None,
            ToolDisplayMeta::Todo(todo) => todo.progress_label(),
            ToolDisplayMeta::ListFiles(list) => Some(format!(
                "{} ({})",
                list.path,
                format_count(list.count, "item", "items")
            )),
            ToolDisplayMeta::Grep(grep) => Some(format!(
                "{} in {} ({})",
                grep.pattern,
                grep.path,
                format_count(grep.match_count, "match", "matches")
            )),
            ToolDisplayMeta::Find(find) => Some(format!(
                "{} in {} ({})",
                find.pattern,
                find.path,
                format_count(find.count, "file", "files")
            )),
            ToolDisplayMeta::WebFetch(fetch) => Some(match fetch.title.as_deref() {
                Some(title) => format!("{title} · {}", fetch.url),
                None => fetch.url.clone(),
            }),
            ToolDisplayMeta::WebSearch(search) => Some(format!(
                "{} ({})",
                search.query,
                format_count(search.result_count, "result", "results")
            )),
            ToolDisplayMeta::LspSymbol(lsp) => {
                let detail = format!("{} {}", lsp.operation, lsp.symbol);
                if let Some(count) = lsp.result_count {
                    Some(format!(
                        "{detail} ({})",
                        format_count(count, "result", "results")
                    ))
                } else {
                    Some(detail)
                }
            }
            ToolDisplayMeta::SpawnSubAgent(sub_agent) => {
                if sub_agent.task_count > 1 {
                    Some(format!(
                        "{} (task {}/{})",
                        sub_agent.agent_name, sub_agent.task_index, sub_agent.task_count
                    ))
                } else {
                    Some(sub_agent.agent_name.clone())
                }
            }
        }
    }
}

fn format_count(count: usize, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("{count} {singular}")
    } else {
        format!("{count} {plural}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_command_display_meta() {
        let json = r#"{
            "type": "Command",
            "command": "cargo check",
            "description": "Check compilation",
            "exitCode": 0
        }"#;

        let meta: ToolDisplayMeta = serde_json::from_str(json).unwrap();
        match meta {
            ToolDisplayMeta::Command(cmd) => {
                assert_eq!(cmd.command, "cargo check");
                assert_eq!(cmd.description, Some("Check compilation".to_string()));
                assert_eq!(cmd.exit_code, 0);
            }
            _ => panic!("Expected Command variant"),
        }
    }

    #[test]
    fn test_parse_read_file_display_meta() {
        let json = r#"{
            "type": "ReadFile",
            "filePath": "/path/to/file.rs",
            "size": 1024,
            "lines": 50
        }"#;

        let meta: ToolDisplayMeta = serde_json::from_str(json).unwrap();
        match meta {
            ToolDisplayMeta::ReadFile(read) => {
                assert_eq!(read.file_path, "/path/to/file.rs");
                assert_eq!(read.size, Some(1024));
                assert_eq!(read.lines, Some(50));
            }
            _ => panic!("Expected ReadFile variant"),
        }
    }

    #[test]
    fn test_parse_todo_display_meta() {
        let json = r#"{
            "type": "Todo",
            "items": [
                {"content": "Task 1", "completed": false, "activeForm": "Working on task 1"},
                {"content": "Task 2", "completed": true}
            ]
        }"#;

        let meta: ToolDisplayMeta = serde_json::from_str(json).unwrap();
        match meta {
            ToolDisplayMeta::Todo(todo) => {
                assert_eq!(todo.items.len(), 2);
                assert_eq!(todo.items[0].content, "Task 1");
                assert!(!todo.items[0].completed);
                assert_eq!(
                    todo.items[0].active_form,
                    Some("Working on task 1".to_string())
                );
                assert_eq!(todo.items[1].content, "Task 2");
                assert!(todo.items[1].completed);
            }
            _ => panic!("Expected Todo variant"),
        }
    }

    #[test]
    fn test_todo_progress_label() {
        let meta = TodoDisplayMeta {
            items: vec![
                TodoItemMeta {
                    content: "Task 1".to_string(),
                    completed: true,
                    active_form: None,
                },
                TodoItemMeta {
                    content: "Task 2".to_string(),
                    completed: false,
                    active_form: None,
                },
            ],
        };

        assert_eq!(meta.progress_label(), Some("(1/2)".to_string()));
    }

    #[test]
    fn test_todo_progress_label_empty() {
        let meta = TodoDisplayMeta { items: vec![] };
        assert_eq!(meta.progress_label(), None);
    }

    #[test]
    fn test_detail_line_grep_includes_count() {
        let meta = ToolDisplayMeta::Grep(GrepDisplayMeta {
            pattern: "todo".to_string(),
            path: "src".to_string(),
            match_count: 2,
        });

        assert_eq!(
            meta.detail_line(),
            Some("todo in src (2 matches)".to_string())
        );
    }

    #[test]
    fn test_detail_line_command_is_none() {
        let meta = ToolDisplayMeta::Command(CommandDisplayMeta {
            command: "ls".to_string(),
            description: None,
            exit_code: 0,
            killed: None,
        });

        assert_eq!(meta.detail_line(), None);
    }

    #[test]
    fn test_from_result_with_meta() {
        let result = r#"{
            "status": "success",
            "_meta": {
                "display": {
                    "type": "Command",
                    "command": "ls -la",
                    "exitCode": 0
                }
            }
        }"#;

        let meta = ToolDisplayMeta::from_result(result);
        assert!(meta.is_some());
        match meta.unwrap() {
            ToolDisplayMeta::Command(cmd) => {
                assert_eq!(cmd.command, "ls -la");
            }
            _ => panic!("Expected Command variant"),
        }
    }

    #[test]
    fn test_from_result_without_meta() {
        let result = r#"{"status": "success"}"#;
        let meta = ToolDisplayMeta::from_result(result);
        assert!(meta.is_none());
    }

    #[test]
    fn test_from_result_invalid_json() {
        let result = "not valid json";
        let meta = ToolDisplayMeta::from_result(result);
        assert!(meta.is_none());
    }

    #[test]
    fn test_try_extract_with_some() {
        let result = Some(
            r#"{"_meta": {"display": {"type": "Command", "command": "echo", "exitCode": 0}}}"#,
        );
        let meta = ToolDisplayMeta::try_extract(result.as_deref());
        assert!(meta.is_some());
    }

    #[test]
    fn test_try_extract_with_none() {
        let meta = ToolDisplayMeta::try_extract(None);
        assert!(meta.is_none());
    }

    #[test]
    fn test_from_result_missing_display_field() {
        let result = r#"{
            "_meta": {
                "otherField": "value"
            }
        }"#;
        let meta = ToolDisplayMeta::from_result(result);
        assert!(meta.is_none());
    }

    #[test]
    fn test_from_result_invalid_enum_type() {
        let result = r#"{
            "_meta": {
                "display": {
                    "type": "InvalidType",
                    "command": "ls"
                }
            }
        }"#;
        let meta = ToolDisplayMeta::from_result(result);
        // Should fail to parse unknown enum variant
        assert!(meta.is_none());
    }

    #[test]
    fn test_from_result_missing_required_fields() {
        let result = r#"{
            "_meta": {
                "display": {
                    "type": "Command"
                }
            }
        }"#;
        let meta = ToolDisplayMeta::from_result(result);
        // Should fail because Command requires "command" field
        assert!(meta.is_none());
    }

    #[test]
    fn test_from_result_partial_read_file_meta() {
        let result = r#"{
            "_meta": {
                "display": {
                    "type": "ReadFile",
                    "filePath": "/path/to/file.rs"
                }
            }
        }"#;
        // This should succeed because size and lines are optional
        let meta = ToolDisplayMeta::from_result(result);
        assert!(meta.is_some());
        match meta.unwrap() {
            ToolDisplayMeta::ReadFile(read) => {
                assert_eq!(read.file_path, "/path/to/file.rs");
                assert_eq!(read.size, None);
                assert_eq!(read.lines, None);
            }
            _ => panic!("Expected ReadFile variant"),
        }
    }

    #[test]
    fn test_from_result_spawn_subagent() {
        let result = r#"{
            "_meta": {
                "display": {
                    "type": "SpawnSubAgent",
                    "agentName": "codebase-explorer",
                    "prompt": "Explore codebase",
                    "taskCount": 1,
                    "taskIndex": 1
                }
            }
        }"#;
        let meta = ToolDisplayMeta::from_result(result);
        assert!(meta.is_some());
        match meta.unwrap() {
            ToolDisplayMeta::SpawnSubAgent(sub_agent) => {
                assert_eq!(sub_agent.agent_name, "codebase-explorer");
                assert_eq!(sub_agent.prompt, "Explore codebase");
                assert_eq!(sub_agent.task_count, 1);
                assert_eq!(sub_agent.task_index, 1);
            }
            _ => panic!("Expected SpawnSubAgent variant"),
        }
    }

    #[test]
    fn test_from_result_spawn_subagent_multi_task() {
        let result = r#"{
            "_meta": {
                "display": {
                    "type": "SpawnSubAgent",
                    "agentName": "linear-task-planner",
                    "prompt": "Generate implementation plan",
                    "taskCount": 3,
                    "taskIndex": 2
                }
            }
        }"#;
        let meta = ToolDisplayMeta::from_result(result);
        assert!(meta.is_some());
        // Check detail_line() for multi-task scenario
        assert_eq!(
            meta.as_ref().unwrap().detail_line(),
            Some("linear-task-planner (task 2/3)".to_string())
        );
    }
}
