use aether_project::{AgentEntry, Settings};
use std::fs;

use crate::agent::ListArgs;
use crate::error::CliError;

pub fn run_list(args: ListArgs) -> Result<(), CliError> {
    let project_root = args.path.canonicalize().unwrap_or(args.path);
    let settings_path = project_root.join(".aether/settings.json");

    let content = match fs::read_to_string(&settings_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!("No agents found. Run `aether agent new` to create one.");
            return Ok(());
        }
        Err(e) => return Err(CliError::IoError(e)),
    };

    let settings: Settings =
        serde_json::from_str(&content).map_err(|e| CliError::AgentError(format!("Failed to parse settings: {e}")))?;

    if settings.agents.is_empty() {
        println!("No agents found. Run `aether agent new` to create one.");
        return Ok(());
    }

    let mut sorted: Vec<&AgentEntry> = settings.agents.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));

    for (i, agent) in sorted.iter().enumerate() {
        if i > 0 {
            println!();
        }
        print_agent(agent, &settings);
    }

    Ok(())
}

fn print_agent(agent: &AgentEntry, settings: &Settings) {
    println!("{}", agent.name);
    println!("  model:       {}", agent.model);

    let reasoning = agent.reasoning_effort.as_ref().map_or("none".to_string(), std::string::ToString::to_string);
    println!("  reasoning:   {reasoning}");

    println!("  description: {}", agent.description);

    let mut surfaces = Vec::new();
    if agent.user_invocable {
        surfaces.push("user");
    }
    if agent.agent_invocable {
        surfaces.push("agent");
    }
    println!("  invocable:   {}", surfaces.join(", "));

    let effective_prompts = if agent.prompts.is_empty() { &settings.prompts } else { &agent.prompts };
    if !effective_prompts.is_empty() {
        println!(
            "  prompts:     {}",
            effective_prompts.iter().map(std::string::String::as_str).collect::<Vec<_>>().join(", ")
        );
    }

    let effective_mcp = if agent.mcp_servers.is_empty() { &settings.mcp_servers } else { &agent.mcp_servers };
    if !effective_mcp.is_empty() {
        println!(
            "  mcp servers: {}",
            effective_mcp.iter().map(std::string::String::as_str).collect::<Vec<_>>().join(", ")
        );
    }

    if !agent.tools.allow.is_empty() {
        println!("  tools allow: {}", agent.tools.allow.join(", "));
    }
    if !agent.tools.deny.is_empty() {
        println!("  tools deny:  {}", agent.tools.deny.join(", "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::new_agent_wizard::{DraftAgentEntry, scaffold};

    #[test]
    fn list_empty_project() {
        let dir = tempfile::tempdir().unwrap();
        let args = super::super::ListArgs { path: dir.path().to_path_buf() };
        run_list(args).unwrap();
    }

    #[test]
    fn list_project_with_agents() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path(), &test_draft()).unwrap();
        let args = super::super::ListArgs { path: dir.path().to_path_buf() };
        run_list(args).unwrap();
    }

    fn test_draft() -> DraftAgentEntry {
        use crate::agent::new_agent_wizard::build_system_md;

        let mut draft = DraftAgentEntry {
            entry: AgentEntry {
                name: "Coder".to_string(),
                description: "A coding agent".to_string(),
                user_invocable: true,
                agent_invocable: true,
                model: "anthropic:claude-sonnet-4-5".to_string(),
                prompts: vec!["AGENTS.md".to_string()],
                mcp_servers: vec!["coding".to_string()],
                ..AgentEntry::default()
            },
            system_md_content: String::new(),
            system_md_edited: false,
            workspace_mcp_configs: vec![],
        };
        draft.system_md_content = build_system_md(&draft);
        draft
    }
}
