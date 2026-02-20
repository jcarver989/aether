//! LSP diagnostics tool for querying compiler errors and warnings

use crate::lsp::diagnostics::{
    DiagnosticCounts, FormattedDiagnostic, count_by_severity, format_diagnostics,
};
use crate::tools_trait::CodingTools;
use lsp_types::Diagnostic;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Input for the `lsp_diagnostics` tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct LspDiagnosticsInput {
    /// Optional: filter to specific file path. If not provided, returns all diagnostics.
    #[serde(default)]
    pub file_path: Option<String>,
}

/// Output from the `lsp_diagnostics` tool
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LspDiagnosticsOutput {
    /// List of diagnostics
    pub diagnostics: Vec<FormattedDiagnostic>,
    /// Summary counts
    pub summary: DiagnosticCounts,
}

/// Execute the `lsp_diagnostics` operation
pub async fn execute_lsp_diagnostics<T: CodingTools>(
    input: LspDiagnosticsInput,
    tools: &T,
) -> Result<LspDiagnosticsOutput, String> {
    let diagnostics_cache = tools
        .get_lsp_diagnostics()
        .await
        .map_err(|e| e.to_string())?;
    Ok(get_diagnostics(input.file_path, &diagnostics_cache))
}

fn get_diagnostics(
    file_path: Option<String>,
    diagnostics_cache: &HashMap<String, Vec<Diagnostic>>,
) -> LspDiagnosticsOutput {
    let mut all_diagnostics: Vec<FormattedDiagnostic> = Vec::new();

    if let Some(path) = file_path {
        for (uri_str, diagnostics) in diagnostics_cache {
            if (uri_str.ends_with(&path) || uri_str.contains(&path))
                && let Ok(uri) = uri_str.parse()
            {
                let params = lsp_types::PublishDiagnosticsParams {
                    uri,
                    diagnostics: diagnostics.clone(),
                    version: None,
                };
                all_diagnostics.extend(format_diagnostics(&params));
            }
        }
    } else {
        for (uri_str, diagnostics) in diagnostics_cache {
            if let Ok(uri) = uri_str.parse() {
                let params = lsp_types::PublishDiagnosticsParams {
                    uri,
                    diagnostics: diagnostics.clone(),
                    version: None,
                };
                all_diagnostics.extend(format_diagnostics(&params));
            }
        }
    }

    all_diagnostics.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.column.cmp(&b.column))
    });

    let summary = count_by_severity(&all_diagnostics);

    LspDiagnosticsOutput {
        diagnostics: all_diagnostics,
        summary,
    }
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

        let result = get_diagnostics(None, &cache);

        assert_eq!(result.diagnostics.len(), 3);
        assert_eq!(result.summary.errors, 2);
        assert_eq!(result.summary.warnings, 1);
        assert_eq!(result.summary.infos, 0);
        assert_eq!(result.summary.hints, 0);
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

        let result = get_diagnostics(Some("main.rs".to_string()), &cache);

        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].file.contains("main.rs"));
        assert_eq!(result.diagnostics[0].message, "type mismatch");
        assert_eq!(result.summary.total, 1);
    }

    #[test]
    fn test_empty_diagnostics() {
        let cache: HashMap<String, Vec<Diagnostic>> = HashMap::new();

        let result = get_diagnostics(None, &cache);

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

        let result = get_diagnostics(None, &cache);

        // Should be sorted by file path
        assert!(result.diagnostics[0].file.contains("a.rs"));
        assert!(result.diagnostics[1].file.contains("b.rs"));
    }
}
