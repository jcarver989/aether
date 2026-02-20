//! Consolidated LSP symbol lookup tool
//!
//! This module provides a unified interface for symbol-based LSP operations:
//! - definition: Go to the definition of a symbol
//! - implementation: Go to the implementation of an interface/trait
//! - references: Find all references to a symbol
//! - hover: Get type and documentation info for a symbol
//! - `prepare_call_hierarchy`: Get call hierarchy items for a symbol

use lsp_types::GotoDefinitionResponse;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::lsp::common::{LocationResult, parse_line, uri_to_path};
use crate::tools_trait::CodingTools;

use super::call_hierarchy::CallHierarchyItemResult;

/// The operation to perform on a symbol
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SymbolLookupOperation {
    /// Go to the definition of the symbol
    Definition,
    /// Go to the implementation of an interface/trait method
    Implementation,
    /// Find all references to the symbol
    References,
    /// Get hover information (type, documentation) for the symbol
    Hover,
    /// Prepare call hierarchy items for the symbol (used with `lsp_call_hierarchy`)
    PrepareCallHierarchy,
}

/// Input for the `lsp_symbol` tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct LspSymbolInput {
    /// The operation to perform
    pub operation: SymbolLookupOperation,
    /// The file path containing the symbol
    pub file_path: String,
    /// The symbol name to look up (e.g., "`HashMap`", "spawn", "`LspClient`")
    pub symbol: String,
    /// Line number where the symbol appears (1-indexed, as shown by the `read_file` tool)
    pub line: String,
    /// Whether to include the declaration in references results (default: true, only used for references operation)
    #[serde(default = "default_true")]
    pub include_declaration: bool,
}

fn default_true() -> bool {
    true
}

/// Output from the `lsp_symbol` tool
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LspSymbolOutput {
    /// The operation that was performed
    pub operation: String,
    /// Location results (for definition, implementation, references)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locations: Option<Vec<LocationResult>>,
    /// Hover contents as markdown (for hover operation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover_contents: Option<String>,
    /// Call hierarchy items (for `prepare_call_hierarchy` operation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_hierarchy_items: Option<Vec<CallHierarchyItemResult>>,
    /// Total count of results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_count: Option<usize>,
}

/// Execute the `lsp_symbol` operation
pub async fn execute_lsp_symbol<T: CodingTools>(
    input: LspSymbolInput,
    tools: &T,
) -> Result<LspSymbolOutput, String> {
    let line = parse_line(&input.line)?;

    match input.operation {
        SymbolLookupOperation::Definition => {
            let response = tools
                .goto_definition(&input.file_path, &input.symbol, line)
                .await
                .map_err(|e| e.to_string())?;
            let locations = definition_response_to_locations(response);
            let total_count = locations.len();
            Ok(LspSymbolOutput {
                operation: "definition".to_string(),
                locations: Some(locations),
                hover_contents: None,
                call_hierarchy_items: None,
                total_count: Some(total_count),
            })
        }
        SymbolLookupOperation::Implementation => {
            let response = tools
                .goto_implementation(&input.file_path, &input.symbol, line)
                .await
                .map_err(|e| e.to_string())?;
            let locations = definition_response_to_locations(response);
            let total_count = locations.len();
            Ok(LspSymbolOutput {
                operation: "implementation".to_string(),
                locations: Some(locations),
                hover_contents: None,
                call_hierarchy_items: None,
                total_count: Some(total_count),
            })
        }
        SymbolLookupOperation::References => {
            let lsp_locations = tools
                .find_references(
                    &input.file_path,
                    &input.symbol,
                    line,
                    input.include_declaration,
                )
                .await
                .map_err(|e| e.to_string())?;
            let locations: Vec<LocationResult> = lsp_locations
                .iter()
                .map(LocationResult::from_location)
                .collect();
            let total_count = locations.len();
            Ok(LspSymbolOutput {
                operation: "references".to_string(),
                locations: Some(locations),
                hover_contents: None,
                call_hierarchy_items: None,
                total_count: Some(total_count),
            })
        }
        SymbolLookupOperation::Hover => {
            let hover = tools
                .hover(&input.file_path, &input.symbol, line)
                .await
                .map_err(|e| e.to_string())?;
            let hover_contents = hover.map(|h| format_hover_contents(&h));
            Ok(LspSymbolOutput {
                operation: "hover".to_string(),
                locations: None,
                hover_contents,
                call_hierarchy_items: None,
                total_count: None,
            })
        }
        SymbolLookupOperation::PrepareCallHierarchy => {
            let items = tools
                .prepare_call_hierarchy(&input.file_path, &input.symbol, line)
                .await
                .map_err(|e| e.to_string())?;
            let call_hierarchy_items: Vec<CallHierarchyItemResult> = items
                .into_iter()
                .map(CallHierarchyItemResult::from)
                .collect();
            let total_count = call_hierarchy_items.len();
            Ok(LspSymbolOutput {
                operation: "prepare_call_hierarchy".to_string(),
                locations: None,
                hover_contents: None,
                call_hierarchy_items: Some(call_hierarchy_items),
                total_count: Some(total_count),
            })
        }
    }
}

/// Convert `GotoDefinitionResponse` to a list of `LocationResult`
fn definition_response_to_locations(response: GotoDefinitionResponse) -> Vec<LocationResult> {
    match response {
        GotoDefinitionResponse::Scalar(loc) => vec![LocationResult::from_location(&loc)],
        GotoDefinitionResponse::Array(locs) => {
            locs.iter().map(LocationResult::from_location).collect()
        }
        GotoDefinitionResponse::Link(links) => links
            .iter()
            .map(|link| {
                let file_path = uri_to_path(&link.target_uri);
                LocationResult {
                    file_path,
                    start_line: link.target_selection_range.start.line + 1,
                    start_column: link.target_selection_range.start.character + 1,
                    end_line: link.target_selection_range.end.line + 1,
                    end_column: link.target_selection_range.end.character + 1,
                }
            })
            .collect(),
    }
}

/// Format hover contents to a readable string
fn format_hover_contents(hover: &lsp_types::Hover) -> String {
    use lsp_types::HoverContents;

    match &hover.contents {
        HoverContents::Scalar(marked) => format_marked_string(marked),
        HoverContents::Array(arr) => arr
            .iter()
            .map(format_marked_string)
            .collect::<Vec<_>>()
            .join("\n\n"),
        HoverContents::Markup(markup) => markup.value.clone(),
    }
}

/// Format a single `MarkedString`
fn format_marked_string(marked: &lsp_types::MarkedString) -> String {
    match marked {
        lsp_types::MarkedString::String(s) => s.clone(),
        lsp_types::MarkedString::LanguageString(ls) => {
            format!("```{}\n{}\n```", ls.language, ls.value)
        }
    }
}
