//! LSP hover tool for getting type information and documentation

use lsp_types::Hover;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::coding::lsp::common::{LocationResult, parse_line};
use crate::coding::tools_trait::CodingTools;

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
