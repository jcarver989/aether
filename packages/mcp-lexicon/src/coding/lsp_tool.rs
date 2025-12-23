//! LSP tools for querying language server information
//!
//! This module provides MCP tools that expose LSP functionality to LLMs,
//! with separate tools for each operation type for clear, self-documenting schemas.

use lsp_types::{Diagnostic, GotoDefinitionResponse, Hover, Location, SymbolKind, Uri};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::lsp::{FormattedDiagnostic, format_diagnostics};
use super::tools_trait::CodingTools;

// ============================================================================
// Input types - one per tool
// ============================================================================

/// Input for the lsp_diagnostics tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct LspDiagnosticsInput {
    /// Optional: filter to specific file path. If not provided, returns all diagnostics.
    #[serde(default)]
    pub file_path: Option<String>,
}

/// Input for the lsp_goto_definition tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct LspGotoDefinitionInput {
    /// The file path containing the symbol
    pub file_path: String,
    /// The symbol name to look up (e.g., "HashMap", "spawn", "LspClient")
    pub symbol: String,
    /// Line number where the symbol appears (1-indexed, as shown by the read_file tool)
    pub line: String,
}

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

/// Input for the lsp_hover tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct LspHoverInput {
    /// The file path containing the symbol
    pub file_path: String,
    /// The symbol name to look up (e.g., "HashMap", "spawn", "LspClient")
    pub symbol: String,
    /// Line number where the symbol appears (1-indexed, as shown by the read_file tool)
    pub line: String,
}

/// Input for the lsp_workspace_symbol tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct LspWorkspaceSymbolInput {
    /// The search query (fuzzy matching is used by most language servers)
    pub query: String,
}

// ============================================================================
// Output types - shared where appropriate
// ============================================================================

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

/// Output from the lsp_diagnostics tool
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LspDiagnosticsOutput {
    /// List of diagnostics
    pub diagnostics: Vec<LspDiagnostic>,
    /// Summary counts
    pub summary: DiagnosticsSummary,
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

/// Output from the lsp_goto_definition tool
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LspGotoDefinitionOutput {
    /// List of definition locations (usually 1, but can be multiple for overloaded symbols)
    pub locations: Vec<LocationResult>,
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

/// Output from the lsp_hover tool
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LspHoverOutput {
    /// The hover contents (type info, documentation, etc.)
    pub contents: String,
    /// The range of the symbol being hovered (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<LocationResult>,
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

// ============================================================================
// Execute functions - one per tool
// ============================================================================

/// Execute the lsp_diagnostics operation
pub async fn execute_lsp_diagnostics<T: CodingTools>(
    input: LspDiagnosticsInput,
    tools: &T,
) -> Result<LspDiagnosticsOutput, String> {
    let diagnostics_cache = tools.get_lsp_diagnostics().await?;
    get_diagnostics(input.file_path, &diagnostics_cache)
}

/// Execute the lsp_goto_definition operation
pub async fn execute_lsp_goto_definition<T: CodingTools>(
    input: LspGotoDefinitionInput,
    tools: &T,
) -> Result<LspGotoDefinitionOutput, String> {
    let line = parse_line(&input.line)?;
    let response = tools
        .goto_definition(&input.file_path, &input.symbol, line)
        .await?;
    let locations = definition_response_to_locations(response);
    Ok(LspGotoDefinitionOutput { locations })
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

/// Execute the lsp_hover operation
pub async fn execute_lsp_hover<T: CodingTools>(
    input: LspHoverInput,
    tools: &T,
) -> Result<LspHoverOutput, String> {
    let line = parse_line(&input.line)?;
    let hover = tools.hover(&input.file_path, &input.symbol, line).await?;
    let output = match hover {
        Some(h) => {
            let contents = hover_contents_to_string(&h);
            let range = h.range.map(|r| LocationResult {
                file_path: input.file_path.clone(),
                start_line: r.start.line + 1,
                start_column: r.start.character + 1,
                end_line: r.end.line + 1,
                end_column: r.end.character + 1,
            });
            LspHoverOutput { contents, range }
        }
        None => LspHoverOutput {
            contents: String::new(),
            range: None,
        },
    };
    Ok(output)
}

/// Execute the lsp_workspace_symbol operation
pub async fn execute_lsp_workspace_symbol<T: CodingTools>(
    input: LspWorkspaceSymbolInput,
    tools: &T,
) -> Result<LspWorkspaceSymbolOutput, String> {
    let lsp_symbols = tools.workspace_symbol(&input.query).await?;
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

// ============================================================================
// Helper functions
// ============================================================================

/// Parse a line number string to u32
fn parse_line(s: &str) -> Result<u32, String> {
    s.trim()
        .parse()
        .map_err(|_| format!("Invalid line number: {}", s))
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

/// Convert SymbolKind to a human-readable string
fn symbol_kind_to_string(kind: SymbolKind) -> String {
    match kind {
        SymbolKind::FILE => "file",
        SymbolKind::MODULE => "module",
        SymbolKind::NAMESPACE => "namespace",
        SymbolKind::PACKAGE => "package",
        SymbolKind::CLASS => "class",
        SymbolKind::METHOD => "method",
        SymbolKind::PROPERTY => "property",
        SymbolKind::FIELD => "field",
        SymbolKind::CONSTRUCTOR => "constructor",
        SymbolKind::ENUM => "enum",
        SymbolKind::INTERFACE => "interface",
        SymbolKind::FUNCTION => "function",
        SymbolKind::VARIABLE => "variable",
        SymbolKind::CONSTANT => "constant",
        SymbolKind::STRING => "string",
        SymbolKind::NUMBER => "number",
        SymbolKind::BOOLEAN => "boolean",
        SymbolKind::ARRAY => "array",
        SymbolKind::OBJECT => "object",
        SymbolKind::KEY => "key",
        SymbolKind::NULL => "null",
        SymbolKind::ENUM_MEMBER => "enum_member",
        SymbolKind::STRUCT => "struct",
        SymbolKind::EVENT => "event",
        SymbolKind::OPERATOR => "operator",
        SymbolKind::TYPE_PARAMETER => "type_parameter",
        _ => "unknown",
    }
    .to_string()
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
    diagnostics_cache: &HashMap<String, Vec<Diagnostic>>,
) -> Result<LspDiagnosticsOutput, String> {
    let mut all_diagnostics: Vec<LspDiagnostic> = Vec::new();

    // If a specific file path is requested, filter to that file
    if let Some(path) = file_path {
        // Try to find the URI that matches this path
        for (uri_str, diagnostics) in diagnostics_cache {
            // Check if the URI ends with the requested path or matches exactly
            if (uri_str.ends_with(&path) || uri_str.contains(&path))
                && let Ok(uri) = uri_str.parse()
            {
                let params = lsp_types::PublishDiagnosticsParams {
                    uri,
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
        for (uri_str, diagnostics) in diagnostics_cache {
            if let Ok(uri) = uri_str.parse() {
                let params = lsp_types::PublishDiagnosticsParams {
                    uri,
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

    Ok(LspDiagnosticsOutput {
        diagnostics: all_diagnostics,
        summary: DiagnosticsSummary {
            errors,
            warnings,
            info,
            hints,
            total,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{DiagnosticSeverity, Position, Range};

    fn make_uri_string(path: &str) -> String {
        format!("file://{}", path)
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
        let mut cache: HashMap<String, Vec<Diagnostic>> = HashMap::new();

        cache.insert(
            make_uri_string("/project/src/main.rs"),
            vec![
                make_diagnostic(DiagnosticSeverity::ERROR, "type mismatch", 10),
                make_diagnostic(DiagnosticSeverity::WARNING, "unused variable", 20),
            ],
        );
        cache.insert(
            make_uri_string("/project/src/lib.rs"),
            vec![make_diagnostic(
                DiagnosticSeverity::ERROR,
                "missing field",
                5,
            )],
        );

        let result = get_diagnostics(None, &cache).unwrap();

        assert_eq!(result.diagnostics.len(), 3);
        assert_eq!(result.summary.errors, 2);
        assert_eq!(result.summary.warnings, 1);
        assert_eq!(result.summary.total, 3);
    }

    #[test]
    fn test_get_diagnostics_for_file() {
        let mut cache: HashMap<String, Vec<Diagnostic>> = HashMap::new();

        cache.insert(
            make_uri_string("/project/src/main.rs"),
            vec![make_diagnostic(
                DiagnosticSeverity::ERROR,
                "type mismatch",
                10,
            )],
        );
        cache.insert(
            make_uri_string("/project/src/lib.rs"),
            vec![make_diagnostic(
                DiagnosticSeverity::ERROR,
                "missing field",
                5,
            )],
        );

        let result = get_diagnostics(Some("main.rs".to_string()), &cache).unwrap();

        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].file.contains("main.rs"));
        assert_eq!(result.diagnostics[0].message, "type mismatch");
    }

    #[test]
    fn test_empty_diagnostics() {
        let cache: HashMap<String, Vec<Diagnostic>> = HashMap::new();

        let result = get_diagnostics(None, &cache).unwrap();

        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.summary.total, 0);
    }

    #[test]
    fn test_diagnostics_sorted() {
        let mut cache: HashMap<String, Vec<Diagnostic>> = HashMap::new();

        cache.insert(
            make_uri_string("/project/src/b.rs"),
            vec![make_diagnostic(DiagnosticSeverity::ERROR, "error in b", 5)],
        );
        cache.insert(
            make_uri_string("/project/src/a.rs"),
            vec![make_diagnostic(DiagnosticSeverity::ERROR, "error in a", 10)],
        );

        let result = get_diagnostics(None, &cache).unwrap();

        // Should be sorted by file path
        assert!(result.diagnostics[0].file.contains("a.rs"));
        assert!(result.diagnostics[1].file.contains("b.rs"));
    }
}
