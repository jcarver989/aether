use aether::core::Prompt;
use mcp_utils::client::ServerInstructions;
use std::path::Path;

pub async fn build_system_prompt(
    roots_path: &Path,
    instructions: Vec<ServerInstructions>,
    custom_prompt: Option<&str>,
) -> Result<String, String> {
    let mut parts = vec![
        Prompt::agents_md().with_cwd(roots_path.to_path_buf()),
        Prompt::system_env().with_cwd(roots_path.to_path_buf()),
        Prompt::mcp_instructions(instructions),
    ];

    if let Some(custom) = custom_prompt {
        parts.push(Prompt::text(custom));
    }

    Prompt::build_all(&parts)
        .await
        .map_err(|e| format!("Failed to build system prompt: {e}"))
}
