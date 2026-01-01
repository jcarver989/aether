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
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};
use tokio::sync::{Mutex, RwLock};

// Submodules - import types from their source modules directly
pub mod default_tools;
pub mod error;
pub mod file_types;
pub mod lsp;
pub mod tools;
pub mod tools_trait;

// =============================================================================
// Public API exports
// =============================================================================
// These are the stable public types for external consumers.
// Import other types directly from their submodules (e.g., tools::bash::BashInput).

pub use default_tools::DefaultCodingTools;
pub use tools::lsp::LspCodingTools;
pub use tools_trait::CodingTools;

// =============================================================================
// Internal imports (used by CodingMcp implementation below)
// =============================================================================

use tools::bash::{
    BackgroundProcessHandle, BashInput, BashOutput, BashResult, ReadBackgroundBashInput,
    ReadBackgroundBashOutput, execute_command, read_background_bash,
};
use tools::edit_file::{EditFileArgs, EditFileResponse, edit_file_contents};
use tools::find::{FindInput, FindOutput, find_files_by_name};
use tools::grep::{GrepInput, GrepOutput, perform_grep};
use tools::list_files::{ListFilesArgs, ListFilesResult, list_files};
use tools::lsp::check_errors::{
    LspDiagnosticsInput, LspDiagnosticsOutput, execute_lsp_diagnostics,
};
use tools::lsp::find_definition::{
    LspGotoDefinitionInput, LspGotoDefinitionOutput, execute_lsp_goto_definition,
};
use tools::lsp::find_usages::{
    LspFindReferencesInput, LspFindReferencesOutput, execute_lsp_find_references,
};
use tools::lsp::get_type_info::{LspHoverInput, LspHoverOutput, execute_lsp_hover};
use tools::lsp::search_symbols::{
    LspWorkspaceSymbolInput, LspWorkspaceSymbolOutput, execute_lsp_workspace_symbol,
};
use tools::read_file::{ReadFileArgs, ReadFileResult, read_file_contents};
use tools::todo_write::{TodoItem, TodoWriteInput, TodoWriteOutput, process_todo_write};
use tools::web_fetch::{WebFetchInput, WebFetchOutput, WebFetcher};
use tools::write_file::{WriteFileArgs, WriteFileResponse, write_file_contents};

/// Extension trait for converting tool results to MCP format
trait IntoMcpResult<T> {
    fn into_mcp(self) -> Result<Json<T>, String>;
}

impl<T, E: std::fmt::Display> IntoMcpResult<T> for Result<T, E> {
    fn into_mcp(self) -> Result<Json<T>, String> {
        self.map(Json).map_err(|e| e.to_string())
    }
}

/// CLI arguments for CodingMcp server
#[derive(Debug, Clone, Parser)]
pub struct CodingMcpArgs {
    /// Root directory for workspace (used for LSP initialization)
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

    /// Parse the root directory from an mcp.json config file.
    ///
    /// Looks for the "coding" server entry and parses its args for `--root-dir`.
    /// Relative paths (like ".") are resolved against the mcp.json's directory.
    pub fn parse_root_dir_from_config(mcp_config_path: &Path) -> Option<PathBuf> {
        let raw_config = RawMcpConfig::from_json_file(mcp_config_path).ok()?;
        let coding_config = raw_config.servers.get("coding")?;

        if let RawMcpServerConfig::InMemory { args } = coding_config {
            let parsed_args = Self::from_args(args.clone()).ok()?;
            let root_dir = parsed_args.root_dir?;

            if root_dir.is_relative() {
                let config_dir = mcp_config_path.parent()?;
                Some(config_dir.join(&root_dir).canonicalize().ok()?)
            } else {
                Some(root_dir)
            }
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct CodingMcp<T: CodingTools = DefaultCodingTools> {
    tool_router: ToolRouter<Self>,
    background_processes: Mutex<HashMap<String, BackgroundProcessHandle>>,
    todos: Mutex<Vec<TodoItem>>,
    /// Track files that have been read to enforce read-before-edit safety
    files_read: RwLock<HashSet<String>>,
    tools: T,
    web_fetcher: WebFetcher,
    /// Workspace root directory - communicated to LLMs via server instructions
    root_dir: Option<PathBuf>,
}

#[tool_handler(router = self.tool_router)]
impl<T: CodingTools + 'static> ServerHandler for CodingMcp<T> {
    fn get_info(&self) -> ServerInfo {
        let instructions = self.build_instructions();
        ServerInfo {
            server_info: Implementation {
                name: "coding-mcp".to_string(),
                version: "0.1.0".to_string(),
                title: None,
                icons: None,
                website_url: None,
            },
            instructions: Some(instructions),
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
            files_read: RwLock::new(HashSet::new()),
            tools: DefaultCodingTools::new(),
            web_fetcher: WebFetcher::new(),
            root_dir: None,
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
            files_read: RwLock::new(HashSet::new()),
            tools,
            web_fetcher: WebFetcher::new(),
            root_dir: None,
        }
    }

    /// Set the workspace root directory.
    ///
    /// This path is communicated to LLMs via the server instructions,
    /// helping them use correct absolute paths when calling file tools.
    pub fn with_root_dir(mut self, root_dir: PathBuf) -> Self {
        self.root_dir = Some(root_dir);
        self
    }

    fn build_instructions(&self) -> String {
        match &self.root_dir {
            Some(root) => format!(
                r#" # Coding MCP server
This MCP server is equipped with grep-powered search, file operations (read/write), and bash command execution capabilities.

When using tools from this server that take file path(s) as input, always use absolute paths starting from the workspace root:.
<workspace-root>{}</workspace-root>
"#,
                root.display()
            ),
            None => "A coding MCP server with grep-powered search, file operations (read/write), and bash command execution capabilities.".to_string(),
        }
    }

    #[doc = include_str!("tools/grep/description.md")]
    #[tool]
    pub async fn grep(&self, request: Parameters<GrepInput>) -> Result<Json<GrepOutput>, String> {
        let Parameters(args) = request;
        self.tools.grep(args).await.into_mcp()
    }

    #[doc = include_str!("tools/find/description.md")]
    #[tool]
    pub async fn find(&self, request: Parameters<FindInput>) -> Result<Json<FindOutput>, String> {
        let Parameters(args) = request;
        self.tools.find(args).await.into_mcp()
    }

    #[doc = include_str!("tools/read_file/description.md")]
    #[tool]
    pub async fn read_file(
        &self,
        request: Parameters<ReadFileArgs>,
    ) -> Result<Json<ReadFileResult>, String> {
        let Parameters(args) = request;
        let file_path = args.file_path.clone();
        let result = self
            .tools
            .read_file(args)
            .await
            .map_err(|e| e.to_string())?;
        self.files_read.write().await.insert(file_path);

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
            let files_read = self.files_read.read().await;
            if !files_read.contains(&args.file_path) {
                return Err(format!(
                    "Safety check failed: File '{}' already exists. You must use read_file on it before overwriting. This prevents accidental data loss.",
                    args.file_path
                ));
            }
        }

        let response = self
            .tools
            .write_file(args)
            .await
            .map_err(|e| e.to_string())?;

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
            let files_read = self.files_read.read().await;
            if !files_read.contains(&args.file_path) {
                return Err(format!(
                    "Safety check failed: You must use read_file on '{}' before editing it. This ensures you understand the current file contents before making changes.",
                    args.file_path
                ));
            }
        }

        let response = self
            .tools
            .edit_file(args)
            .await
            .map_err(|e| e.to_string())?;

        Ok(Json(response))
    }

    #[doc = include_str!("tools/list_files/description.md")]
    #[tool]
    pub async fn list_files(
        &self,
        request: Parameters<ListFilesArgs>,
    ) -> Result<Json<ListFilesResult>, String> {
        let Parameters(args) = request;
        self.tools.list_files(args).await.into_mcp()
    }

    #[doc = include_str!("tools/bash/description.md")]
    #[tool]
    pub async fn bash(&self, request: Parameters<BashInput>) -> Result<Json<BashOutput>, String> {
        let Parameters(args) = request;
        let result = self.tools.bash(args).await.map_err(|e| e.to_string())?;

        match result {
            BashResult::Completed(output) => Ok(Json(output)),
            BashResult::Background(handle) => {
                let shell_id = handle.shell_id.clone();

                // Store the background process
                self.background_processes
                    .lock()
                    .await
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
            .await
            .remove(&args.bash_id)
            .ok_or_else(|| format!("Shell ID not found: {}", args.bash_id))?;

        let (result, handle_opt) = self
            .tools
            .read_background_bash(handle, args.filter)
            .await
            .map_err(|e| e.to_string())?;

        // Put handle back if still running
        if let Some(handle) = handle_opt {
            self.background_processes
                .lock()
                .await
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
            let mut todos = self.todos.lock().await;
            *todos = input.todos.clone();
        };

        let output = process_todo_write(input);
        Ok(Json(output))
    }

    #[doc = include_str!("tools/lsp/check_errors/description.md")]
    #[tool]
    pub async fn check_errors(
        &self,
        request: Parameters<LspDiagnosticsInput>,
    ) -> Result<Json<LspDiagnosticsOutput>, String> {
        let Parameters(input) = request;
        execute_lsp_diagnostics(input, &self.tools).await.map(Json)
    }

    #[doc = include_str!("tools/lsp/find_definition/description.md")]
    #[tool]
    pub async fn find_definition(
        &self,
        request: Parameters<LspGotoDefinitionInput>,
    ) -> Result<Json<LspGotoDefinitionOutput>, String> {
        let Parameters(input) = request;
        execute_lsp_goto_definition(input, &self.tools)
            .await
            .map(Json)
    }

    #[doc = include_str!("tools/lsp/find_usages/description.md")]
    #[tool]
    pub async fn find_usages(
        &self,
        request: Parameters<LspFindReferencesInput>,
    ) -> Result<Json<LspFindReferencesOutput>, String> {
        let Parameters(input) = request;
        execute_lsp_find_references(input, &self.tools)
            .await
            .map(Json)
    }

    #[doc = include_str!("tools/lsp/get_type_info/description.md")]
    #[tool]
    pub async fn get_type_info(
        &self,
        request: Parameters<LspHoverInput>,
    ) -> Result<Json<LspHoverOutput>, String> {
        let Parameters(input) = request;
        execute_lsp_hover(input, &self.tools).await.map(Json)
    }

    #[doc = include_str!("tools/lsp/search_symbols/description.md")]
    #[tool]
    pub async fn search_symbols(
        &self,
        request: Parameters<LspWorkspaceSymbolInput>,
    ) -> Result<Json<LspWorkspaceSymbolOutput>, String> {
        let Parameters(input) = request;
        execute_lsp_workspace_symbol(input, &self.tools)
            .await
            .map(Json)
    }

    #[doc = include_str!("tools/web_fetch/description.md")]
    #[tool]
    pub async fn web_fetch(
        &self,
        request: Parameters<WebFetchInput>,
    ) -> Result<Json<WebFetchOutput>, String> {
        let Parameters(args) = request;
        self.web_fetcher.fetch(args).await.into_mcp()
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
        assert!(matches!(
            result.unwrap_err(),
            error::CodingError::NotConfigured(_)
        ));
    }
}
