use lsp_types::Uri;
use rmcp::{
    ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use lsp::{LspClient, path_to_uri};

pub mod bash;
pub mod common;
pub mod default_tools;
pub mod edit_file;
pub mod find;
pub mod grep;
pub mod list_files;
pub mod lsp;
pub mod lsp_tool;
pub mod read_file;
pub mod todo_write;
pub mod tools_trait;
pub mod write_file;

pub use bash::{
    BackgroundProcessHandle, BashInput, BashOutput, BashResult, ReadBackgroundBashInput,
    ReadBackgroundBashOutput, execute_command, read_background_bash,
};
pub use default_tools::DefaultCodingTools;
pub use edit_file::{EditFileArgs, EditFileResponse, edit_file_contents};
pub use find::{FindInput, FindOutput, find_files_by_name};
pub use grep::{GrepInput, GrepOutput, perform_grep};
pub use list_files::{ListFilesArgs, ListFilesResult, list_files};
pub use read_file::{ReadFileArgs, ReadFileResult, read_file_contents};
pub use todo_write::{TodoItem, TodoStatus, TodoWriteInput, TodoWriteOutput, process_todo_write};
pub use tools_trait::CodingTools;
pub use write_file::{WriteFileArgs, WriteFileResponse, write_file_contents};
pub use lsp_tool::{LspInput, LspOperation, LspOutput, execute_lsp_operation};

#[derive(Debug)]
pub struct CodingMcp<T: CodingTools = DefaultCodingTools> {
    tool_router: ToolRouter<Self>,
    background_processes: Mutex<HashMap<String, BackgroundProcessHandle>>,
    todos: Mutex<Vec<TodoItem>>,
    /// Track files that have been read to enforce read-before-edit safety
    files_read: Mutex<HashSet<String>>,
    /// Track document versions for LSP (URI -> version)
    document_versions: Mutex<HashMap<Uri, i32>>,
    /// Optional LSP client for code intelligence
    lsp_client: Mutex<Option<LspClient>>,
    tools: T,
}

#[tool_handler(router = self.tool_router)]
impl<T: CodingTools + 'static> ServerHandler for CodingMcp<T> {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "coding-mcp".to_string(),
                version: "0.1.0".to_string(),
                title: None,
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "A coding MCP server with grep-powered search, file operations (read/write), and bash command execution capabilities".into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

impl CodingMcp<DefaultCodingTools> {
    /// Create a new CodingMcp with default (local filesystem) tools
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            background_processes: Mutex::new(HashMap::new()),
            todos: Mutex::new(Vec::new()),
            files_read: Mutex::new(HashSet::new()),
            document_versions: Mutex::new(HashMap::new()),
            lsp_client: Mutex::new(None),
            tools: DefaultCodingTools,
        }
    }
}

#[tool_router]
impl<T: CodingTools + 'static> CodingMcp<T> {
    /// Create a CodingMcp with custom tool implementation
    pub fn with_tools(tools: T) -> Self {
        Self {
            tool_router: Self::tool_router(),
            background_processes: Mutex::new(HashMap::new()),
            todos: Mutex::new(Vec::new()),
            files_read: Mutex::new(HashSet::new()),
            document_versions: Mutex::new(HashMap::new()),
            lsp_client: Mutex::new(None),
            tools,
        }
    }

    /// Set the LSP client for code intelligence features
    ///
    /// This enables diagnostics querying and keeps the LSP in sync with file operations.
    /// The client should already be initialized before calling this method.
    pub fn set_lsp_client(&self, client: LspClient) {
        *self.lsp_client.lock().unwrap() = Some(client);
    }

    /// Remove the LSP client
    pub fn clear_lsp_client(&self) -> Option<LspClient> {
        self.lsp_client.lock().unwrap().take()
    }

    /// Notify the LSP client that a file was opened
    fn notify_lsp_did_open(&self, file_path: &str) {
        let client_guard = self.lsp_client.lock().unwrap();
        if let Some(client) = &*client_guard {
            // Read file content for LSP
            if let Ok(content) = std::fs::read_to_string(file_path) {
                if let Ok(uri) = path_to_uri(std::path::Path::new(file_path)) {
                    let language_id = detect_language_id(file_path);
                    // Ignore errors - LSP notifications are fire-and-forget
                    let _ = client.did_open(uri, language_id, content);
                }
            }
        }
    }

    /// Notify the LSP client that a file was changed
    fn notify_lsp_did_change(&self, file_path: &str, content: &str) {
        let client_guard = self.lsp_client.lock().unwrap();
        if let Some(client) = &*client_guard {
            if let Ok(uri) = path_to_uri(std::path::Path::new(file_path)) {
                // Get and increment version for this document
                let version = {
                    let mut versions = self.document_versions.lock().unwrap();
                    let version = versions.entry(uri.clone()).or_insert(0);
                    *version += 1;
                    *version
                };
                // Ignore errors - LSP notifications are fire-and-forget
                let _ = client.did_change(uri, version, content.to_string());
            }
        }
    }

    #[tool(
        description = "A powerful search tool built on ripgrep for finding patterns in file contents.

Usage:
- ALWAYS use this tool for search tasks. NEVER invoke `grep` or `rg` as bash commands.
- Supports full regex syntax (e.g., \"log.*Error\", \"function\\s+\\w+\")
- Filter files with glob parameter (e.g., \"*.js\", \"**/*.tsx\") or type parameter (e.g., \"js\", \"py\", \"rust\")
- Output modes: \"content\" shows matching lines, \"files_with_matches\" shows only file paths (default), \"count\" shows match counts
- Use Task tool for open-ended searches requiring multiple rounds
- Pattern syntax: Uses ripgrep (not grep) - literal braces need escaping (use `interface\\{\\}` to find `interface{}` in Go code)
- Multiline matching: By default patterns match within single lines only. For cross-line patterns like `struct \\{[\\s\\S]*?field`, use `multiline: true`
- You can call multiple tools in a single response. It is always better to speculatively perform multiple searches in parallel if they are potentially useful."
    )]
    pub async fn grep(&self, request: Parameters<GrepInput>) -> Result<Json<GrepOutput>, String> {
        let Parameters(args) = request;
        match perform_grep(args).await {
            Ok(result) => Ok(Json(result)),
            Err(e) => Err(format!("Grep error: {e}")),
        }
    }

    #[tool(
        description = "Fast file pattern matching tool that works with any codebase size.

Usage:
- Supports glob patterns like \"**/*.js\" or \"src/**/*.ts\"
- Returns matching file paths sorted alphabetically
- Use this tool when you need to find files by name patterns
- When doing an open-ended search that may require multiple rounds of globbing and grepping, use the Task tool instead
- You can call multiple tools in a single response. It is always better to speculatively perform multiple searches in parallel if they are potentially useful."
    )]
    pub async fn find(&self, request: Parameters<FindInput>) -> Result<Json<FindOutput>, String> {
        let Parameters(args) = request;
        match find_files_by_name(args).await {
            Ok(result) => Ok(Json(result)),
            Err(e) => Err(format!("Find error: {e}")),
        }
    }

    #[tool(
        description = "Reads a file from the local filesystem with line numbers. You can access any file directly by using this tool.

Usage:
- The file_path parameter must be an absolute path, not a relative path
- By default, reads up to 2000 lines starting from the beginning of the file
- You can optionally specify a line offset (1-indexed) and limit (especially handy for long files), but it's recommended to read the whole file by not providing these parameters
- Any lines longer than 2000 characters will be truncated
- Results are returned using line numbers starting at 1, formatted as '    1\\tline content'
- This tool can only read files, not directories. To read a directory, use the list_files tool
- You can call multiple tools in a single response. It is always better to speculatively read multiple potentially useful files in parallel
- Assume this tool is able to read all files. If the user provides a path to a file, assume that path is valid. It is okay to read a file that does not exist; an error will be returned

IMPORTANT - Safety Tracking:
- Reading a file successfully tracks it in the session
- You MUST read a file before you can edit it with edit_file or overwrite it with write_file
- This safety mechanism prevents accidental data loss and ensures you understand file contents before making changes"
    )]
    pub async fn read_file(
        &self,
        request: Parameters<ReadFileArgs>,
    ) -> Result<Json<ReadFileResult>, String> {
        let Parameters(args) = request;
        let file_path = args.file_path.clone();

        // Delegate to the tools implementation
        let result = self.tools.read_file(args).await?;

        // Track that this file has been read (safety check)
        self.files_read.lock().unwrap().insert(file_path.clone());

        // Notify LSP client if available (did_open)
        self.notify_lsp_did_open(&file_path);

        Ok(Json(result))
    }

    #[tool(
        description = "Writes a file to the local filesystem, replacing the entire file contents.

Usage:
- This tool will overwrite the existing file if there is one at the provided path
- ALWAYS prefer editing existing files in the codebase using edit_file. NEVER write new files unless explicitly required
- NEVER proactively create documentation files (*.md) or README files. Only create documentation files if explicitly requested by the user
- Only use emojis if the user explicitly requests it. Avoid writing emojis to files unless asked
- The file_path parameter must be an absolute path, not a relative path
- Creates parent directories automatically if they don't exist

IMPORTANT - Safety Requirements:
- If the file already exists, you MUST use read_file on it first before calling write_file
- This tool will return an error if you attempt to overwrite an existing file without reading it first
- New files (that don't exist yet) can be created without reading
- This safety check prevents accidental data loss by ensuring you see the current file contents before overwriting them"
    )]
    pub async fn write_file(
        &self,
        request: Parameters<WriteFileArgs>,
    ) -> Result<Json<WriteFileResponse>, String> {
        let Parameters(args) = request;

        // Safety check: if file exists, ensure it has been read first
        if std::path::Path::new(&args.file_path).exists() {
            let files_read = self.files_read.lock().unwrap();
            if !files_read.contains(&args.file_path) {
                return Err(format!(
                    "Safety check failed: File '{}' already exists. You must use read_file on it before overwriting. This prevents accidental data loss.",
                    args.file_path
                ));
            }
        }

        // Delegate to the tools implementation
        let response = self.tools.write_file(args.clone()).await?;

        // Notify LSP client if available (did_change)
        self.notify_lsp_did_change(&args.file_path, &args.content);

        Ok(Json(response))
    }

    #[tool(description = "Performs exact string replacements in files.

Usage:
- You must use read_file on this file at least once in the session before editing. This tool will error if you attempt an edit without reading the file
- When editing text from read_file output, ensure you preserve the exact indentation (tabs/spaces) as it appears AFTER the line number prefix
- The line number prefix format is: spaces + line number + tab character ('\\t'). Everything after that tab is the actual file content to match
- Never include any part of the line number prefix in the old_string or new_string
- ALWAYS prefer editing existing files in the codebase. NEVER write new files unless explicitly required
- Only use emojis if the user explicitly requests it. Avoid adding emojis to files unless asked
- The edit will FAIL if `old_string` is not unique in the file. Either provide a larger string with more surrounding context to make it unique or use `replace_all` to change every instance of `old_string`
- Use `replace_all` for replacing and renaming strings across the file. This parameter is useful if you want to rename a variable for instance")]
    pub async fn edit_file(
        &self,
        request: Parameters<EditFileArgs>,
    ) -> Result<Json<EditFileResponse>, String> {
        let Parameters(args) = request;

        // Safety check: ensure file has been read first
        {
            let files_read = self.files_read.lock().unwrap();
            if !files_read.contains(&args.file_path) {
                return Err(format!(
                    "Safety check failed: You must use read_file on '{}' before editing it. This ensures you understand the current file contents before making changes.",
                    args.file_path
                ));
            }
        }

        // Delegate to the tools implementation
        let file_path = args.file_path.clone();
        let response = self.tools.edit_file(args).await?;

        // Notify LSP client if available (did_change) - read new content
        if let Ok(content) = std::fs::read_to_string(&file_path) {
            self.notify_lsp_did_change(&file_path, &content);
        }

        Ok(Json(response))
    }

    #[tool(
        description = "List files and directories in a specified path with detailed metadata.

Usage:
- Returns file information including name, path, type (file/directory/symlink), size, permissions, and modification time
- By default, hidden files (starting with '.') are excluded unless include_hidden is set to true
- Results are sorted alphabetically by name
- Use this tool to explore directory contents before performing other operations
- File paths returned are absolute paths that can be used directly with other tools"
    )]
    pub async fn list_files(
        &self,
        request: Parameters<ListFilesArgs>,
    ) -> Result<Json<ListFilesResult>, String> {
        let Parameters(args) = request;
        self.tools.list_files(args).await.map(Json)
    }

    #[tool(
        description = "Executes a given bash command in a persistent shell session with optional timeout, ensuring proper handling and security measures.

IMPORTANT: This tool is for terminal operations like git, npm, docker, cargo, etc. DO NOT use it for file operations (reading, writing, editing, searching, finding files) - use the specialized tools for this instead.

Usage:
- The command argument is required
- You can specify an optional timeout in milliseconds (up to 600000ms / 10 minutes). If not specified, commands will timeout after 120000ms (2 minutes)
- It is very helpful if you write a clear, concise description of what this command does in 5-10 words in the description parameter
- If the output exceeds 30000 characters, output will be truncated before being returned to you
- You can use the `run_in_background` parameter to run the command in the background, which allows you to continue working while the command runs. You can monitor the output using the read_background_bash tool as it becomes available. Never use `run_in_background` to run 'sleep' as it will return immediately. You do not need to use '&' at the end of the command when using this parameter
- Avoid using bash with the `find`, `grep`, `cat`, `head`, `tail`, `sed`, `awk`, or `echo` commands, unless explicitly instructed or when these commands are truly necessary for the task. Instead, always prefer using the dedicated tools for these commands:
  - File search: Use find tool (NOT find or ls bash commands)
  - Content search: Use grep tool (NOT grep or rg bash commands)
  - Read files: Use read_file tool (NOT cat/head/tail)
  - Edit files: Use edit_file tool (NOT sed/awk)
  - Write files: Use write_file tool (NOT echo >/cat <<EOF)
- When issuing multiple commands:
  - If the commands are independent and can run in parallel, make multiple bash tool calls in a single message. For example, if you need to run \"git status\" and \"git diff\", send a single message with two bash tool calls in parallel
  - If the commands depend on each other and must run sequentially, use a single bash call with '&&' to chain them together (e.g., `git add . && git commit -m \"message\" && git push`). For instance, if one operation must complete before another starts (like mkdir before cp, write_file before bash for git operations, or git add before git commit), run these operations sequentially instead
  - Use ';' only when you need to run commands sequentially but don't care if earlier commands fail
  - DO NOT use newlines to separate commands (newlines are ok in quoted strings)"
    )]
    pub async fn bash(&self, request: Parameters<BashInput>) -> Result<Json<BashOutput>, String> {
        let Parameters(args) = request;
        match self.tools.bash(args).await? {
            BashResult::Completed(output) => Ok(Json(output)),
            BashResult::Background(handle) => {
                let shell_id = handle.shell_id.clone();

                // Store the background process
                self.background_processes
                    .lock()
                    .unwrap()
                    .insert(shell_id.clone(), handle);

                // Return immediate response with shell_id
                Ok(Json(BashOutput {
                    output: String::new(),
                    exit_code: 0,
                    killed: None,
                    shell_id: Some(shell_id),
                }))
            }
        }
    }

    #[tool(
        description = "Retrieves output from a running or completed background bash shell.

Usage:
- Takes a bash_id parameter identifying the shell (returned from bash tool when run_in_background is true)
- Always returns only new output since the last check
- Returns stdout and stderr output along with shell status (running/completed/failed)
- Supports optional regex filtering to show only lines matching a pattern
- Use this tool when you need to monitor or check the output of a long-running shell
- When a shell is completed, the output is final and the shell ID becomes invalid"
    )]
    pub async fn read_background_bash(
        &self,
        request: Parameters<ReadBackgroundBashInput>,
    ) -> Result<Json<ReadBackgroundBashOutput>, String> {
        let Parameters(args) = request;

        let handle = self
            .background_processes
            .lock()
            .unwrap()
            .remove(&args.bash_id)
            .ok_or_else(|| format!("Shell ID not found: {}", args.bash_id))?;

        let (result, handle_opt) = self.tools.read_background_bash(handle, args.filter).await?;

        // Put handle back if still running
        if let Some(handle) = handle_opt {
            self.background_processes
                .lock()
                .unwrap()
                .insert(args.bash_id, handle);
        }

        Ok(Json(result))
    }

    #[tool(
        description = "Use this tool to create and manage a structured task list for your current coding session. This helps you track progress, organize complex tasks, and demonstrate thoroughness to the user.

## When to Use This Tool
Use this tool proactively in these scenarios:
1. Complex multi-step tasks - When a task requires 3 or more distinct steps or actions
2. Non-trivial and complex tasks - Tasks that require careful planning or multiple operations
3. User explicitly requests todo list - When the user directly asks you to use the todo list
4. User provides multiple tasks - When users provide a list of things to be done (numbered or comma-separated)
5. After receiving new instructions - Immediately capture user requirements as todos
6. When you start working on a task - Mark it as in_progress BEFORE beginning work. Ideally you should only have one todo as in_progress at a time
7. After completing a task - Mark it as completed and add any new follow-up tasks discovered during implementation

## When NOT to Use This Tool
Skip using this tool when:
1. There is only a single, straightforward task
2. The task is trivial and tracking it provides no organizational benefit
3. The task can be completed in less than 3 trivial steps
4. The task is purely conversational or informational

## Task States and Management
1. Task States: Use these states to track progress:
   - pending: Task not yet started
   - in_progress: Currently working on (limit to ONE task at a time)
   - completed: Task finished successfully

   IMPORTANT: Task descriptions must have two forms:
   - content: The imperative form describing what needs to be done (e.g., 'Run tests', 'Build the project')
   - active_form: The present continuous form shown during execution (e.g., 'Running tests', 'Building the project')

2. Task Management:
   - Update task status in real-time as you work
   - Mark tasks complete IMMEDIATELY after finishing (don't batch completions)
   - Exactly ONE task must be in_progress at any time (not less, not more)
   - Complete current tasks before starting new ones
   - Remove tasks that are no longer relevant from the list entirely

3. Task Completion Requirements:
   - ONLY mark a task as completed when you have FULLY accomplished it
   - If you encounter errors, blockers, or cannot finish, keep the task as in_progress
   - When blocked, create a new task describing what needs to be resolved
   - Never mark a task as completed if:
     - Tests are failing
     - Implementation is partial
     - You encountered unresolved errors
     - You couldn't find necessary files or dependencies

4. Task Breakdown:
   - Create specific, actionable items
   - Break complex tasks into smaller, manageable steps
   - Use clear, descriptive task names
   - Always provide both forms:
     - content: 'Fix authentication bug'
     - active_form: 'Fixing authentication bug'

When in doubt, use this tool. Being proactive with task management demonstrates attentiveness and ensures you complete all requirements successfully."
    )]
    pub async fn todo_write(
        &self,
        request: Parameters<TodoWriteInput>,
    ) -> Result<Json<TodoWriteOutput>, String> {
        let Parameters(input) = request;

        {
            let mut todos = self.todos.lock().unwrap();
            *todos = input.todos.clone();
        };

        let output = process_todo_write(input);
        Ok(Json(output))
    }

    #[tool(
        description = "Query language server information for code intelligence.

This tool provides access to LSP (Language Server Protocol) features like diagnostics (errors, warnings).
The language server must be initialized separately before using this tool.

Operations:
- get_diagnostics: Get compiler errors and warnings. Optionally filter by file_path.

Example usage:
- Get all diagnostics: {\"operation\": \"get_diagnostics\"}
- Get diagnostics for a file: {\"operation\": \"get_diagnostics\", \"file_path\": \"src/main.rs\"}"
    )]
    pub async fn lsp(&self, request: Parameters<LspInput>) -> Result<Json<LspOutput>, String> {
        let Parameters(input) = request;

        // Get diagnostics from the LSP client if available
        let diagnostics_cache = {
            let client_guard = self.lsp_client.lock().unwrap();
            match &*client_guard {
                Some(client) => client.get_all_diagnostics(),
                None => HashMap::new(),
            }
        };

        execute_lsp_operation(input.operation, &diagnostics_cache).map(Json)
    }
}

impl Default for CodingMcp<DefaultCodingTools> {
    fn default() -> Self {
        Self::new()
    }
}

/// Detect language ID from file extension for LSP
fn detect_language_id(file_path: &str) -> &'static str {
    let path = std::path::Path::new(file_path);
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("py" | "pyi" | "pyw") => "python",
        Some("js" | "mjs") => "javascript",
        Some("jsx") => "javascriptreact",
        Some("ts") => "typescript",
        Some("tsx") => "typescriptreact",
        Some("go") => "go",
        Some("java") => "java",
        Some("c" | "h") => "c",
        Some("cpp" | "cxx" | "cc" | "hpp" | "hxx" | "hh") => "cpp",
        Some("cs") => "csharp",
        Some("rb") => "ruby",
        Some("php") => "php",
        Some("swift") => "swift",
        Some("kt" | "kts") => "kotlin",
        Some("scala") => "scala",
        Some("html" | "htm") => "html",
        Some("css") => "css",
        Some("json") => "json",
        Some("yaml" | "yml") => "yaml",
        Some("toml") => "toml",
        Some("md" | "markdown") => "markdown",
        Some("xml") => "xml",
        Some("sql") => "sql",
        Some("sh" | "bash" | "zsh") => "shellscript",
        _ => "plaintext",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language_id() {
        assert_eq!(detect_language_id("/path/to/file.rs"), "rust");
        assert_eq!(detect_language_id("main.py"), "python");
        assert_eq!(detect_language_id("script.js"), "javascript");
        assert_eq!(detect_language_id("component.tsx"), "typescriptreact");
        assert_eq!(detect_language_id("main.go"), "go");
        assert_eq!(detect_language_id("config.yaml"), "yaml");
        assert_eq!(detect_language_id("config.yml"), "yaml");
        assert_eq!(detect_language_id("unknown.xyz"), "plaintext");
        assert_eq!(detect_language_id("noextension"), "plaintext");
    }

    #[tokio::test]
    async fn test_lsp_tool_without_client_returns_empty() {
        use lsp_tool::LspOperation;

        let mcp = CodingMcp::new();

        // Without an LSP client, get_diagnostics should return empty
        let diagnostics_cache = {
            let client_guard = mcp.lsp_client.lock().unwrap();
            match &*client_guard {
                Some(client) => client.get_all_diagnostics(),
                None => HashMap::new(),
            }
        };

        let result = execute_lsp_operation(
            LspOperation::GetDiagnostics { file_path: None },
            &diagnostics_cache,
        ).unwrap();

        match result {
            LspOutput::Diagnostics(output) => {
                assert_eq!(output.diagnostics.len(), 0);
                assert_eq!(output.summary.total, 0);
            }
        }
    }
}
