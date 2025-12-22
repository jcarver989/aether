//! LSP tool for querying language server information
//!
//! This module provides an MCP tool that exposes LSP functionality to LLMs,
//! starting with diagnostics queries and extensible for future operations.

use lsp_types::{Diagnostic, Uri};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::lsp::{FormattedDiagnostic, format_diagnostics};

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
    // Future operations:
    // Hover { file_path: String, line: u32, column: u32 },
    // GoToDefinition { file_path: String, line: u32, column: u32 },
    // FindReferences { file_path: String, line: u32, column: u32 },
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

/// Output from the LSP tool
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum LspOutput {
    /// Diagnostics output
    Diagnostics(GetDiagnosticsOutput),
}

/// Execute an LSP operation using the provided diagnostics cache
///
/// # Arguments
/// * `operation` - The LSP operation to perform
/// * `diagnostics_cache` - Current diagnostics from the LSP client
///
/// # Returns
/// The result of the operation
pub fn execute_lsp_operation(
    operation: LspOperation,
    diagnostics_cache: &HashMap<Uri, Vec<Diagnostic>>,
) -> Result<LspOutput, String> {
    match operation {
        LspOperation::GetDiagnostics { file_path } => get_diagnostics(file_path, diagnostics_cache),
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

        let result =
            execute_lsp_operation(LspOperation::GetDiagnostics { file_path: None }, &cache)
                .unwrap();

        match result {
            LspOutput::Diagnostics(output) => {
                assert_eq!(output.diagnostics.len(), 3);
                assert_eq!(output.summary.errors, 2);
                assert_eq!(output.summary.warnings, 1);
                assert_eq!(output.summary.total, 3);
            }
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

        let result = execute_lsp_operation(
            LspOperation::GetDiagnostics {
                file_path: Some("main.rs".to_string()),
            },
            &cache,
        )
        .unwrap();

        match result {
            LspOutput::Diagnostics(output) => {
                assert_eq!(output.diagnostics.len(), 1);
                assert!(output.diagnostics[0].file.contains("main.rs"));
                assert_eq!(output.diagnostics[0].message, "type mismatch");
            }
        }
    }

    #[test]
    fn test_empty_diagnostics() {
        let cache: HashMap<Uri, Vec<Diagnostic>> = HashMap::new();

        let result =
            execute_lsp_operation(LspOperation::GetDiagnostics { file_path: None }, &cache)
                .unwrap();

        match result {
            LspOutput::Diagnostics(output) => {
                assert_eq!(output.diagnostics.len(), 0);
                assert_eq!(output.summary.total, 0);
            }
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

        let result =
            execute_lsp_operation(LspOperation::GetDiagnostics { file_path: None }, &cache)
                .unwrap();

        match result {
            LspOutput::Diagnostics(output) => {
                // Should be sorted by file path
                assert!(output.diagnostics[0].file.contains("a.rs"));
                assert!(output.diagnostics[1].file.contains("b.rs"));
            }
        }
    }
}
