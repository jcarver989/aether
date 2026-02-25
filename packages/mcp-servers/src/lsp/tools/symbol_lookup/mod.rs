//! Consolidated LSP symbol lookup tool
//!
//! This module provides a unified interface for symbol-based LSP operations:
//! - definition: Go to the definition of a symbol
//! - implementation: Go to the implementation of an interface/trait
//! - references: Find all references to a symbol
//! - hover: Get type and documentation info for a symbol
//! - `incoming_calls` / `outgoing_calls`: One-step call hierarchy lookup

use lsp_types::GotoDefinitionResponse;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::lsp::common::{LocationResult, uri_to_path};
use crate::lsp::registry::LspRegistry;

use super::call_hierarchy::CallSiteResult;
use super::resolve_symbol_position;

/// Direction for one-step call hierarchy lookups.
enum CallDirection {
    Incoming,
    Outgoing,
}

impl CallDirection {
    fn as_str(&self) -> &'static str {
        match self {
            CallDirection::Incoming => "incoming",
            CallDirection::Outgoing => "outgoing",
        }
    }
}

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
    /// Find functions/methods that call this symbol (one-step call hierarchy)
    IncomingCalls,
    /// Find functions/methods that this symbol calls (one-step call hierarchy)
    OutgoingCalls,
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
    /// Optional 1-indexed line number. When provided, skips automatic symbol resolution
    /// (faster). When omitted, the line is resolved via document symbols.
    #[serde(default)]
    pub line: Option<u32>,
    /// Whether to include the declaration in references results (default: true, only used for references operation)
    #[serde(default = "default_true")]
    pub include_declaration: bool,
}

fn default_true() -> bool {
    true
}

/// Output from the `lsp_symbol` tool
#[derive(Debug, Clone, Default, Serialize, JsonSchema)]
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
    /// Call site results (for `incoming_calls` / `outgoing_calls` operations)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_sites: Option<Vec<CallSiteResult>>,
    /// Total count of results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_count: Option<usize>,
}

impl LspSymbolOutput {
    fn with_locations(operation: &str, locations: Vec<LocationResult>) -> Self {
        let total_count = locations.len();
        Self {
            operation: operation.to_string(),
            locations: Some(locations),
            total_count: Some(total_count),
            ..Self::default()
        }
    }
}

/// Resolve the line number for a symbol, using the explicit `line` if provided
/// or falling back to automatic document symbol resolution.
async fn resolve_line(
    file_path: &str,
    symbol: &str,
    explicit_line: Option<u32>,
    tools: &LspRegistry,
) -> Result<u32, String> {
    match explicit_line {
        Some(line) => Ok(line),
        None => resolve_symbol_position(file_path, symbol, tools)
            .await
            .map_err(|e| e.to_string()),
    }
}

/// Execute the `lsp_symbol` operation
pub async fn execute_lsp_symbol(
    input: LspSymbolInput,
    registry: &LspRegistry,
) -> Result<LspSymbolOutput, String> {
    let line = resolve_line(&input.file_path, &input.symbol, input.line, registry).await?;

    match input.operation {
        SymbolLookupOperation::Definition => {
            let resolved = registry
                .resolve_symbol(&input.file_path, &input.symbol, line)
                .await
                .map_err(|e| e.to_string())?;
            let response = resolved
                .client
                .goto_definition(resolved.uri, resolved.line, resolved.column)
                .await
                .map_err(|e| e.to_string())?;
            let locations = definition_response_to_locations(response);
            Ok(LspSymbolOutput::with_locations("definition", locations))
        }
        SymbolLookupOperation::Implementation => {
            let resolved = registry
                .resolve_symbol(&input.file_path, &input.symbol, line)
                .await
                .map_err(|e| e.to_string())?;
            let response = resolved
                .client
                .goto_implementation(resolved.uri, resolved.line, resolved.column)
                .await
                .map_err(|e| e.to_string())?;
            let locations = definition_response_to_locations(response);
            Ok(LspSymbolOutput::with_locations("implementation", locations))
        }
        SymbolLookupOperation::References => {
            let resolved = registry
                .resolve_symbol(&input.file_path, &input.symbol, line)
                .await
                .map_err(|e| e.to_string())?;
            let lsp_locations = resolved
                .client
                .find_references(resolved.uri, resolved.line, resolved.column, input.include_declaration)
                .await
                .map_err(|e| e.to_string())?;
            let locations: Vec<LocationResult> = lsp_locations
                .iter()
                .map(LocationResult::from_location)
                .collect();
            Ok(LspSymbolOutput::with_locations("references", locations))
        }
        SymbolLookupOperation::Hover => {
            let resolved = registry
                .resolve_symbol(&input.file_path, &input.symbol, line)
                .await
                .map_err(|e| e.to_string())?;
            let hover = resolved
                .client
                .hover(resolved.uri, resolved.line, resolved.column)
                .await
                .map_err(|e| e.to_string())?;
            Ok(LspSymbolOutput {
                operation: "hover".to_string(),
                hover_contents: hover.map(|h| format_hover_contents(&h)),
                ..LspSymbolOutput::default()
            })
        }
        SymbolLookupOperation::IncomingCalls => {
            execute_one_step_call_hierarchy(registry, &input.file_path, &input.symbol, line, CallDirection::Incoming)
                .await
        }
        SymbolLookupOperation::OutgoingCalls => {
            execute_one_step_call_hierarchy(registry, &input.file_path, &input.symbol, line, CallDirection::Outgoing)
                .await
        }
    }
}

/// Perform a one-step call hierarchy: prepare + query in a single operation.
async fn execute_one_step_call_hierarchy(
    registry: &LspRegistry,
    file_path: &str,
    symbol: &str,
    line: u32,
    direction: CallDirection,
) -> Result<LspSymbolOutput, String> {
    let resolved = registry
        .resolve_symbol(file_path, symbol, line)
        .await
        .map_err(|e| e.to_string())?;

    let items = resolved
        .client
        .prepare_call_hierarchy(resolved.uri, resolved.line, resolved.column)
        .await
        .map_err(|e| e.to_string())?;

    let Some(item) = items.into_iter().next() else {
        return Ok(LspSymbolOutput {
            operation: direction.as_str().to_string(),
            call_sites: Some(Vec::new()),
            total_count: Some(0),
            ..LspSymbolOutput::default()
        });
    };

    // For incoming/outgoing calls, we need a client for the item's file.
    // The item may be in a different file than the original request.
    let item_file_path = uri_to_path(&item.uri);
    let item_client = registry
        .require_client(&item_file_path)
        .await
        .map_err(|e| e.to_string())?;

    let calls = match direction {
        CallDirection::Incoming => {
            let incoming = item_client
                .incoming_calls(item)
                .await
                .map_err(|e| e.to_string())?;
            super::call_hierarchy::convert_incoming_calls(incoming)
        }
        CallDirection::Outgoing => {
            let outgoing = item_client
                .outgoing_calls(item)
                .await
                .map_err(|e| e.to_string())?;
            super::call_hierarchy::convert_outgoing_calls(outgoing)
        }
    };

    let total_count = calls.iter().map(|c| c.call_sites.len()).sum();
    Ok(LspSymbolOutput {
        operation: direction.as_str().to_string(),
        call_sites: Some(calls),
        total_count: Some(total_count),
        ..LspSymbolOutput::default()
    })
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
