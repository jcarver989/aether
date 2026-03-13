use clap::Parser;
use mcp_utils::client::{RawMcpConfig, RawMcpServerConfig};
use rmcp::{
    RoleServer, ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{Implementation, ProgressNotificationParam, ServerCapabilities, ServerInfo},
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use std::fmt::Write as _;
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
pub mod prompt_rule_matcher;
pub mod tools;
pub mod tools_trait;

pub use default_tools::DefaultCodingTools;
pub use tools_trait::CodingTools;

use crate::lsp::registry::LspRegistry;
use crate::lsp::tools::check_errors::{
    LspDiagnosticsInput, LspDiagnosticsOutput, LspDiagnosticsRequest, execute_lsp_diagnostics,
};
use crate::lsp::tools::document_info::{LspDocumentInput, LspDocumentOutput, execute_lsp_document};
use crate::lsp::tools::rename::{LspRenameInput, LspRenameOutput, execute_lsp_rename};
use crate::lsp::tools::symbol_lookup::{LspSymbolInput, LspSymbolOutput, execute_lsp_symbol};

use mcp_utils::display_meta::{ToolDisplayMeta, ToolResultMeta, basename, truncate};
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

        if let RawMcpServerConfig::InMemory { args, .. } = coding_config {
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
    /// Read rules discovered from skill files (activated on file reads)
    read_rule_state: prompt_rule_matcher::PromptRuleMatcher,
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
            read_rule_state: prompt_rule_matcher::PromptRuleMatcher::default(),
        }
    }
}

async fn notify_preview(context: &RequestContext<RoleServer>, meta: ToolDisplayMeta) {
    if let Some(token) = context.meta.get_progress_token() {
        let result_meta = ToolResultMeta::from(meta);
        let message = serde_json::to_string(&result_meta).unwrap_or_default();
        let _ = context
            .peer
            .notify_progress(ProgressNotificationParam {
                progress_token: token,
                progress: 0.0,
                total: None,
                message: Some(message),
            })
            .await;
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
            read_rule_state: prompt_rule_matcher::PromptRuleMatcher::default(),
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
        let catalog = match roots.first() {
            Some(root) => {
                let skills_dir = root.join(".aether").join("skills");
                aether_project::PromptCatalog::from_dir(&skills_dir).unwrap_or_else(|e| {
                    tracing::warn!("Failed to load skill catalog: {e}");
                    aether_project::PromptCatalog::empty()
                })
            }
            None => aether_project::PromptCatalog::empty(),
        };
        self.read_rule_state = prompt_rule_matcher::PromptRuleMatcher::new(catalog);
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
- **Rename symbol** (refactor across codebase): `lsp_rename`
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
    pub async fn grep(
        &self,
        request: Parameters<GrepInput>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<GrepOutput>, String> {
        let Parameters(args) = request;
        notify_preview(
            &context,
            ToolDisplayMeta::new("Grep", format!("'{}'", args.pattern)),
        )
        .await;
        self.tools.grep(args).await.into_mcp()
    }

    #[doc = include_str!("tools/find/description.md")]
    #[tool]
    pub async fn find(
        &self,
        request: Parameters<FindInput>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<FindOutput>, String> {
        let Parameters(args) = request;
        notify_preview(
            &context,
            ToolDisplayMeta::new("Find", format!("'{}'", args.pattern)),
        )
        .await;
        self.tools.find(args).await.into_mcp()
    }

    #[doc = include_str!("tools/read_file/description.md")]
    #[tool]
    pub async fn read_file(
        &self,
        request: Parameters<ReadFileArgs>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<ReadFileResult>, String> {
        let Parameters(args) = request;
        notify_preview(
            &context,
            ToolDisplayMeta::new("Read file", basename(&args.file_path)),
        )
        .await;
        let file_path = args.file_path.clone();
        let mut result = self
            .tools
            .read_file(args)
            .await
            .map_err(|e| e.to_string())?;
        self.files_read.write().await.insert(file_path.clone());

        let total_lines = result.total_lines;
        let roots = self.roots.read().await;
        let matched = self.read_rule_state.get_matched_rules(&roots, &file_path);
        for rule in &matched {
            write!(
                result.content,
                "\n\n<system-reminder>\n{}\n</system-reminder>",
                rule.body
            )
            .unwrap();
        }

        if !matched.is_empty() {
            let rule_names: Vec<&str> = matched.iter().map(|r| r.name.as_str()).collect();
            let base = format!("{}, {total_lines} lines", basename(&file_path));
            let value = format!("{base} +rules: {}", rule_names.join(", "));
            result._meta = Some(ToolDisplayMeta::new("Read file", value).into());
        }

        Ok(Json(result))
    }

    #[doc = include_str!("tools/write_file/description.md")]
    #[tool]
    pub async fn write_file(
        &self,
        request: Parameters<WriteFileArgs>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<WriteFileResponse>, String> {
        let Parameters(args) = request;
        notify_preview(
            &context,
            ToolDisplayMeta::new("Write file", basename(&args.file_path)),
        )
        .await;

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
        context: RequestContext<RoleServer>,
    ) -> Result<Json<EditFileResponse>, String> {
        let Parameters(args) = request;
        notify_preview(
            &context,
            ToolDisplayMeta::new("Edit file", basename(&args.file_path)),
        )
        .await;

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
        context: RequestContext<RoleServer>,
    ) -> Result<Json<ListFilesResult>, String> {
        let Parameters(args) = request;
        let preview_value = args
            .path
            .as_deref()
            .map_or_else(|| ".".to_string(), basename);
        notify_preview(&context, ToolDisplayMeta::new("List files", preview_value)).await;
        self.tools.list_files(args).await.into_mcp()
    }

    #[doc = include_str!("tools/bash/description.md")]
    #[tool]
    pub async fn bash(
        &self,
        request: Parameters<BashInput>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<BashOutput>, String> {
        let Parameters(args) = request;
        notify_preview(
            &context,
            ToolDisplayMeta::new("Run command", truncate(&args.command, 40)),
        )
        .await;
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
        context: RequestContext<RoleServer>,
    ) -> Result<Json<WebFetchOutput>, String> {
        let Parameters(args) = request;
        notify_preview(
            &context,
            ToolDisplayMeta::new("Fetch URL", truncate(&args.url, 60)),
        )
        .await;
        self.web_fetcher.fetch(args).await.into_mcp()
    }

    #[doc = include_str!("tools/web_search/description.md")]
    #[tool]
    pub async fn web_search(
        &self,
        request: Parameters<WebSearchInput>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<WebSearchOutput>, String> {
        let Parameters(args) = request;
        notify_preview(
            &context,
            ToolDisplayMeta::new("Web search", format!("'{}'", args.query)),
        )
        .await;

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
        context: RequestContext<RoleServer>,
    ) -> Result<Json<LspSymbolOutput>, String> {
        let Parameters(input) = request;
        notify_preview(&context, ToolDisplayMeta::new("LSP symbol", &input.symbol)).await;
        let lsp = self.lsp.as_ref().ok_or("LSP not configured")?;
        execute_lsp_symbol(input, lsp.as_ref()).await.map(Json)
    }

    #[doc = include_str!("../lsp/tools/document_info/description.md")]
    #[tool]
    pub async fn lsp_document(
        &self,
        request: Parameters<LspDocumentInput>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<LspDocumentOutput>, String> {
        let Parameters(input) = request;
        notify_preview(
            &context,
            ToolDisplayMeta::new("LSP document", basename(&input.file_path)),
        )
        .await;
        let lsp = self.lsp.as_ref().ok_or("LSP not configured")?;
        execute_lsp_document(input, lsp.as_ref()).await.map(Json)
    }

    #[doc = include_str!("../lsp/tools/check_errors/description.md")]
    #[tool]
    pub async fn lsp_check_errors(
        &self,
        request: Parameters<LspDiagnosticsRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<LspDiagnosticsOutput>, String> {
        let Parameters(request) = request;
        let preview_value = match &request.input {
            LspDiagnosticsInput::Workspace {} => "workspace".to_string(),
            LspDiagnosticsInput::File { file_path } => basename(file_path),
        };
        notify_preview(&context, ToolDisplayMeta::new("LSP errors", preview_value)).await;
        let lsp = self.lsp.as_ref().ok_or("LSP not configured")?;
        execute_lsp_diagnostics(request, lsp.as_ref())
            .await
            .map(Json)
    }

    #[doc = include_str!("../lsp/tools/rename/description.md")]
    #[tool]
    pub async fn lsp_rename(
        &self,
        request: Parameters<LspRenameInput>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<LspRenameOutput>, String> {
        let Parameters(input) = request;
        notify_preview(&context, ToolDisplayMeta::new("LSP rename", &input.symbol)).await;
        let lsp = self.lsp.as_ref().ok_or("LSP not configured")?;
        execute_lsp_rename(input, lsp.as_ref()).await.map(Json)
    }
}

impl Default for CodingMcp<DefaultCodingTools> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "test-helpers")]
impl<T: CodingTools + 'static> CodingMcp<T> {
    /// Read a file and track it in the read set (test helper, no MCP context needed).
    pub async fn test_read_file(&self, args: ReadFileArgs) -> Result<Json<ReadFileResult>, String> {
        let file_path = args.file_path.clone();
        let mut result = self
            .tools
            .read_file(args)
            .await
            .map_err(|e| e.to_string())?;
        self.files_read.write().await.insert(file_path.clone());

        let total_lines = result.total_lines;
        let roots = self.roots.read().await;
        let matched = self.read_rule_state.get_matched_rules(&roots, &file_path);
        for rule in &matched {
            write!(
                result.content,
                "\n\n<system-reminder>\n{}\n</system-reminder>",
                rule.body
            )
            .unwrap();
        }

        if !matched.is_empty() {
            let rule_names: Vec<&str> = matched.iter().map(|r| r.name.as_str()).collect();
            let base = format!("{}, {total_lines} lines", basename(&file_path));
            let value = format!("{base} +rules: {}", rule_names.join(", "));
            result._meta = Some(ToolDisplayMeta::new("Read file", value).into());
        }

        Ok(Json(result))
    }

    /// Write a file with read-before-write safety check (test helper, no MCP context needed).
    pub async fn test_write_file(
        &self,
        args: WriteFileArgs,
    ) -> Result<Json<WriteFileResponse>, String> {
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
        self.tools
            .write_file(args)
            .await
            .map(Json)
            .map_err(|e| e.to_string())
    }

    /// Edit a file with read-before-edit safety check (test helper, no MCP context needed).
    pub async fn test_edit_file(
        &self,
        args: EditFileArgs,
    ) -> Result<Json<EditFileResponse>, String> {
        {
            let files_read = self.files_read.read().await;
            if !files_read.contains(&args.file_path) {
                return Err(format!(
                    "Safety check failed: You must use read_file on '{}' before editing it. This ensures you understand the current file contents before making changes.",
                    args.file_path
                ));
            }
        }
        self.tools
            .edit_file(args)
            .await
            .map(Json)
            .map_err(|e| e.to_string())
    }
}
