//! LSP diagnostics tool for querying compiler errors and warnings

use lsp_types::Diagnostic;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::coding::lsp::diagnostics::{FormattedDiagnostic, format_diagnostics};
use crate::coding::tools_trait::CodingTools;

/// Input for the lsp_diagnostics tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct LspDiagnosticsInput {
    /// Optional: filter to specific file path. If not provided, returns all diagnostics.
    #[serde(default)]
    pub file_path: Option<String>,
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

/// Execute the lsp_diagnostics operation
pub async fn execute_lsp_diagnostics<T: CodingTools>(
    input: LspDiagnosticsInput,
    tools: &T,
) -> Result<LspDiagnosticsOutput, String> {
    let diagnostics_cache = tools
        .get_lsp_diagnostics()
        .await
        .map_err(|e| e.to_string())?;
    get_diagnostics(input.file_path, &diagnostics_cache)
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
