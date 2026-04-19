use std::path::Path;

use tui::SelectOption;

pub enum NewAgentMode {
    ScaffoldProject,
    AddAgentToExistingProject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptFile {
    Agents,
    Claude,
    Gemini,
}

impl PromptFile {
    pub fn all() -> &'static [PromptFile] {
        &[PromptFile::Agents, PromptFile::Claude, PromptFile::Gemini]
    }

    pub fn filename(self) -> &'static str {
        match self {
            PromptFile::Agents => "AGENTS.md",
            PromptFile::Claude => "CLAUDE.md",
            PromptFile::Gemini => "GEMINI.md",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            PromptFile::Agents => "Project-level instructions shared across agents",
            PromptFile::Claude => "Claude Code prompt file",
            PromptFile::Gemini => "Gemini CLI prompt file",
        }
    }
}

pub fn detect_prompt_files(project_root: &Path) -> Vec<PromptFile> {
    PromptFile::all().iter().copied().filter(|d| project_root.join(d.filename()).is_file()).collect()
}

/// Returns the prompt files the wizard should offer on the Prompts step.
///
/// Scaffold mode always offers `AGENTS.md` even when it's absent from disk,
/// because scaffolding creates it. Add-agent mode only offers files that
/// actually exist — aether doesn't author `CLAUDE.md`/`GEMINI.md` and there's
/// no useful default `AGENTS.md` to append to.
pub fn available_prompt_files(mode: &NewAgentMode, project_root: &Path) -> Vec<PromptFile> {
    let mut detected = detect_prompt_files(project_root);
    if matches!(mode, NewAgentMode::ScaffoldProject) && !detected.contains(&PromptFile::Agents) {
        detected.insert(0, PromptFile::Agents);
    }
    detected
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewAgentOutcome {
    Applied,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewAgentStep {
    Identity,
    Model,
    Prompts,
    Tools,
}

impl NewAgentStep {
    pub fn all() -> &'static [NewAgentStep] {
        &[NewAgentStep::Identity, NewAgentStep::Model, NewAgentStep::Prompts, NewAgentStep::Tools]
    }

    pub fn title(self) -> &'static str {
        match self {
            NewAgentStep::Identity => "Identity",
            NewAgentStep::Model => "Model",
            NewAgentStep::Prompts => "Prompts",
            NewAgentStep::Tools => "Tools",
        }
    }

    pub fn heading(self) -> &'static str {
        match self {
            NewAgentStep::Identity => "Name your agent",
            NewAgentStep::Model => "Select one or more models",
            NewAgentStep::Prompts => "Select System Prompt Files",
            NewAgentStep::Tools => "Select Tools",
        }
    }

    pub fn next(self) -> Option<NewAgentStep> {
        match self {
            NewAgentStep::Identity => Some(NewAgentStep::Model),
            NewAgentStep::Model => Some(NewAgentStep::Prompts),
            NewAgentStep::Prompts => Some(NewAgentStep::Tools),
            NewAgentStep::Tools => None,
        }
    }

    pub fn prev(self) -> Option<NewAgentStep> {
        match self {
            NewAgentStep::Identity => None,
            NewAgentStep::Model => Some(NewAgentStep::Identity),
            NewAgentStep::Prompts => Some(NewAgentStep::Model),
            NewAgentStep::Tools => Some(NewAgentStep::Prompts),
        }
    }
}

pub fn should_run_onboarding(dir: &Path) -> bool {
    !dir.join(".aether/settings.json").exists()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpConfigFile {
    McpJson,
    DotMcpJson,
}

impl McpConfigFile {
    pub fn all() -> &'static [McpConfigFile] {
        &[McpConfigFile::McpJson, McpConfigFile::DotMcpJson]
    }

    pub fn filename(self) -> &'static str {
        match self {
            McpConfigFile::McpJson => "mcp.json",
            McpConfigFile::DotMcpJson => ".mcp.json",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            McpConfigFile::McpJson => "MCP server configuration",
            McpConfigFile::DotMcpJson => "MCP server configuration (dotfile)",
        }
    }
}

pub fn detect_mcp_configs(project_root: &Path) -> Vec<McpConfigFile> {
    McpConfigFile::all().iter().copied().filter(|c| project_root.join(c.filename()).is_file()).collect()
}

pub fn server_options() -> Vec<tui::SelectOption> {
    vec![
        tui::SelectOption {
            value: "coding".to_string(),
            title: "Coding".to_string(),
            description: Some("Filesystem, search, and bash tools".to_string()),
        },
        tui::SelectOption {
            value: "lsp".to_string(),
            title: "Lsp".to_string(),
            description: Some("Language Server Protocol integration".to_string()),
        },
        tui::SelectOption {
            value: "skills".to_string(),
            title: "Skills".to_string(),
            description: Some("Skills and slash-commands".to_string()),
        },
        tui::SelectOption {
            value: "subagents".to_string(),
            title: "Subagents".to_string(),
            description: Some("Spawn sub-agents in parallel".to_string()),
        },
        tui::SelectOption {
            value: "tasks".to_string(),
            title: "Tasks".to_string(),
            description: Some("Task management tools, backed by JSONL files".to_string()),
        },
        tui::SelectOption {
            value: "survey".to_string(),
            title: "Survey".to_string(),
            description: Some("Allow your agent to ask you structured questions".to_string()),
        },
        SelectOption {
            value: "plan".to_string(),
            title: "Plan".to_string(),
            description: Some("Plan-mode prompt and plan review via elicitation".to_string()),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn should_run_onboarding_when_settings_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(should_run_onboarding(dir.path()));
    }

    #[test]
    fn does_not_run_onboarding_when_settings_exists() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".aether")).unwrap();
        std::fs::write(dir.path().join(".aether/settings.json"), "{}").unwrap();
        assert!(!should_run_onboarding(dir.path()));
    }

    #[test]
    fn wizard_step_ordering() {
        assert_eq!(NewAgentStep::Identity.next(), Some(NewAgentStep::Model));
        assert_eq!(NewAgentStep::Model.next(), Some(NewAgentStep::Prompts));
        assert_eq!(NewAgentStep::Prompts.next(), Some(NewAgentStep::Tools));
        assert_eq!(NewAgentStep::Tools.next(), None);

        assert_eq!(NewAgentStep::Identity.prev(), None);
        assert_eq!(NewAgentStep::Model.prev(), Some(NewAgentStep::Identity));
        assert_eq!(NewAgentStep::Tools.prev(), Some(NewAgentStep::Prompts));
    }

    #[test]
    fn detect_prompt_files_returns_only_existing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "a").unwrap();
        std::fs::write(dir.path().join("GEMINI.md"), "g").unwrap();

        let detected = detect_prompt_files(dir.path());
        assert_eq!(detected, vec![PromptFile::Agents, PromptFile::Gemini]);
    }

    #[test]
    fn available_prompt_files_scaffold_always_includes_agents_md() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_files = available_prompt_files(&NewAgentMode::ScaffoldProject, dir.path());
        assert_eq!(prompt_files, vec![PromptFile::Agents]);
    }

    #[test]
    fn available_prompt_files_scaffold_preserves_order_when_all_present() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "a").unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "c").unwrap();
        std::fs::write(dir.path().join("GEMINI.md"), "g").unwrap();

        let prompt_files = available_prompt_files(&NewAgentMode::ScaffoldProject, dir.path());
        assert_eq!(prompt_files, vec![PromptFile::Agents, PromptFile::Claude, PromptFile::Gemini]);
    }

    #[test]
    fn available_prompt_files_scaffold_prepends_agents_md_when_only_others_exist() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "c").unwrap();

        let prompt_files = available_prompt_files(&NewAgentMode::ScaffoldProject, dir.path());
        assert_eq!(prompt_files, vec![PromptFile::Agents, PromptFile::Claude]);
    }

    #[test]
    fn detect_mcp_configs_returns_only_existing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("mcp.json"), r#"{"servers":{}}"#).unwrap();

        let detected = detect_mcp_configs(dir.path());
        assert_eq!(detected, vec![McpConfigFile::McpJson]);
    }

    #[test]
    fn detect_mcp_configs_finds_both() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("mcp.json"), r#"{"servers":{}}"#).unwrap();
        std::fs::write(dir.path().join(".mcp.json"), r#"{"servers":{}}"#).unwrap();

        let detected = detect_mcp_configs(dir.path());
        assert_eq!(detected, vec![McpConfigFile::McpJson, McpConfigFile::DotMcpJson]);
    }

    #[test]
    fn detect_mcp_configs_returns_empty_when_none() {
        let dir = tempfile::tempdir().unwrap();
        let detected = detect_mcp_configs(dir.path());
        assert!(detected.is_empty());
    }

    #[test]
    fn available_prompt_files_add_agent_is_detection_only() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_files = available_prompt_files(&NewAgentMode::AddAgentToExistingProject, dir.path());
        assert!(prompt_files.is_empty());
    }
}
