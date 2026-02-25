use clap::Parser;
use mcp_utils::client::{RawMcpConfig, RawMcpServerConfig};
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
    sync::Arc,
};
use tokio::{
    fs::try_exists,
    sync::{Mutex, RwLock},
};

pub mod default_tools;
pub mod error;
pub mod tools;
pub mod tools_trait;

pub use default_tools::DefaultCodingTools;
pub use tools_trait::CodingTools;

use crate::lsp::registry::LspRegistry;
use crate::lsp::tools::check_errors::{
    LspDiagnosticsInput, LspDiagnosticsOutput, execute_lsp_diagnostics,
};
use crate::lsp::tools::document_info::{LspDocumentInput, LspDocumentOutput, execute_lsp_document};
use crate::lsp::tools::symbol_lookup::{LspSymbolInput, LspSymbolOutput, execute_lsp_symbol};

use mcp_utils::display_meta::{ToolDisplayMeta, truncate};
use tools::bash::{
    BackgroundProcessHandle, BashInput, BashOutput, BashResult, ReadBackgroundBashInput,
    ReadBackgroundBashOutput, execute_command, read_background_bash,
};
use tools::edit_file::{EditFileArgs, EditFileResponse, edit_file_contents};
use tools::find::{FindInput, FindOutput, find_files_by_name};
use tools::grep::{GrepInput, GrepOutput, perform_grep};
use tools::list_files::{ListFilesArgs, ListFilesResult, list_files};
use tools::read_file::{ReadFileArgs, ReadFileResult, read_file_contents};
use tools::web_fetch::{WebFetchInput, WebFetchOutput, WebFetcher};
use tools::web_search::search_client::BraveSearchClient;
use tools::web_search::{WebSearchInput, WebSearchOutput, WebSearcher};
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

/// CLI arguments for `CodingMcp` server
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
    /// Track files that have been read to enforce read-before-edit safety
    files_read: RwLock<HashSet<String>>,
    tools: T,
    /// Optional LSP operations (enabled with `.with_lsp()`)
    lsp: Option<Arc<LspRegistry>>,
    web_fetcher: WebFetcher,
    web_searcher: Option<WebSearcher<BraveSearchClient>>,
    /// Workspace roots (from MCP protocol or CLI args)
    roots: RwLock<Vec<PathBuf>>,
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
                description: None,
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
    /// Create a new `CodingMcp` with default (local filesystem) tools
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            background_processes: Mutex::new(HashMap::new()),
            files_read: RwLock::new(HashSet::new()),
            tools: DefaultCodingTools::new(),
            lsp: None,
            web_fetcher: WebFetcher::new(),
            web_searcher: WebSearcher::try_new().ok(),
            roots: RwLock::new(Vec::new()),
        }
    }
}

#[tool_router]
impl<T: CodingTools + 'static> CodingMcp<T> {
    /// Create a `CodingMcp` with custom tool implementation
    pub fn with_tools(tools: T) -> Self {
        Self {
            tool_router: Self::tool_router(),
            background_processes: Mutex::new(HashMap::new()),
            files_read: RwLock::new(HashSet::new()),
            tools,
            lsp: None,
            web_fetcher: WebFetcher::new(),
            web_searcher: WebSearcher::try_new().ok(),
            roots: RwLock::new(Vec::new()),
        }
    }

    /// Enable LSP code intelligence for the given project root.
    ///
    /// LSP servers for detected project languages are spawned immediately
    /// in the background, allowing indexing to start right away.
    pub fn with_lsp(mut self, root_path: PathBuf) -> Self {
        self.lsp = Some(LspRegistry::new_and_spawn(root_path));
        self
    }

    /// Set workspace roots.
    pub fn with_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.roots = RwLock::new(roots);
        self
    }

    /// Set the workspace root directory from a single path.
    pub fn with_root_dir(self, root_dir: PathBuf) -> Self {
        self.with_roots(vec![root_dir])
    }

    /// Get the current workspace root.
    fn get_workspace_root(&self) -> Option<PathBuf> {
        self.roots
            .try_read()
            .ok()
            .and_then(|roots| roots.first().cloned())
    }

    fn build_instructions(&self) -> String {
        let base = r"# Coding MCP Server

File I/O, search, shell, and LSP code intelligence tools for coding workflows.

## Quick Reference

- **Text patterns** (TODOs, logs, strings): `grep`
- **File names** (find *.test.ts): `find`
- **Read/write/edit** files: `read_file`, `write_file`, `edit_file`
- **Shell commands**: `bash`
- **Errors & warnings** (instant check without build): `lsp_check_errors`
- **Code symbols** (definitions, usages, types): `lsp_symbol`
- **File structure** (what's in this file?): `lsp_document`
";

        match self.get_workspace_root() {
            Some(root) => format!(
                r"{}

When using tools that take file paths, always use absolute paths from:
<workspace-root>{}</workspace-root>",
                base,
                root.display()
            ),
            None => base.to_string(),
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
        if try_exists(&args.file_path)
            .await
            .map_err(|e| format!("Failed to check existence of {}: {e}", args.file_path))?
        {
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
        let command = args.command.clone();
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

                let display_meta = ToolDisplayMeta::new(
                    "Run command",
                    format!("{} (background)", truncate(&command, 40)),
                );

                // Return immediate response with shell_id
                Ok(Json(BashOutput {
                    output: String::new(),
                    exit_code: 0,
                    killed: None,
                    shell_id: Some(shell_id),
                    _meta: Some(display_meta.into()),
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

    #[doc = include_str!("tools/web_fetch/description.md")]
    #[tool]
    pub async fn web_fetch(
        &self,
        request: Parameters<WebFetchInput>,
    ) -> Result<Json<WebFetchOutput>, String> {
        let Parameters(args) = request;
        self.web_fetcher.fetch(args).await.into_mcp()
    }

    #[doc = include_str!("tools/web_search/description.md")]
    #[tool]
    pub async fn web_search(
        &self,
        request: Parameters<WebSearchInput>,
    ) -> Result<Json<WebSearchOutput>, String> {
        let Parameters(args) = request;

        let searcher = self.web_searcher.as_ref().ok_or_else(|| {
            "Web search not available: BRAVE_SEARCH_API_KEY environment variable not set. \
                 Get a free API key from https://api.search.brave.com/app/keys"
                .to_string()
        })?;

        searcher
            .search(args)
            .await
            .map_err(|e| e.to_string())
            .map(Json)
    }

    #[doc = include_str!("../lsp/tools/symbol_lookup/description.md")]
    #[tool]
    pub async fn lsp_symbol(
        &self,
        request: Parameters<LspSymbolInput>,
    ) -> Result<Json<LspSymbolOutput>, String> {
        let Parameters(input) = request;
        let lsp = self.lsp.as_ref().ok_or("LSP not configured")?;
        execute_lsp_symbol(input, lsp.as_ref()).await.map(Json)
    }

    #[doc = include_str!("../lsp/tools/document_info/description.md")]
    #[tool]
    pub async fn lsp_document(
        &self,
        request: Parameters<LspDocumentInput>,
    ) -> Result<Json<LspDocumentOutput>, String> {
        let Parameters(input) = request;
        let lsp = self.lsp.as_ref().ok_or("LSP not configured")?;
        execute_lsp_document(input, lsp.as_ref()).await.map(Json)
    }

    #[doc = include_str!("../lsp/tools/check_errors/description.md")]
    #[tool]
    pub async fn lsp_check_errors(
        &self,
        request: Parameters<LspDiagnosticsInput>,
    ) -> Result<Json<LspDiagnosticsOutput>, String> {
        let Parameters(input) = request;
        let lsp = self.lsp.as_ref().ok_or("LSP not configured")?;
        execute_lsp_diagnostics(input, lsp.as_ref()).await.map(Json)
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
    async fn test_lsp_tool_without_lsp_returns_error() {
        let mcp = CodingMcp::new();
        let input = LspDiagnosticsInput { file_path: None };
        let result = mcp.lsp_check_errors(Parameters(input)).await;
        assert!(result.is_err());
    }
}
