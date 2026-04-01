use crate::agent::NewArgs;
use crate::error::CliError;
use llm::LlmModel;
use llm::ReasoningEffort;
use llm::catalog::available_models;
use llm::providers::local::discovery::discover_local_models;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use tokio::sync::mpsc::UnboundedReceiver;
use tui::{
    Component, CrosstermEvent, Event, Form, FormField, FormFieldKind, FormMessage, KeyCode, KeyModifiers, Line,
    MouseCapture, MultiSelect, Renderer, SelectOption, TerminalSession, TextField, Theme, ViewContext,
    spawn_terminal_event_task, terminal_size,
};
use wisp::components::model_selector::{ModelEntry, ModelSelector};

const SYSTEM_MD_TEMPLATE: &str = include_str!("../../templates/SYSTEM.md");

pub async fn run_new(args: NewArgs) -> Result<(), CliError> {
    let project_root = args.path.canonicalize().unwrap_or(args.path);
    let settings_path = project_root.join(".aether/settings.json");
    let is_existing_project = settings_path.is_file();

    let (form_values, selection) = {
        let size = terminal_size().unwrap_or((80, 24));
        let mut renderer = Renderer::new(io::stdout(), Theme::default(), size);
        let _session = TerminalSession::new(false, MouseCapture::Disabled).map_err(CliError::IoError)?;
        let mut terminal_rx = spawn_terminal_event_task();

        let discovery_handle = tokio::spawn(discover_local_models());

        let mut form = build_form(is_existing_project);
        let form_result = run_form(&mut form, &mut renderer, &mut terminal_rx).await?;
        let Some(form_values) = form_result else {
            renderer.clear_screen().map_err(CliError::IoError)?;
            println!("Cancelled.");
            return Ok(());
        };

        renderer.clear_screen().map_err(CliError::IoError)?;

        let discovered = discovery_handle.await.unwrap_or_default();
        run_provider_screen(&discovered, &mut renderer, &mut terminal_rx).await?;

        renderer.clear_screen().map_err(CliError::IoError)?;

        let entries = build_model_entries(&discovered);
        if entries.is_empty() {
            renderer.clear_screen().map_err(CliError::IoError)?;
            println!("No providers detected. Set an API key environment variable and try again.");
            return Ok(());
        }

        let mut selector = ModelSelector::new(entries, "model".to_string(), None, None);

        let Some(selection) = run_model_selector(&mut selector, &mut renderer, &mut terminal_rx).await? else {
            renderer.clear_screen().map_err(CliError::IoError)?;
            println!("Cancelled.");
            return Ok(());
        };

        renderer.clear_screen().map_err(CliError::IoError)?;
        (form_values, selection)
    };

    let input = WizardInput::from_form_and_selection(&form_values, selection);

    if is_existing_project {
        add_agent(&settings_path, &input)?;
    } else {
        scaffold(&project_root, &input)?;
    }

    Ok(())
}

fn build_model_entries(discovered: &[LlmModel]) -> Vec<ModelEntry> {
    available_models()
        .into_iter()
        .chain(discovered.iter().cloned())
        .map(|m| ModelEntry {
            value: m.to_string(),
            name: format!("{} / {}", m.provider_display_name(), m.display_name()),
            reasoning_levels: m.reasoning_levels().to_vec(),
            supports_image: m.supports_image(),
            supports_audio: m.supports_audio(),
        })
        .collect()
}

struct ProviderStatus {
    display_name: String,
    detected: bool,
    config_hint: String,
}

fn detect_providers(discovered: &[LlmModel]) -> Vec<ProviderStatus> {
    let mut providers: BTreeMap<String, ProviderStatus> = BTreeMap::new();

    for model in LlmModel::all() {
        let display = model.provider_display_name().to_string();
        providers.entry(display.clone()).or_insert_with(|| {
            let env_var = model.required_env_var();
            let detected = env_var.is_none_or(|var| std::env::var(var).is_ok());
            let config_hint = env_var.unwrap_or("(no key required)").to_string();
            ProviderStatus { display_name: display, detected, config_hint }
        });
    }

    if !discovered.is_empty() {
        let mut ollama_count = 0;
        let mut llamacpp_count = 0;
        for m in discovered {
            match m {
                LlmModel::Ollama(_) => ollama_count += 1,
                LlmModel::LlamaCpp(_) => llamacpp_count += 1,
                _ => {}
            }
        }
        if ollama_count > 0 {
            providers.insert(
                "Ollama".to_string(),
                ProviderStatus {
                    display_name: "Ollama".to_string(),
                    detected: true,
                    config_hint: format!("localhost:11434 ({ollama_count} models)"),
                },
            );
        }
        if llamacpp_count > 0 {
            providers.insert(
                "LlamaCpp".to_string(),
                ProviderStatus {
                    display_name: "LlamaCpp".to_string(),
                    detected: true,
                    config_hint: format!("localhost:8080 ({llamacpp_count} models)"),
                },
            );
        }
    }

    providers.into_values().collect()
}

fn format_provider_lines(statuses: &[ProviderStatus], theme: &Theme) -> Vec<Line> {
    let (detected, missing): (Vec<_>, Vec<_>) = statuses.iter().partition(|p| p.detected);

    let name_width = detected.iter().chain(missing.iter()).map(|p| p.display_name.len()).max().unwrap_or(0).max(8);

    let mut lines = Vec::new();

    if detected.is_empty() {
        lines.push(Line::styled("  No providers detected.".to_string(), theme.text_primary()));
    } else {
        lines.push(Line::styled("  Available Providers".to_string(), theme.text_primary()));
        lines.push(Line::new(String::new()));
        lines.push(Line::styled(
            "  Models from these providers will be shown in the next step.".to_string(),
            theme.text_secondary(),
        ));
        lines.push(Line::new(String::new()));

        let header = format!("  {:<name_width$}  {}", "Provider", "Configuration");
        lines.push(Line::styled(header, theme.text_secondary()));
        let separator = format!("  {:-<name_width$}  {:-<15}", "", "");
        lines.push(Line::styled(separator, theme.muted()));

        for p in &detected {
            let row = format!("  {:<name_width$}  {}", p.display_name, p.config_hint);
            lines.push(Line::styled(row, theme.text_primary()));
        }
    }

    if !missing.is_empty() {
        let dim = theme.text_secondary();
        lines.push(Line::new(String::new()));
        lines.push(Line::styled("  Not Configured".to_string(), dim));
        lines.push(Line::new(String::new()));
        lines.push(Line::styled("  Set the environment variable to enable these providers.".to_string(), dim));
        lines.push(Line::new(String::new()));

        let header = format!("  {:<name_width$}  {}", "Provider", "Environment Variable");
        lines.push(Line::styled(header, dim));
        let separator = format!("  {:-<name_width$}  {:-<20}", "", "");
        lines.push(Line::styled(separator, dim));

        for p in &missing {
            let row = format!("  {:<name_width$}  {}", p.display_name, p.config_hint);
            lines.push(Line::styled(row, dim));
        }
    }

    lines.push(Line::new(String::new()));
    lines.push(Line::styled("  Press any key to continue.".to_string(), theme.muted()));

    lines
}

async fn run_provider_screen<W: io::Write>(
    discovered: &[LlmModel],
    renderer: &mut Renderer<W>,
    terminal_rx: &mut UnboundedReceiver<CrosstermEvent>,
) -> Result<(), CliError> {
    let statuses = detect_providers(discovered);

    renderer
        .render_frame(|ctx| tui::Frame::new(format_provider_lines(&statuses, &ctx.theme)))
        .map_err(CliError::IoError)?;

    loop {
        let Some(event) = terminal_rx.recv().await else {
            return Ok(());
        };
        if let CrosstermEvent::Resize(c, r) = &event {
            renderer.on_resize((*c, *r));
            renderer
                .render_frame(|ctx| tui::Frame::new(format_provider_lines(&statuses, &ctx.theme)))
                .map_err(CliError::IoError)?;
            continue;
        }
        if let CrosstermEvent::Key(_) = event {
            return Ok(());
        }
    }
}

struct WizardInput {
    name: String,
    description: String,
    model: String,
    reasoning_effort: Option<ReasoningEffort>,
    servers: Vec<String>,
}

impl WizardInput {
    fn from_form_and_selection(json: &Value, selection: ModelSelection) -> Self {
        let name = json["name"].as_str().unwrap_or("").to_string();
        let description = json["description"].as_str().unwrap_or("").to_string();
        let servers = json["servers"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        Self { name, description, model: selection.model, reasoning_effort: selection.reasoning_effort, servers }
    }
}

fn build_form(is_existing_project: bool) -> Form {
    let title =
        if is_existing_project { "Add a new agent".to_string() } else { "Create a new Aether project".to_string() };

    let mut fields = vec![
        FormField {
            name: "name".to_string(),
            label: "Agent Name".to_string(),
            description: None,
            required: true,
            kind: FormFieldKind::Text(TextField::new(String::new())),
        },
        FormField {
            name: "description".to_string(),
            label: "Description".to_string(),
            description: None,
            required: true,
            kind: FormFieldKind::Text(TextField::new(String::new())),
        },
    ];

    if !is_existing_project {
        let server_options = vec![
            SelectOption {
                value: "coding".to_string(),
                title: "Coding".to_string(),
                description: Some("Filesystem, search, and bash tools".to_string()),
            },
            SelectOption {
                value: "lsp".to_string(),
                title: "Lsp".to_string(),
                description: Some("Language Server Protocol integration".to_string()),
            },
            SelectOption {
                value: "skills".to_string(),
                title: "Skills".to_string(),
                description: Some("Skills and slash-commands".to_string()),
            },
            SelectOption {
                value: "subagents".to_string(),
                title: "Subagents".to_string(),
                description: Some("Spawn sub-agents in parallel".to_string()),
            },
            SelectOption {
                value: "tasks".to_string(),
                title: "Tasks".to_string(),
                description: Some("Task management tools, backed by JSONL files".to_string()),
            },
            SelectOption {
                value: "survey".to_string(),
                title: "Survey".to_string(),
                description: Some("Allow your agent to ask you structured questions".to_string()),
            },
        ];

        fields.push(FormField {
            name: "servers".to_string(),
            label: "MCP Servers".to_string(),
            description: None,
            required: true,
            kind: FormFieldKind::MultiSelect(MultiSelect::new(
                server_options,
                vec![true, true, true, true, true, true],
            )),
        });
    }

    Form::new(title, fields)
}

async fn run_form<W: io::Write>(
    form: &mut Form,
    renderer: &mut Renderer<W>,
    terminal_rx: &mut UnboundedReceiver<CrosstermEvent>,
) -> Result<Option<Value>, CliError> {
    renderer.render_frame(|ctx| form.render(ctx)).map_err(CliError::IoError)?;

    loop {
        let Some(event) = terminal_rx.recv().await else {
            return Ok(None);
        };
        if let CrosstermEvent::Resize(c, r) = &event {
            renderer.on_resize((*c, *r));
        }
        if let Ok(tui_event) = Event::try_from(event) {
            if let Some(msg) = form.on_event(&tui_event).await.and_then(|msgs| msgs.into_iter().next()) {
                match msg {
                    FormMessage::Submit => return Ok(Some(form.to_json())),
                    FormMessage::Close => return Ok(None),
                }
            }
            renderer.render_frame(|ctx| form.render(ctx)).map_err(CliError::IoError)?;
        }
    }
}

fn render_selector_with_footer(selector: &mut ModelSelector, ctx: &ViewContext) -> tui::Frame {
    selector.update_viewport(ctx.size.height.saturating_sub(2) as usize);
    let frame = selector.render(ctx);
    let mut lines = frame.into_lines();
    lines.push(Line::new(String::new()));
    lines.push(Line::styled(
        "  [enter] toggle  [tab] reasoning  [ctrl+s] done  [esc] cancel".to_string(),
        ctx.theme.muted(),
    ));
    tui::Frame::new(lines)
}

struct ModelSelection {
    model: String,
    reasoning_effort: Option<ReasoningEffort>,
}

async fn run_model_selector<W: io::Write>(
    selector: &mut ModelSelector,
    renderer: &mut Renderer<W>,
    terminal_rx: &mut UnboundedReceiver<CrosstermEvent>,
) -> Result<Option<ModelSelection>, CliError> {
    renderer.render_frame(|ctx| render_selector_with_footer(selector, ctx)).map_err(CliError::IoError)?;

    loop {
        let Some(event) = terminal_rx.recv().await else {
            return Ok(None);
        };
        if let CrosstermEvent::Resize(c, r) = &event {
            renderer.on_resize((*c, *r));
        }
        if let CrosstermEvent::Key(key) = &event {
            if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
                let selected = selector.selected_values();
                if selected.is_empty() {
                    return Ok(None);
                }
                let joined = selected.iter().cloned().collect::<Vec<_>>().join(",");
                return Ok(Some(ModelSelection { model: joined, reasoning_effort: selector.reasoning_effort() }));
            }
            if key.code == KeyCode::Esc {
                return Ok(None);
            }
        }
        if let Ok(tui_event) = Event::try_from(event) {
            let _ = selector.on_event(&tui_event).await;
            renderer.render_frame(|ctx| render_selector_with_footer(selector, ctx)).map_err(CliError::IoError)?;
        }
    }
}

fn scaffold(project_root: &Path, input: &WizardInput) -> Result<(), CliError> {
    std::fs::create_dir_all(project_root).map_err(CliError::IoError)?;

    write_if_absent(&project_root.join(".aether/SYSTEM.md"), SYSTEM_MD_TEMPLATE)?;
    write_if_absent(&project_root.join(".aether/mcp.json"), &build_mcp_json(input))?;
    write_if_absent(&project_root.join("AGENTS.md"), &build_agents_md(input))?;
    write_if_absent(&project_root.join(".aether/settings.json"), &build_settings_json(input))?;

    Ok(())
}

fn add_agent(settings_path: &Path, input: &WizardInput) -> Result<(), CliError> {
    let content = std::fs::read_to_string(settings_path).map_err(CliError::IoError)?;
    let mut settings: Value = serde_json::from_str(&content).map_err(|e| CliError::AgentError(e.to_string()))?;

    let agents = settings
        .as_object_mut()
        .and_then(|obj| obj.entry("agents").or_insert_with(|| Value::Array(Vec::new())).as_array_mut())
        .ok_or_else(|| CliError::AgentError("settings.json is not a valid object".to_string()))?;

    agents.push(build_agent_json(input));

    let output = serde_json::to_string_pretty(&settings).map_err(|e| CliError::AgentError(e.to_string()))?;
    std::fs::write(settings_path, output).map_err(CliError::IoError)?;
    println!("Added agent '{}' to {}", input.name, settings_path.display());

    Ok(())
}

fn write_if_absent(path: &Path, content: &str) -> Result<(), CliError> {
    if path.exists() {
        println!("Skipping: {}", path.display());
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(CliError::IoError)?;
    }
    std::fs::write(path, content).map_err(CliError::IoError)?;
    println!("Created: {}", path.display());
    Ok(())
}

fn build_agent_json(input: &WizardInput) -> Value {
    let mut agent = serde_json::json!({
        "name": input.name,
        "description": input.description,
        "model": input.model,
        "userInvocable": true,
        "agentInvocable": true,
        "prompts": []
    });
    if let Some(effort) = input.reasoning_effort {
        agent["reasoningEffort"] = Value::String(effort.as_str().to_string());
    }
    agent
}

fn build_settings_json(input: &WizardInput) -> String {
    let value = serde_json::json!({
        "prompts": [".aether/SYSTEM.md", "AGENTS.md"],
        "mcpServers": ".aether/mcp.json",
        "agents": [build_agent_json(input)]
    });
    serde_json::to_string_pretty(&value).expect("settings serialization cannot fail")
}

fn build_mcp_json(input: &WizardInput) -> String {
    let mut servers = serde_json::Map::new();
    for server in &input.servers {
        let mut entry = serde_json::Map::new();
        entry.insert("type".to_string(), serde_json::json!("in-memory"));
        if server == "skills" {
            entry.insert("args".to_string(), serde_json::json!(["--dir", "$HOME/.aether"]));
        }
        servers.insert(server.clone(), Value::Object(entry));
    }
    let value = serde_json::json!({ "servers": servers });
    serde_json::to_string_pretty(&value).expect("mcp serialization cannot fail")
}

fn build_agents_md(input: &WizardInput) -> String {
    format!("# {}\n\n{}\n\nYou are an expert coding assistant.\n", input.name, input.description)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_project::load_agent_catalog;
    use mcp_utils::client::config::RawMcpConfig;
    use std::fs;

    fn default_input() -> WizardInput {
        WizardInput {
            name: "Default".to_string(),
            description: "Default coding agent".to_string(),
            model: "anthropic:claude-sonnet-4-5".to_string(),
            reasoning_effort: None,
            servers: vec!["coding".to_string(), "skills".to_string(), "tasks".to_string()],
        }
    }

    #[test]
    fn scaffold_writes_all_files() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path(), &default_input()).unwrap();

        assert!(dir.path().join(".aether/settings.json").exists());
        assert!(dir.path().join(".aether/mcp.json").exists());
        assert!(dir.path().join(".aether/SYSTEM.md").exists());
        assert!(dir.path().join("AGENTS.md").exists());
    }

    #[test]
    fn scaffold_skips_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        let agents_path = dir.path().join("AGENTS.md");
        fs::write(&agents_path, "My custom prompt").unwrap();

        scaffold(dir.path(), &default_input()).unwrap();

        let content = fs::read_to_string(&agents_path).unwrap();
        assert_eq!(content, "My custom prompt");
    }

    #[test]
    fn scaffold_rejects_invalid_model() {
        let input = WizardInput { model: "invalid:nope".to_string(), ..default_input() };
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path(), &input).unwrap();

        let result = load_agent_catalog(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn scaffold_settings_json_is_valid() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path(), &default_input()).unwrap();

        let catalog = load_agent_catalog(dir.path()).unwrap();
        assert_eq!(catalog.all().len(), 1);
        assert_eq!(catalog.all()[0].name, "Default");
    }

    #[test]
    fn scaffold_mcp_json_is_valid() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path(), &default_input()).unwrap();

        let mcp_path = dir.path().join(".aether/mcp.json");
        let raw = RawMcpConfig::from_json_file(&mcp_path).unwrap();
        assert_eq!(raw.servers.len(), 3);
        assert!(raw.servers.contains_key("coding"));
        assert!(raw.servers.contains_key("skills"));
        assert!(raw.servers.contains_key("tasks"));
    }

    #[test]
    fn scaffold_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let input = default_input();
        scaffold(dir.path(), &input).unwrap();
        scaffold(dir.path(), &input).unwrap();

        assert!(dir.path().join(".aether/settings.json").exists());
    }

    #[test]
    fn scaffold_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("deep/nested/project");
        scaffold(&nested, &default_input()).unwrap();

        assert!(nested.join(".aether/settings.json").exists());
        assert!(nested.join(".aether/mcp.json").exists());
        assert!(nested.join(".aether/SYSTEM.md").exists());
        assert!(nested.join("AGENTS.md").exists());
    }

    #[test]
    fn scaffold_custom_servers() {
        let dir = tempfile::tempdir().unwrap();
        let input = WizardInput { servers: vec!["coding".to_string(), "lsp".to_string()], ..default_input() };
        scaffold(dir.path(), &input).unwrap();

        let raw = RawMcpConfig::from_json_file(&dir.path().join(".aether/mcp.json")).unwrap();
        assert_eq!(raw.servers.len(), 2);
        assert!(raw.servers.contains_key("coding"));
        assert!(raw.servers.contains_key("lsp"));
        assert!(!raw.servers.contains_key("tasks"));
    }

    #[test]
    fn build_model_entries_includes_available() {
        let items = build_model_entries(&[]);
        // Should only include models where the env var is set (or has no requirement)
        for item in &items {
            let model: LlmModel = item.value.parse().unwrap();
            assert!(
                model.required_env_var().is_none_or(|var| std::env::var(var).is_ok()),
                "model {} should be available",
                item.value
            );
        }
    }

    #[test]
    fn generated_settings_reference_aether_paths() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path(), &default_input()).unwrap();

        let settings_path = dir.path().join(".aether/settings.json");
        let content = fs::read_to_string(&settings_path).unwrap();
        let settings: Value = serde_json::from_str(&content).unwrap();

        let prompts = settings["prompts"].as_array().unwrap();
        assert!(prompts.contains(&Value::String(".aether/SYSTEM.md".to_string())));
        assert!(prompts.contains(&Value::String("AGENTS.md".to_string())));

        assert_eq!(settings["mcpServers"].as_str().unwrap(), ".aether/mcp.json");

        let agents = settings["agents"].as_array().unwrap();
        assert_eq!(agents.len(), 1);
        assert!(agents[0]["prompts"].as_array().unwrap().is_empty());
    }

    #[test]
    fn scaffold_system_md_is_written() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path(), &default_input()).unwrap();

        let system_md_path = dir.path().join(".aether/SYSTEM.md");
        assert!(system_md_path.exists());

        let content = fs::read_to_string(&system_md_path).unwrap();
        assert!(!content.is_empty());
    }

    #[test]
    fn scaffold_system_md_matches_template() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path(), &default_input()).unwrap();

        let system_md_path = dir.path().join(".aether/SYSTEM.md");
        let content = fs::read_to_string(&system_md_path).unwrap();

        assert_eq!(content, SYSTEM_MD_TEMPLATE);
    }

    #[test]
    fn default_agent_inherits_generated_mcp() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path(), &default_input()).unwrap();

        let catalog = load_agent_catalog(dir.path()).unwrap();
        let model: llm::LlmModel = "anthropic:claude-sonnet-4-5".parse().unwrap();
        let default_agent = catalog.resolve_default(&model, None, dir.path());

        let expected_path = dir.path().join(".aether/mcp.json");
        assert_eq!(default_agent.mcp_config_path, Some(expected_path));
    }

    #[test]
    fn add_agent_appends_to_existing_settings() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path(), &default_input()).unwrap();

        let settings_path = dir.path().join(".aether/settings.json");
        let new_agent = WizardInput {
            name: "Researcher".to_string(),
            description: "Research agent".to_string(),
            model: "anthropic:claude-sonnet-4-5".to_string(),
            reasoning_effort: None,
            servers: vec![],
        };
        add_agent(&settings_path, &new_agent).unwrap();

        let catalog = load_agent_catalog(dir.path()).unwrap();
        assert_eq!(catalog.all().len(), 2);
        assert_eq!(catalog.all()[0].name, "Default");
        assert_eq!(catalog.all()[1].name, "Researcher");
    }

    #[test]
    fn scaffold_includes_reasoning_effort() {
        let dir = tempfile::tempdir().unwrap();
        let input = WizardInput { reasoning_effort: Some(ReasoningEffort::High), ..default_input() };
        scaffold(dir.path(), &input).unwrap();

        let catalog = load_agent_catalog(dir.path()).unwrap();
        assert_eq!(catalog.all()[0].reasoning_effort, Some(ReasoningEffort::High));
    }

    #[test]
    fn scaffold_omits_reasoning_effort_when_none() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path(), &default_input()).unwrap();

        let content = std::fs::read_to_string(dir.path().join(".aether/settings.json")).unwrap();
        assert!(!content.contains("reasoningEffort"));
    }

    #[test]
    fn detect_providers_includes_catalog_providers() {
        let statuses = detect_providers(&[]);
        assert!(statuses.iter().any(|p| p.display_name == "Anthropic"));
    }

    #[test]
    fn detect_providers_includes_discovered_local() {
        let discovered = vec![LlmModel::Ollama("llama3".to_string()), LlmModel::Ollama("phi3".to_string())];
        let statuses = detect_providers(&discovered);
        let ollama = statuses.iter().find(|p| p.display_name == "Ollama").expect("expected Ollama entry");
        assert!(ollama.detected);
        assert!(ollama.config_hint.contains("2 models"));
    }

    #[test]
    fn format_provider_lines_includes_all_providers() {
        let theme = Theme::default();
        let statuses = detect_providers(&[]);
        let lines = format_provider_lines(&statuses, &theme);
        let text: String = lines.iter().map(|l| l.plain_text()).collect::<Vec<_>>().join("\n");
        assert!(text.contains("Not Configured"));
        assert!(text.contains("Anthropic"));
    }
}
