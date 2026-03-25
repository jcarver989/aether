use std::collections::BTreeMap;
use std::path::Path;

use super::PromptArgs;
use crate::error::CliError;
use crate::runtime::RuntimeBuilder;
use aether_core::agent_spec::AgentSpec;
use aether_core::core::Prompt;
use aether_project::{AgentCatalog, load_agent_catalog};
use llm::ToolDefinition;
use serde_json::Value;

pub async fn run_prompt(args: PromptArgs) -> Result<(), CliError> {
    let cwd = args.cwd.canonicalize().map_err(CliError::IoError)?;
    let catalog = load_agent_catalog(&cwd).map_err(|e| CliError::AgentError(e.to_string()))?;
    let spec = resolve_agent_spec(&catalog, args.agent.as_deref(), &cwd)?;

    let info = RuntimeBuilder::from_spec(cwd, spec)
        .mcp_config_opt(args.mcp_config)
        .build_prompt_info()
        .await?;

    let system_prompt = build_prompt(&info.spec.prompts, args.system_prompt.as_deref()).await?;
    let tools_output = build_tools(&info.tool_definitions);

    println!("{system_prompt}");

    if !tools_output.is_empty() {
        println!();
        println!("--- Tools ({} tools) ---", info.tool_definitions.len());
        println!();
        println!("{tools_output}");
    }

    println!();
    println!(
        "{}",
        format_stats(
            system_prompt.len(),
            tools_output.len(),
            info.tool_definitions.len()
        )
    );

    Ok(())
}

pub async fn build_prompt(prompts: &[Prompt], custom: Option<&str>) -> Result<String, CliError> {
    let mut prompts = prompts.to_vec();
    if let Some(custom) = custom {
        prompts.push(Prompt::text(custom));
    }
    Prompt::build_all(&prompts)
        .await
        .map_err(|e| CliError::AgentError(e.to_string()))
}

pub fn build_tools(tools: &[ToolDefinition]) -> String {
    if tools.is_empty() {
        return String::new();
    }

    let mut grouped: BTreeMap<&str, Vec<Value>> = BTreeMap::new();
    for tool in tools {
        let server = tool.server.as_deref().unwrap_or("(built-in)");
        let input_schema = serde_json::from_str::<Value>(&tool.parameters).unwrap_or(Value::Null);
        let entry = serde_json::json!({
            "name": tool.name,
            "description": tool.description,
            "input_schema": input_schema,
        });
        grouped.entry(server).or_default().push(entry);
    }

    let mut sections = Vec::new();
    for (server, entries) in &grouped {
        let json = serde_json::to_string_pretty(entries).unwrap_or_default();
        sections.push(format!("Server: {server}\n{json}"));
    }

    sections.join("\n\n")
}

pub fn format_stats(prompt_chars: usize, tool_schema_chars: usize, tool_count: usize) -> String {
    let est_tokens = (prompt_chars + tool_schema_chars) / 4;
    format!(
        "---\n\
         Prompt chars:     {prompt_chars:>8}\n\
         Tool schema chars:{tool_schema_chars:>8}\n\
         Est. tokens:     ~{est_tokens:>8}\n\
         MCP tools:        {tool_count:>8}"
    )
}

fn resolve_agent_spec(
    catalog: &AgentCatalog,
    agent_name: Option<&str>,
    cwd: &Path,
) -> Result<AgentSpec, CliError> {
    match agent_name {
        Some(name) => catalog
            .resolve(name, cwd)
            .map_err(|e| CliError::AgentError(e.to_string())),

        None => match catalog.user_invocable().next() {
            Some(first) => catalog
                .resolve(&first.name, cwd)
                .map_err(|e| CliError::AgentError(e.to_string())),

            None => {
                let model = "anthropic:claude-sonnet-4-5"
                    .parse()
                    .map_err(|e: String| CliError::ModelError(e))?;

                Ok(catalog.resolve_default(&model, None, cwd))
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str, desc: &str, params: &str, server: Option<&str>) -> ToolDefinition {
        ToolDefinition {
            name: name.to_string(),
            description: desc.to_string(),
            parameters: params.to_string(),
            server: server.map(String::from),
        }
    }

    #[test]
    fn format_stats_computes_token_estimate() {
        let output = format_stats(12000, 8500, 14);
        assert_eq!(
            output,
            "---\n\
             Prompt chars:        12000\n\
             Tool schema chars:    8500\n\
             Est. tokens:     ~    5125\n\
             MCP tools:              14"
        );
    }

    #[test]
    fn format_stats_handles_zero() {
        let output = format_stats(0, 0, 0);
        assert_eq!(
            output,
            "---\n\
             Prompt chars:            0\n\
             Tool schema chars:       0\n\
             Est. tokens:     ~       0\n\
             MCP tools:               0"
        );
    }

    #[test]
    fn format_stats_handles_small_values() {
        let output = format_stats(3, 0, 1);
        assert_eq!(
            output,
            "---\n\
             Prompt chars:            3\n\
             Tool schema chars:       0\n\
             Est. tokens:     ~       0\n\
             MCP tools:               1"
        );
    }

    #[test]
    fn build_tools_groups_by_server() {
        let tools = vec![
            tool(
                "fs_read",
                "Read a file",
                r#"{"type":"object"}"#,
                Some("filesystem"),
            ),
            tool("git_log", "Show log", r#"{"type":"object"}"#, Some("git")),
            tool(
                "fs_write",
                "Write a file",
                r#"{"type":"object"}"#,
                Some("filesystem"),
            ),
        ];
        let output = build_tools(&tools);
        // BTreeMap sorts: filesystem < git
        let fs_pos = output.find("Server: filesystem").unwrap();
        let git_pos = output.find("Server: git").unwrap();
        assert!(fs_pos < git_pos);
        // filesystem group has both tools
        assert!(output.contains("fs_read"));
        assert!(output.contains("fs_write"));
    }

    #[test]
    fn build_tools_handles_no_server() {
        let tools = vec![tool(
            "builtin_tool",
            "A built-in",
            r#"{"type":"object"}"#,
            None,
        )];
        let output = build_tools(&tools);
        assert!(output.contains("Server: (built-in)"));
        assert!(output.contains("builtin_tool"));
    }

    #[test]
    fn build_tools_produces_api_format() {
        let tools = vec![tool(
            "my_tool",
            "Does stuff",
            r#"{"type":"object","properties":{}}"#,
            Some("test"),
        )];
        let output = build_tools(&tools);
        // Strip "Server: test\n" prefix to get the JSON
        let json_start = output.find('[').unwrap();
        let parsed: Vec<Value> = serde_json::from_str(&output[json_start..]).unwrap();
        assert_eq!(parsed.len(), 1);
        let entry = &parsed[0];
        assert_eq!(entry["name"], "my_tool");
        assert_eq!(entry["description"], "Does stuff");
        assert!(entry["input_schema"].is_object());
    }

    #[test]
    fn build_tools_empty() {
        assert_eq!(build_tools(&[]), "");
    }

    #[test]
    fn build_tools_malformed_params() {
        let tools = vec![tool(
            "bad_tool",
            "Broken params",
            "not valid json",
            Some("srv"),
        )];
        let output = build_tools(&tools);
        assert!(output.contains("bad_tool"));
        assert!(output.contains("null"));
    }

    fn write_file(dir: &std::path::Path, path: &str, content: &str) {
        let full = dir.join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(full, content).unwrap();
    }

    fn setup_catalog(settings_json: &str) -> (tempfile::TempDir, AgentCatalog) {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), "PROMPT.md", "Be helpful");
        write_file(dir.path(), ".aether/settings.json", settings_json);
        let catalog = load_agent_catalog(dir.path()).unwrap();
        (dir, catalog)
    }

    #[test]
    fn resolve_agent_spec_with_explicit_name() {
        let (dir, catalog) = setup_catalog(
            r#"{"agents": [
                {"name": "first", "description": "First", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["PROMPT.md"]},
                {"name": "second", "description": "Second", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["PROMPT.md"]}
            ]}"#,
        );
        let spec = resolve_agent_spec(&catalog, Some("second"), dir.path()).unwrap();
        assert_eq!(spec.name, "second");
    }

    #[test]
    fn resolve_agent_spec_auto_selects_first_user_invocable() {
        let (dir, catalog) = setup_catalog(
            r#"{"agents": [
                {"name": "internal", "description": "Internal", "model": "anthropic:claude-sonnet-4-5", "agentInvocable": true, "prompts": ["PROMPT.md"]},
                {"name": "visible", "description": "Visible", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["PROMPT.md"]}
            ]}"#,
        );
        let spec = resolve_agent_spec(&catalog, None, dir.path()).unwrap();
        assert_eq!(spec.name, "visible");
    }

    #[test]
    fn resolve_agent_spec_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = AgentCatalog::empty(dir.path().to_path_buf());
        let spec = resolve_agent_spec(&catalog, None, dir.path()).unwrap();
        assert_eq!(spec.name, "__default__");
    }

    #[test]
    fn resolve_agent_spec_unknown_name_errors() {
        let (dir, catalog) = setup_catalog(
            r#"{"agents": [
                {"name": "alpha", "description": "Alpha", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["PROMPT.md"]}
            ]}"#,
        );
        let result = resolve_agent_spec(&catalog, Some("nonexistent"), dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn resolve_agent_spec_preserves_agent_mcp_config() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), "PROMPT.md", "Be helpful");
        write_file(dir.path(), "agent-mcp.json", "{}");
        write_file(
            dir.path(),
            ".aether/settings.json",
            r#"{"agents": [
                {"name": "with-mcp", "description": "Has MCP", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["PROMPT.md"], "mcpServers": "agent-mcp.json"}
            ]}"#,
        );
        let catalog = load_agent_catalog(dir.path()).unwrap();
        let spec = resolve_agent_spec(&catalog, None, dir.path()).unwrap();
        assert_eq!(
            spec.mcp_config_path,
            Some(dir.path().join("agent-mcp.json"))
        );
    }
}
