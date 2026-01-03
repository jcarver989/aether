//! LSP document info tool
//!
//! This module provides document-level LSP operations:
//! - symbols: Get all symbols in a document

use lsp_types::DocumentSymbolResponse;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use aether_lspd::symbol_kind_to_string;
use crate::coding::lsp::common::LocationResult;
use crate::coding::tools_trait::CodingTools;

/// The operation to perform on a document
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DocumentInfoOperation {
    /// Get all symbols in the document
    Symbols,
}

/// Input for the lsp_document tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct LspDocumentInput {
    /// The operation to perform
    pub operation: DocumentInfoOperation,
    /// The file path to analyze
    pub file_path: String,
}

/// A document symbol with hierarchical structure
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSymbolResult {
    /// The name of the symbol
    pub name: String,
    /// The kind of the symbol (function, class, etc.)
    pub kind: String,
    /// Additional detail about the symbol
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// The range of the entire symbol
    pub range: LocationResult,
    /// The range of the symbol name
    pub selection_range: LocationResult,
    /// Child symbols (for hierarchical structure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<DocumentSymbolResult>>,
}

/// Output from the lsp_document tool
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LspDocumentOutput {
    /// The operation that was performed
    pub operation: String,
    /// The symbols in the document (for symbols operation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbols: Option<Vec<DocumentSymbolResult>>,
    /// Total count of top-level symbols
    pub total_count: usize,
}

/// Execute the lsp_document operation
pub async fn execute_lsp_document<T: CodingTools>(
    input: LspDocumentInput,
    tools: &T,
) -> Result<LspDocumentOutput, String> {
    match input.operation {
        DocumentInfoOperation::Symbols => {
            let response = tools
                .document_symbol(&input.file_path)
                .await
                .map_err(|e| e.to_string())?;

            let symbols = convert_document_symbols(&input.file_path, response);
            let total_count = symbols.len();

            Ok(LspDocumentOutput {
                operation: "symbols".to_string(),
                symbols: Some(symbols),
                total_count,
            })
        }
    }
}

/// Convert DocumentSymbolResponse to our result format
fn convert_document_symbols(
    file_path: &str,
    response: DocumentSymbolResponse,
) -> Vec<DocumentSymbolResult> {
    match response {
        DocumentSymbolResponse::Flat(symbols) => symbols
            .into_iter()
            .map(|sym| {
                let range = LocationResult {
                    file_path: file_path.to_string(),
                    start_line: sym.location.range.start.line + 1,
                    start_column: sym.location.range.start.character + 1,
                    end_line: sym.location.range.end.line + 1,
                    end_column: sym.location.range.end.character + 1,
                };
                DocumentSymbolResult {
                    name: sym.name,
                    kind: symbol_kind_to_string(sym.kind).to_string(),
                    detail: None,
                    range: range.clone(),
                    selection_range: range,
                    children: None,
                }
            })
            .collect(),
        DocumentSymbolResponse::Nested(symbols) => symbols
            .into_iter()
            .map(|sym| convert_document_symbol(file_path, sym))
            .collect(),
    }
}

/// Convert a single DocumentSymbol to our result format (recursive for children)
fn convert_document_symbol(
    file_path: &str,
    sym: lsp_types::DocumentSymbol,
) -> DocumentSymbolResult {
    let range = LocationResult {
        file_path: file_path.to_string(),
        start_line: sym.range.start.line + 1,
        start_column: sym.range.start.character + 1,
        end_line: sym.range.end.line + 1,
        end_column: sym.range.end.character + 1,
    };
    let selection_range = LocationResult {
        file_path: file_path.to_string(),
        start_line: sym.selection_range.start.line + 1,
        start_column: sym.selection_range.start.character + 1,
        end_line: sym.selection_range.end.line + 1,
        end_column: sym.selection_range.end.character + 1,
    };

    let children = sym.children.map(|children| {
        children
            .into_iter()
            .map(|child| convert_document_symbol(file_path, child))
            .collect()
    });

    DocumentSymbolResult {
        name: sym.name,
        kind: symbol_kind_to_string(sym.kind).to_string(),
        detail: sym.detail,
        range,
        selection_range,
        children,
    }
}
