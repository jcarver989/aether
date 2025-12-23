//! LSP find references tool for locating all usages of a symbol

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{LocationResult, parse_line};
use crate::coding::tools_trait::CodingTools;

/// Input for the lsp_find_references tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct LspFindReferencesInput {
    /// The file path containing the symbol
    pub file_path: String,
    /// The symbol name to look up (e.g., "HashMap", "spawn", "LspClient")
    pub symbol: String,
    /// Line number where the symbol appears (1-indexed, as shown by the read_file tool)
    pub line: String,
    /// Whether to include the declaration in the results (default: true)
    #[serde(default = "default_include_declaration")]
    pub include_declaration: bool,
}

fn default_include_declaration() -> bool {
    true
}

/// Output from the lsp_find_references tool
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LspFindReferencesOutput {
    /// List of reference locations
    pub references: Vec<LocationResult>,
    /// Total count of references found
    pub total_count: usize,
}

/// Execute the lsp_find_references operation
pub async fn execute_lsp_find_references<T: CodingTools>(
    input: LspFindReferencesInput,
    tools: &T,
) -> Result<LspFindReferencesOutput, String> {
    let line = parse_line(&input.line)?;
    let lsp_locations = tools
        .find_references(
            &input.file_path,
            &input.symbol,
            line,
            input.include_declaration,
        )
        .await?;
    let references: Vec<LocationResult> = lsp_locations
        .iter()
        .map(LocationResult::from_location)
        .collect();
    let total_count = references.len();
    Ok(LspFindReferencesOutput {
        references,
        total_count,
    })
}
