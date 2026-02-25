//! LSP call hierarchy tool
//!
//! This module provides call hierarchy operations:
//! - incoming: Find functions/methods that call the given item
//! - outgoing: Find functions/methods that the given item calls

use lsp_types::CallHierarchyItem;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::lsp::common::{LocationResult, uri_to_path};
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
    #[serde(skip_serializing)]
    #[schemars(skip)]
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
            context: None,
        };
        let selection_range = LocationResult {
            file_path: file_path.clone(),
            start_line: item.selection_range.start.line + 1,
            start_column: item.selection_range.start.character + 1,
            end_line: item.selection_range.end.line + 1,
            end_column: item.selection_range.end.character + 1,
            context: None,
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

/// A call site result
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CallSiteResult {
    /// The item making or receiving the call
    pub item: CallHierarchyItemResult,
    /// The locations where calls occur
    pub call_sites: Vec<LocationResult>,
}

/// Convert LSP incoming calls to serializable `CallSiteResult`s.
pub fn convert_incoming_calls(
    incoming: Vec<lsp_types::CallHierarchyIncomingCall>,
) -> Vec<CallSiteResult> {
    incoming
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
                        context: None,
                    }
                })
                .collect();
            CallSiteResult {
                item: CallHierarchyItemResult::from(call.from),
                call_sites,
            }
        })
        .collect()
}

/// Convert LSP outgoing calls to serializable `CallSiteResult`s.
pub fn convert_outgoing_calls(
    outgoing: Vec<lsp_types::CallHierarchyOutgoingCall>,
) -> Vec<CallSiteResult> {
    outgoing
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
                    context: None,
                })
                .collect();
            CallSiteResult {
                item: CallHierarchyItemResult::from(call.to),
                call_sites,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use lsp_types::{CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall};

    fn make_item(name: &str, line: u32) -> CallHierarchyItem {
        CallHierarchyItem {
            name: name.to_string(),
            kind: lsp_types::SymbolKind::FUNCTION,
            tags: None,
            detail: None,
            uri: lsp_types::Uri::from_str("file:///src/lib.rs").unwrap(),
            range: lsp_types::Range {
                start: lsp_types::Position { line, character: 0 },
                end: lsp_types::Position {
                    line: line + 5,
                    character: 1,
                },
            },
            selection_range: lsp_types::Range {
                start: lsp_types::Position { line, character: 3 },
                end: lsp_types::Position {
                    line,
                    character: 3 + name.len() as u32,
                },
            },
            data: None,
        }
    }

    fn make_range(line: u32, col: u32) -> lsp_types::Range {
        lsp_types::Range {
            start: lsp_types::Position {
                line,
                character: col,
            },
            end: lsp_types::Position {
                line,
                character: col + 5,
            },
        }
    }

    #[test]
    fn test_convert_incoming_calls() {
        let incoming = vec![CallHierarchyIncomingCall {
            from: make_item("caller_fn", 10),
            from_ranges: vec![make_range(12, 4), make_range(14, 8)],
        }];

        let result = convert_incoming_calls(incoming);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].item.name, "caller_fn");
        assert_eq!(result[0].call_sites.len(), 2);
        // Lines are 1-indexed in the output
        assert_eq!(result[0].call_sites[0].start_line, 13); // 12 + 1
        assert_eq!(result[0].call_sites[1].start_line, 15); // 14 + 1
    }

    #[test]
    fn test_convert_incoming_calls_empty() {
        let result = convert_incoming_calls(vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_convert_outgoing_calls() {
        let outgoing = vec![CallHierarchyOutgoingCall {
            to: make_item("callee_fn", 20),
            from_ranges: vec![make_range(5, 10)],
        }];

        let result = convert_outgoing_calls(outgoing);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].item.name, "callee_fn");
        assert_eq!(result[0].call_sites.len(), 1);
        assert_eq!(result[0].call_sites[0].start_line, 6); // 5 + 1
    }

    #[test]
    fn test_convert_outgoing_calls_empty() {
        let result = convert_outgoing_calls(vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_lsp_item_is_not_serialized() {
        let item = CallHierarchyItemResult::from(make_item("my_fn", 5));
        let json = serde_json::to_string(&item).unwrap();
        assert!(
            !json.contains("lsp_item"),
            "lsp_item should be excluded from serialized output, got: {json}"
        );
    }
}
