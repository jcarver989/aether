//! LSP tool for querying language server information
//!
//! This module provides an MCP tool that exposes LSP functionality to LLMs,
//! starting with diagnostics queries and extensible for future operations.

use lsp_types::{Diagnostic, GotoDefinitionResponse, Hover, Location, Uri};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::lsp::{FormattedDiagnostic, format_diagnostics};
use super::tools_trait::CodingTools;

/// LSP operations that can be performed via the tool
///
/// Uses `#[serde(tag = "operation")]` for extensibility - new operations
/// can be added without breaking existing callers.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum LspOperation {
    /// Get diagnostics (errors, warnings) from the language server
    GetDiagnostics {
        /// Optional: filter to specific file path. If not provided, returns all diagnostics.
        file_path: Option<String>,
    },
    /// Go to the definition of a symbol
    GoToDefinition {
        /// The file path containing the symbol
        file_path: String,
        /// The symbol name to look up (e.g., "HashMap", "spawn", "LspClient")
        symbol: String,
        /// Line number where the symbol appears (1-indexed, as shown by the read_file tool)
        line: String,
    },
    /// Find all references to a symbol
    FindReferences {
        /// The file path containing the symbol
        file_path: String,
        /// The symbol name to look up (e.g., "HashMap", "spawn", "LspClient")
        symbol: String,
        /// Line number where the symbol appears (1-indexed, as shown by the read_file tool)
        line: String,
        /// Whether to include the declaration in the results (default: true)
        #[serde(default = "default_include_declaration")]
        include_declaration: bool,
    },
    /// Get hover information (type, documentation) for a symbol
    Hover {
        /// The file path containing the symbol
        file_path: String,
        /// The symbol name to look up (e.g., "HashMap", "spawn", "LspClient")
        symbol: String,
        /// Line number where the symbol appears (1-indexed, as shown by the read_file tool)
        line: String,
    },
}

fn default_include_declaration() -> bool {
    true
}

/// Parse a line number string to u32
fn parse_line(s: &str) -> Result<u32, String> {
    s.trim()
        .parse()
        .map_err(|_| format!("Invalid line number: {}", s))
}

/// Input for the LSP tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LspInput {
    /// The LSP operation to perform
    #[serde(flatten)]
    pub operation: LspOperation,
}

/// A diagnostic formatted for LLM consumption
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LspDiagnostic {
    /// The file path
    pub file: String,
    /// Line number (1-indexed)
    pub line: u32,
    /// Column number (1-indexed)
    pub column: u32,
    /// Severity: "error", "warning", "info", or "hint"
    pub severity: String,
    /// The diagnostic message
    pub message: String,
    /// The source (e.g., "rustc", "clippy")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// The diagnostic code (e.g., "E0308")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl From<FormattedDiagnostic> for LspDiagnostic {
    fn from(d: FormattedDiagnostic) -> Self {
        Self {
            file: d.file,
            line: d.line,
            column: d.column,
            severity: d.severity.to_string(),
            message: d.message,
            source: d.source,
            code: d.code,
        }
    }
}

/// Output from the get_diagnostics operation
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetDiagnosticsOutput {
    /// List of diagnostics
    pub diagnostics: Vec<LspDiagnostic>,
    /// Summary counts
    pub summary: DiagnosticsSummary,
}

/// Summary of diagnostic counts
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsSummary {
    /// Number of errors
    pub errors: usize,
    /// Number of warnings
    pub warnings: usize,
    /// Number of info messages
    pub info: usize,
    /// Number of hints
    pub hints: usize,
    /// Total number of diagnostics
    pub total: usize,
}

/// A location in source code (file path with range)
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LocationResult {
    /// The file path
    pub file_path: String,
    /// Start line (1-indexed)
    pub start_line: u32,
    /// Start column (1-indexed)
    pub start_column: u32,
    /// End line (1-indexed)
    pub end_line: u32,
    /// End column (1-indexed)
    pub end_column: u32,
}

impl LocationResult {
    /// Create from an LSP Location
    fn from_location(loc: &Location) -> Self {
        let file_path = uri_to_path(&loc.uri);
        Self {
            file_path,
            // Convert from 0-indexed to 1-indexed
            start_line: loc.range.start.line + 1,
            start_column: loc.range.start.character + 1,
            end_line: loc.range.end.line + 1,
            end_column: loc.range.end.character + 1,
        }
    }
}

/// Output from the go_to_definition operation
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GoToDefinitionOutput {
    /// List of definition locations (usually 1, but can be multiple for overloaded symbols)
    pub locations: Vec<LocationResult>,
}

/// Output from the find_references operation
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FindReferencesOutput {
    /// List of reference locations
    pub references: Vec<LocationResult>,
    /// Total count of references found
    pub total_count: usize,
}

/// Output from the hover operation
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HoverOutput {
    /// The hover contents (type info, documentation, etc.)
    pub contents: String,
    /// The range of the symbol being hovered (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<LocationResult>,
}

/// Output from the LSP tool
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum LspOutput {
    /// Diagnostics output
    Diagnostics(GetDiagnosticsOutput),
    /// Go to definition output
    GoToDefinition(GoToDefinitionOutput),
    /// Find references output
    FindReferences(FindReferencesOutput),
    /// Hover output
    Hover(HoverOutput),
}

/// Convert an LSP URI to a file path string
fn uri_to_path(uri: &Uri) -> String {
    let uri_str = uri.as_str();
    // Strip file:// prefix and decode
    if let Some(path) = uri_str.strip_prefix("file://") {
        // Handle Windows paths (file:///C:/...)
        if path.starts_with('/') && path.len() > 2 && path.chars().nth(2) == Some(':') {
            path[1..].to_string()
        } else {
            path.to_string()
        }
    } else {
        uri_str.to_string()
    }
}

/// Execute an LSP operation using the provided tools
///
/// # Arguments
/// * `operation` - The LSP operation to perform
/// * `tools` - The CodingTools implementation (used for LSP requests)
///
/// # Returns
/// The result of the operation
pub async fn execute_lsp_operation<T: CodingTools>(
    operation: LspOperation,
    tools: &T,
) -> Result<LspOutput, String> {
    match operation {
        LspOperation::GetDiagnostics { file_path } => {
            let diagnostics_cache = tools.get_lsp_diagnostics().await?;
            get_diagnostics(file_path, &diagnostics_cache)
        }
        LspOperation::GoToDefinition {
            file_path,
            symbol,
            line,
        } => {
            let line = parse_line(&line)?;
            let response = tools.goto_definition(&file_path, &symbol, line).await?;
            let locations = definition_response_to_locations(response);
            Ok(LspOutput::GoToDefinition(GoToDefinitionOutput { locations }))
        }
        LspOperation::FindReferences {
            file_path,
            symbol,
            line,
            include_declaration,
        } => {
            let line = parse_line(&line)?;
            let lsp_locations = tools
                .find_references(&file_path, &symbol, line, include_declaration)
                .await?;
            let references: Vec<LocationResult> = lsp_locations
                .iter()
                .map(LocationResult::from_location)
                .collect();
            let total_count = references.len();
            Ok(LspOutput::FindReferences(FindReferencesOutput {
                references,
                total_count,
            }))
        }
        LspOperation::Hover {
            file_path,
            symbol,
            line,
        } => {
            let line = parse_line(&line)?;
            let hover = tools.hover(&file_path, &symbol, line).await?;
            let output = match hover {
                Some(h) => {
                    let contents = hover_contents_to_string(&h);
                    let range = h.range.map(|r| LocationResult {
                        file_path: file_path.clone(),
                        start_line: r.start.line + 1,
                        start_column: r.start.character + 1,
                        end_line: r.end.line + 1,
                        end_column: r.end.character + 1,
                    });
                    HoverOutput { contents, range }
                }
                None => HoverOutput {
                    contents: String::new(),
                    range: None,
                },
            };
            Ok(LspOutput::Hover(output))
        }
    }
}

/// Convert GotoDefinitionResponse to a list of LocationResult
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

/// Convert Hover contents to a string
fn hover_contents_to_string(hover: &Hover) -> String {
    match &hover.contents {
        lsp_types::HoverContents::Scalar(marked_string) => marked_string_to_string(marked_string),
        lsp_types::HoverContents::Array(marked_strings) => marked_strings
            .iter()
            .map(marked_string_to_string)
            .collect::<Vec<_>>()
            .join("\n\n"),
        lsp_types::HoverContents::Markup(markup) => markup.value.clone(),
    }
}

/// Convert a MarkedString to a plain string
fn marked_string_to_string(ms: &lsp_types::MarkedString) -> String {
    match ms {
        lsp_types::MarkedString::String(s) => s.clone(),
        lsp_types::MarkedString::LanguageString(ls) => {
            format!("```{}\n{}\n```", ls.language, ls.value)
        }
    }
}

fn get_diagnostics(
    file_path: Option<String>,
    diagnostics_cache: &HashMap<Uri, Vec<Diagnostic>>,
) -> Result<LspOutput, String> {
    let mut all_diagnostics: Vec<LspDiagnostic> = Vec::new();

    // If a specific file path is requested, filter to that file
    if let Some(path) = file_path {
        // Try to find the URI that matches this path
        for (uri, diagnostics) in diagnostics_cache {
            let uri_path = uri.as_str();
            // Check if the URI ends with the requested path or matches exactly
            if uri_path.ends_with(&path) || uri_path.contains(&path) {
                let params = lsp_types::PublishDiagnosticsParams {
                    uri: uri.clone(),
                    diagnostics: diagnostics.clone(),
                    version: None,
                };
                all_diagnostics.extend(
                    format_diagnostics(&params)
                        .into_iter()
                        .map(LspDiagnostic::from),
                );
            }
        }
    } else {
        // Return all diagnostics
        for (uri, diagnostics) in diagnostics_cache {
            let params = lsp_types::PublishDiagnosticsParams {
                uri: uri.clone(),
                diagnostics: diagnostics.clone(),
                version: None,
            };
            all_diagnostics.extend(
                format_diagnostics(&params)
                    .into_iter()
                    .map(LspDiagnostic::from),
            );
        }
    }

    // Sort diagnostics by file, then line, then column for consistent output
    all_diagnostics.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.column.cmp(&b.column))
    });

    // Calculate summary
    let mut errors = 0;
    let mut warnings = 0;
    let mut info = 0;
    let mut hints = 0;

    for d in &all_diagnostics {
        match d.severity.as_str() {
            "error" => errors += 1,
            "warning" => warnings += 1,
            "info" => info += 1,
            "hint" => hints += 1,
            _ => {}
        }
    }

    let total = all_diagnostics.len();

    Ok(LspOutput::Diagnostics(GetDiagnosticsOutput {
        diagnostics: all_diagnostics,
        summary: DiagnosticsSummary {
            errors,
            warnings,
            info,
            hints,
            total,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{DiagnosticSeverity, Position, Range};

    fn make_uri(path: &str) -> Uri {
        format!("file://{}", path).parse().unwrap()
    }

    fn make_diagnostic(severity: DiagnosticSeverity, message: &str, line: u32) -> Diagnostic {
        Diagnostic {
            range: Range {
                start: Position { line, character: 0 },
                end: Position {
                    line,
                    character: 10,
                },
            },
            severity: Some(severity),
            code: Some(lsp_types::NumberOrString::String("E0308".to_string())),
            code_description: None,
            source: Some("rustc".to_string()),
            message: message.to_string(),
            related_information: None,
            tags: None,
            data: None,
        }
    }

    #[test]
    fn test_get_all_diagnostics() {
        let mut cache: HashMap<Uri, Vec<Diagnostic>> = HashMap::new();

        cache.insert(
            make_uri("/project/src/main.rs"),
            vec![
                make_diagnostic(DiagnosticSeverity::ERROR, "type mismatch", 10),
                make_diagnostic(DiagnosticSeverity::WARNING, "unused variable", 20),
            ],
        );
        cache.insert(
            make_uri("/project/src/lib.rs"),
            vec![make_diagnostic(
                DiagnosticSeverity::ERROR,
                "missing field",
                5,
            )],
        );

        let result = get_diagnostics(None, &cache).unwrap();

        match result {
            LspOutput::Diagnostics(output) => {
                assert_eq!(output.diagnostics.len(), 3);
                assert_eq!(output.summary.errors, 2);
                assert_eq!(output.summary.warnings, 1);
                assert_eq!(output.summary.total, 3);
            }
            _ => panic!("Expected Diagnostics output"),
        }
    }

    #[test]
    fn test_get_diagnostics_for_file() {
        let mut cache: HashMap<Uri, Vec<Diagnostic>> = HashMap::new();

        cache.insert(
            make_uri("/project/src/main.rs"),
            vec![make_diagnostic(
                DiagnosticSeverity::ERROR,
                "type mismatch",
                10,
            )],
        );
        cache.insert(
            make_uri("/project/src/lib.rs"),
            vec![make_diagnostic(
                DiagnosticSeverity::ERROR,
                "missing field",
                5,
            )],
        );

        let result = get_diagnostics(Some("main.rs".to_string()), &cache).unwrap();

        match result {
            LspOutput::Diagnostics(output) => {
                assert_eq!(output.diagnostics.len(), 1);
                assert!(output.diagnostics[0].file.contains("main.rs"));
                assert_eq!(output.diagnostics[0].message, "type mismatch");
            }
            _ => panic!("Expected Diagnostics output"),
        }
    }

    #[test]
    fn test_empty_diagnostics() {
        let cache: HashMap<Uri, Vec<Diagnostic>> = HashMap::new();

        let result = get_diagnostics(None, &cache).unwrap();

        match result {
            LspOutput::Diagnostics(output) => {
                assert_eq!(output.diagnostics.len(), 0);
                assert_eq!(output.summary.total, 0);
            }
            _ => panic!("Expected Diagnostics output"),
        }
    }

    #[test]
    fn test_diagnostics_sorted() {
        let mut cache: HashMap<Uri, Vec<Diagnostic>> = HashMap::new();

        cache.insert(
            make_uri("/project/src/b.rs"),
            vec![make_diagnostic(DiagnosticSeverity::ERROR, "error in b", 5)],
        );
        cache.insert(
            make_uri("/project/src/a.rs"),
            vec![make_diagnostic(DiagnosticSeverity::ERROR, "error in a", 10)],
        );

        let result = get_diagnostics(None, &cache).unwrap();

        match result {
            LspOutput::Diagnostics(output) => {
                // Should be sorted by file path
                assert!(output.diagnostics[0].file.contains("a.rs"));
                assert!(output.diagnostics[1].file.contains("b.rs"));
            }
            _ => panic!("Expected Diagnostics output"),
        }
    }
}
