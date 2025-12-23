use clap::Parser;
use rmcp::{
    ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use std::path::PathBuf;
use std::sync::Mutex;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

pub mod default_tools;
pub mod lsp;
pub mod tools;
pub mod tools_trait;

// Re-export from tools module for backwards compatibility
pub use tools::{
    BackgroundProcessHandle, BashInput, BashOutput, BashResult, ReadBackgroundBashInput,
    ReadBackgroundBashOutput, execute_command, read_background_bash,
};
pub use default_tools::DefaultCodingTools;
pub use tools::{EditFileArgs, EditFileResponse, edit_file_contents};
pub use tools::{FindInput, FindOutput, find_files_by_name};
pub use tools::{GrepInput, GrepOutput, perform_grep};
pub use tools::{ListFilesArgs, ListFilesResult, list_files};
pub use tools::LspCodingTools;
pub use tools::{
    DiagnosticsSummary, LocationResult, LspDiagnostic, LspDiagnosticsInput, LspDiagnosticsOutput,
    LspFindReferencesInput, LspFindReferencesOutput, LspGotoDefinitionInput,
    LspGotoDefinitionOutput, LspHoverInput, LspHoverOutput, LspWorkspaceSymbolInput,
    LspWorkspaceSymbolOutput, SymbolResult, execute_lsp_diagnostics, execute_lsp_find_references,
    execute_lsp_goto_definition, execute_lsp_hover, execute_lsp_workspace_symbol,
};
pub use tools::{ReadFileArgs, ReadFileResult, read_file_contents};
pub use tools::{TodoItem, TodoStatus, TodoWriteInput, TodoWriteOutput, process_todo_write};
pub use tools_trait::CodingTools;
pub use tools::{WriteFileArgs, WriteFileResponse, write_file_contents};

/// CLI arguments for CodingMcp server
#[derive(Debug, Clone, Parser)]
pub struct CodingMcpArgs {
    /// Root directory for the workspace (used for LSP initialization)
    #[arg(long = "root-dir")]
    pub root_dir: Option<PathBuf>,
}

impl CodingMcpArgs {
    pub fn from_args(args: Vec<String>) -> Result<Self, String> {
        // Prepend a dummy program name since clap expects it
        let mut full_args = vec!["coding-mcp".to_string()];
        full_args.extend(args);

        Self::try_parse_from(full_args)
            .map_err(|e| format!("Failed to parse CodingMcp arguments: {e}"))
    }
}

#[derive(Debug)]
pub struct CodingMcp<T: CodingTools = DefaultCodingTools> {
    tool_router: ToolRouter<Self>,
    background_processes: Mutex<HashMap<String, BackgroundProcessHandle>>,
    todos: Mutex<Vec<TodoItem>>,
    /// Track files that have been read to enforce read-before-edit safety
    files_read: Mutex<HashSet<String>>,
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
            tools: DefaultCodingTools::new(),
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
            tools,
        }
    }

    #[doc = include_str!("tools/grep/description.md")]
    #[tool]
    pub async fn grep(&self, request: Parameters<GrepInput>) -> Result<Json<GrepOutput>, String> {
        let Parameters(args) = request;
        match perform_grep(args).await {
            Ok(result) => Ok(Json(result)),
            Err(e) => Err(format!("Grep error: {e}")),
        }
    }

    #[doc = include_str!("tools/find/description.md")]
    #[tool]
    pub async fn find(&self, request: Parameters<FindInput>) -> Result<Json<FindOutput>, String> {
        let Parameters(args) = request;
        match find_files_by_name(args).await {
            Ok(result) => Ok(Json(result)),
            Err(e) => Err(format!("Find error: {e}")),
        }
    }

    #[doc = include_str!("tools/read_file/description.md")]
    #[tool]
    pub async fn read_file(
        &self,
        request: Parameters<ReadFileArgs>,
    ) -> Result<Json<ReadFileResult>, String> {
        let Parameters(args) = request;
        let file_path = args.file_path.clone();
        let result = self.tools.read_file(args).await?;
        self.files_read.lock().unwrap().insert(file_path);

        Ok(Json(result))
    }

    #[doc = include_str!("tools/write_file/description.md")]
    #[tool]
    pub async fn write_file(
        &self,
        request: Parameters<WriteFileArgs>,
    ) -> Result<Json<WriteFileResponse>, String> {
        let Parameters(args) = request;

        // Safety check: if file exists, ensure it has been read first
        if Path::new(&args.file_path).exists() {
            let files_read = self.files_read.lock().unwrap();
            if !files_read.contains(&args.file_path) {
                return Err(format!(
                    "Safety check failed: File '{}' already exists. You must use read_file on it before overwriting. This prevents accidental data loss.",
                    args.file_path
                ));
            }
        }

        let response = self.tools.write_file(args).await?;

        Ok(Json(response))
    }

    #[doc = include_str!("tools/edit_file/description.md")]
    #[tool]
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

        let response = self.tools.edit_file(args).await?;

        Ok(Json(response))
    }

    #[doc = include_str!("tools/list_files/description.md")]
    #[tool]
    pub async fn list_files(
        &self,
        request: Parameters<ListFilesArgs>,
    ) -> Result<Json<ListFilesResult>, String> {
        let Parameters(args) = request;
        self.tools.list_files(args).await.map(Json)
    }

    #[doc = include_str!("tools/bash/description.md")]
    #[tool]
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

    #[doc = include_str!("tools/bash/read_background_description.md")]
    #[tool]
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

    #[doc = include_str!("tools/todo_write/description.md")]
    #[tool]
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

    #[doc = include_str!("tools/lsp/diagnostics/description.md")]
    #[tool]
    pub async fn lsp_diagnostics(
        &self,
        request: Parameters<LspDiagnosticsInput>,
    ) -> Result<Json<LspDiagnosticsOutput>, String> {
        let Parameters(input) = request;
        execute_lsp_diagnostics(input, &self.tools).await.map(Json)
    }

    #[doc = include_str!("tools/lsp/goto_definition/description.md")]
    #[tool]
    pub async fn lsp_goto_definition(
        &self,
        request: Parameters<LspGotoDefinitionInput>,
    ) -> Result<Json<LspGotoDefinitionOutput>, String> {
        let Parameters(input) = request;
        execute_lsp_goto_definition(input, &self.tools)
            .await
            .map(Json)
    }

    #[doc = include_str!("tools/lsp/find_references/description.md")]
    #[tool]
    pub async fn lsp_find_references(
        &self,
        request: Parameters<LspFindReferencesInput>,
    ) -> Result<Json<LspFindReferencesOutput>, String> {
        let Parameters(input) = request;
        execute_lsp_find_references(input, &self.tools)
            .await
            .map(Json)
    }

    #[doc = include_str!("tools/lsp/hover/description.md")]
    #[tool]
    pub async fn lsp_hover(
        &self,
        request: Parameters<LspHoverInput>,
    ) -> Result<Json<LspHoverOutput>, String> {
        let Parameters(input) = request;
        execute_lsp_hover(input, &self.tools).await.map(Json)
    }

    #[doc = include_str!("tools/lsp/workspace_symbol/description.md")]
    #[tool]
    pub async fn lsp_workspace_symbol(
        &self,
        request: Parameters<LspWorkspaceSymbolInput>,
    ) -> Result<Json<LspWorkspaceSymbolOutput>, String> {
        let Parameters(input) = request;
        execute_lsp_workspace_symbol(input, &self.tools)
            .await
            .map(Json)
    }
}

impl Default for CodingMcp<DefaultCodingTools> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lsp_tool_without_wrapper_returns_error() {
        let mcp = CodingMcp::new();
        let result = mcp.tools.get_lsp_diagnostics().await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("LSP not configured"));
    }
}
