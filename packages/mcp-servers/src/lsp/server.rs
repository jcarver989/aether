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
use std::sync::Arc;
use std::{fmt::Debug, path::PathBuf};
use tokio::sync::RwLock;

use super::registry::LspRegistry;
use super::tools::check_errors::{
    LspDiagnosticsOutput, LspDiagnosticsRequest, execute_lsp_diagnostics,
};
use super::tools::document_info::{LspDocumentInput, LspDocumentOutput, execute_lsp_document};
use super::tools::rename::{LspRenameInput, LspRenameOutput, execute_lsp_rename};
use super::tools::symbol_lookup::{LspSymbolInput, LspSymbolOutput, execute_lsp_symbol};
use super::tools::workspace_search::{
    LspWorkspaceSearchInput, LspWorkspaceSearchOutput, execute_lsp_workspace_search,
};

/// CLI arguments for `LspMcp` server
#[derive(Debug, Clone, Parser)]
pub struct LspMcpArgs {
    /// Root directory for workspace (used for LSP initialization)
    #[arg(long = "root-dir")]
    pub root_dir: Option<PathBuf>,
}

impl LspMcpArgs {
    pub fn from_args(args: Vec<String>) -> Result<Self, String> {
        let mut full_args = vec!["lsp-mcp".to_string()];
        full_args.extend(args);

        Self::try_parse_from(full_args)
            .map_err(|e| format!("Failed to parse LspMcp arguments: {e}"))
    }
}

/// MCP server that exposes LSP-based code intelligence tools.
///
/// Provides language-aware symbol lookup, document structure, diagnostics,
/// and call hierarchy — without bundling file I/O tools.
pub struct LspMcp {
    tool_router: ToolRouter<Self>,
    lsp: Arc<LspRegistry>,
    roots: RwLock<Vec<PathBuf>>,
}

impl Debug for LspMcp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspMcp").finish_non_exhaustive()
    }
}

impl LspMcp {
    /// Create a standalone `LspMcp` with its own `LspRegistry`.
    pub fn new(root_dir: PathBuf) -> Self {
        let registry = LspRegistry::new_and_spawn(root_dir.clone());
        Self {
            tool_router: Self::tool_router(),
            lsp: registry,
            roots: RwLock::new(vec![root_dir]),
        }
    }

    /// Create from parsed CLI arguments.
    pub fn from_args(args: Vec<String>) -> Result<Self, String> {
        let parsed = LspMcpArgs::from_args(args)?;
        let root_dir = parsed.root_dir.unwrap_or_else(|| PathBuf::from("."));
        Ok(Self::new(root_dir))
    }

    /// Set workspace roots.
    pub fn with_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.roots = RwLock::new(roots);
        self
    }

    /// Set a single workspace root directory.
    pub fn with_root_dir(self, root_dir: PathBuf) -> Self {
        self.with_roots(vec![root_dir])
    }

    fn get_workspace_root(&self) -> Option<PathBuf> {
        self.roots
            .try_read()
            .ok()
            .and_then(|roots| roots.first().cloned())
    }

    fn build_instructions(&self) -> String {
        let base = r"# LSP MCP Server

Code intelligence tools powered by Language Server Protocol.

## Quick Reference

- **Errors & warnings** (instant check without build): `lsp_check_errors`
- **Code symbols** (definitions, usages, types): `lsp_symbol`
- **Find symbol across workspace** (don't know the file?): `lsp_workspace_search`
- **File structure** (what's in this file?): `lsp_document`
- **Call relationships** (who calls X?): `lsp_symbol` with incoming_calls/outgoing_calls operation
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
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for LspMcp {
    fn get_info(&self) -> ServerInfo {
        let instructions = self.build_instructions();
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("lsp-mcp", "0.1.0"))
            .with_instructions(instructions)
    }
}

#[tool_router]
impl LspMcp {
    #[doc = include_str!("tools/symbol_lookup/description.md")]
    #[tool]
    pub async fn lsp_symbol(
        &self,
        request: Parameters<LspSymbolInput>,
    ) -> Result<Json<LspSymbolOutput>, String> {
        let Parameters(input) = request;
        execute_lsp_symbol(input, self.lsp.as_ref()).await.map(Json)
    }

    #[doc = include_str!("tools/document_info/description.md")]
    #[tool]
    pub async fn lsp_document(
        &self,
        request: Parameters<LspDocumentInput>,
    ) -> Result<Json<LspDocumentOutput>, String> {
        let Parameters(input) = request;
        execute_lsp_document(input, self.lsp.as_ref())
            .await
            .map(Json)
    }

    #[doc = include_str!("tools/check_errors/description.md")]
    #[tool]
    pub async fn lsp_check_errors(
        &self,
        request: Parameters<LspDiagnosticsRequest>,
    ) -> Result<Json<LspDiagnosticsOutput>, String> {
        let Parameters(request) = request;
        execute_lsp_diagnostics(request, self.lsp.as_ref())
            .await
            .map(Json)
    }

    #[doc = include_str!("tools/workspace_search/description.md")]
    #[tool]
    pub async fn lsp_workspace_search(
        &self,
        request: Parameters<LspWorkspaceSearchInput>,
    ) -> Result<Json<LspWorkspaceSearchOutput>, String> {
        let Parameters(input) = request;
        execute_lsp_workspace_search(input, self.lsp.as_ref())
            .await
            .map(Json)
    }

    #[doc = include_str!("tools/rename/description.md")]
    #[tool]
    pub async fn lsp_rename(
        &self,
        request: Parameters<LspRenameInput>,
    ) -> Result<Json<LspRenameOutput>, String> {
        let Parameters(input) = request;
        execute_lsp_rename(input, self.lsp.as_ref()).await.map(Json)
    }
}
