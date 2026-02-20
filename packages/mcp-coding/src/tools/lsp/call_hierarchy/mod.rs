//! LSP call hierarchy tool
//!
//! This module provides call hierarchy operations:
//! - incoming: Find functions/methods that call the given item
//! - outgoing: Find functions/methods that the given item calls

use lsp_types::CallHierarchyItem;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::lsp::common::{LocationResult, uri_to_path};
use crate::tools_trait::CodingTools;
use aether_lspd::symbol_kind_to_string;

/// A serializable representation of a `CallHierarchyItem`
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CallHierarchyItemResult {
    /// The name of the symbol
    pub name: String,
    /// The kind of the symbol (function, method, etc.)
    pub kind: String,
    /// Additional detail (e.g., signature)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// The file path containing this symbol
    pub file_path: String,
    /// The range of the entire symbol
    pub range: LocationResult,
    /// The range of the symbol name
    pub selection_range: LocationResult,
    /// JSON-serialized `CallHierarchyItem` for roundtrip (used internally)
    pub lsp_item: String,
}

impl From<CallHierarchyItem> for CallHierarchyItemResult {
    fn from(item: CallHierarchyItem) -> Self {
        let file_path = uri_to_path(&item.uri);
        let range = LocationResult {
            file_path: file_path.clone(),
            start_line: item.range.start.line + 1,
            start_column: item.range.start.character + 1,
            end_line: item.range.end.line + 1,
            end_column: item.range.end.character + 1,
        };
        let selection_range = LocationResult {
            file_path: file_path.clone(),
            start_line: item.selection_range.start.line + 1,
            start_column: item.selection_range.start.character + 1,
            end_line: item.selection_range.end.line + 1,
            end_column: item.selection_range.end.character + 1,
        };
        let lsp_item = serde_json::to_string(&item).unwrap_or_default();

        Self {
            name: item.name,
            kind: symbol_kind_to_string(item.kind).to_string(),
            detail: item.detail,
            file_path,
            range,
            selection_range,
            lsp_item,
        }
    }
}

impl TryFrom<CallHierarchyItemResult> for CallHierarchyItem {
    type Error = String;

    fn try_from(result: CallHierarchyItemResult) -> Result<Self, String> {
        serde_json::from_str(&result.lsp_item)
            .map_err(|e| format!("Failed to deserialize CallHierarchyItem: {e}"))
    }
}

/// The direction of the call hierarchy traversal
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CallHierarchyDirection {
    /// Find functions/methods that call this item
    Incoming,
    /// Find functions/methods that this item calls
    Outgoing,
}

/// Input for the `lsp_call_hierarchy` tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct LspCallHierarchyInput {
    /// The direction of traversal
    pub direction: CallHierarchyDirection,
    /// The call hierarchy item to query (from `lsp_symbol` `prepare_call_hierarchy`)
    pub item: CallHierarchyItemResult,
}

/// A call site result
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CallSiteResult {
    /// The item making or receiving the call
    pub item: CallHierarchyItemResult,
    /// The locations where calls occur
    pub call_sites: Vec<LocationResult>,
}

/// Output from the `lsp_call_hierarchy` tool
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LspCallHierarchyOutput {
    /// The direction of traversal that was performed
    pub direction: String,
    /// The call results
    pub calls: Vec<CallSiteResult>,
    /// Total count of call sites
    pub total_count: usize,
}

/// Execute the `lsp_call_hierarchy` operation
pub async fn execute_lsp_call_hierarchy<T: CodingTools>(
    input: LspCallHierarchyInput,
    tools: &T,
) -> Result<LspCallHierarchyOutput, String> {
    let item: CallHierarchyItem = input.item.try_into()?;

    match input.direction {
        CallHierarchyDirection::Incoming => {
            let incoming = tools
                .incoming_calls(item)
                .await
                .map_err(|e| e.to_string())?;

            let calls: Vec<CallSiteResult> = incoming
                .into_iter()
                .map(|call| {
                    let call_sites = call
                        .from_ranges
                        .iter()
                        .map(|range| {
                            let file_path = uri_to_path(&call.from.uri);
                            LocationResult {
                                file_path,
                                start_line: range.start.line + 1,
                                start_column: range.start.character + 1,
                                end_line: range.end.line + 1,
                                end_column: range.end.character + 1,
                            }
                        })
                        .collect();
                    CallSiteResult {
                        item: CallHierarchyItemResult::from(call.from),
                        call_sites,
                    }
                })
                .collect();

            let total_count = calls.iter().map(|c| c.call_sites.len()).sum();

            Ok(LspCallHierarchyOutput {
                direction: "incoming".to_string(),
                calls,
                total_count,
            })
        }
        CallHierarchyDirection::Outgoing => {
            let outgoing = tools
                .outgoing_calls(item)
                .await
                .map_err(|e| e.to_string())?;

            let calls: Vec<CallSiteResult> = outgoing
                .into_iter()
                .map(|call| {
                    let file_path = uri_to_path(&call.to.uri);
                    let call_sites = call
                        .from_ranges
                        .iter()
                        .map(|range| LocationResult {
                            file_path: file_path.clone(),
                            start_line: range.start.line + 1,
                            start_column: range.start.character + 1,
                            end_line: range.end.line + 1,
                            end_column: range.end.character + 1,
                        })
                        .collect();
                    CallSiteResult {
                        item: CallHierarchyItemResult::from(call.to),
                        call_sites,
                    }
                })
                .collect();

            let total_count = calls.iter().map(|c| c.call_sites.len()).sum();

            Ok(LspCallHierarchyOutput {
                direction: "outgoing".to_string(),
                calls,
                total_count,
            })
        }
    }
}
