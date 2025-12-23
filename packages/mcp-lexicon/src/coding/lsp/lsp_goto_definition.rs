//! LSP goto definition tool for navigating to symbol definitions

use lsp_types::GotoDefinitionResponse;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{LocationResult, parse_line, uri_to_path};
use crate::coding::tools_trait::CodingTools;

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

/// Output from the lsp_goto_definition tool
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LspGotoDefinitionOutput {
    /// List of definition locations (usually 1, but can be multiple for overloaded symbols)
    pub locations: Vec<LocationResult>,
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
