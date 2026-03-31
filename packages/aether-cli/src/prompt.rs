use aether_core::core::Prompt;
use mcp_utils::client::ServerInstructions;
use std::path::Path;

pub async fn build_system_prompt(
    roots_path: &Path,
    instructions: Vec<ServerInstructions>,
    prompt_patterns: Vec<String>,
) -> Result<String, String> {
    let parts =
        vec![Prompt::from_globs(prompt_patterns, roots_path.to_path_buf()), Prompt::mcp_instructions(instructions)];

    Prompt::build_all(&parts).await.map_err(|e| format!("Failed to build system prompt: {e}"))
}
