//! LSP workspace symbol tool for searching symbols across the codebase

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::coding::lsp::common::{LocationResult, symbol_kind_to_string};
use crate::coding::tools_trait::CodingTools;

/// Input for the lsp_workspace_symbol tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct LspWorkspaceSymbolInput {
    /// The search query (fuzzy matching is used by most language servers)
    pub query: String,
}

/// A symbol found in the workspace
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SymbolResult {
    /// Symbol name (e.g., "LspClient", "spawn")
    pub name: String,
    /// Symbol kind (e.g., "function", "struct", "enum", "method")
    pub kind: String,
    /// Container name (e.g., the struct a method belongs to)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
    /// Location in source code
    pub location: LocationResult,
}

/// Output from the lsp_workspace_symbol tool
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LspWorkspaceSymbolOutput {
    /// List of symbols matching the query
    pub symbols: Vec<SymbolResult>,
    /// Total count of symbols found
    pub total_count: usize,
}

/// Execute the lsp_workspace_symbol operation
pub async fn execute_lsp_workspace_symbol<T: CodingTools>(
    input: LspWorkspaceSymbolInput,
    tools: &T,
) -> Result<LspWorkspaceSymbolOutput, String> {
    let lsp_symbols = tools
        .workspace_symbol(&input.query)
        .await
        .map_err(|e| e.to_string())?;
    let symbols: Vec<SymbolResult> = lsp_symbols
        .iter()
        .map(|s| SymbolResult {
            name: s.name.clone(),
            kind: symbol_kind_to_string(s.kind),
            container_name: s.container_name.clone(),
            location: LocationResult::from_location(&s.location),
        })
        .collect();
    let total_count = symbols.len();
    Ok(LspWorkspaceSymbolOutput {
        symbols,
        total_count,
    })
}
