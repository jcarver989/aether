use crate::error::CliError;
use crate::init::InitArgs;
use llm::LlmModel;
use llm::providers::local::discovery::discover_local_models;
use serde_json::Value;
use std::io;
use std::path::Path;
use tokio::sync::mpsc::UnboundedReceiver;
use tui::{
    Component, CrosstermEvent, Event, Form, FormField, FormFieldKind, FormMessage, MouseCapture,
    MultiSelect, Renderer, SelectOption, TerminalSession, TextField, Theme,
    spawn_terminal_event_task, terminal_size,
};
use wisp::components::model_selector::{ModelEntry, ModelSelector, ModelSelectorMessage};

const SYSTEM_MD_TEMPLATE: &str = include_str!("../../templates/SYSTEM.md");

pub async fn run_init(args: InitArgs) -> Result<(), CliError> {
    let project_root = args.path.canonicalize().unwrap_or(args.path);
    // Scope the TUI session and renderer so they drop before we print to
    // stdout — raw-mode causes println! to emit bare \n without \r,
    // producing a staircase effect in the output.
    let (form_values, model) = {
        let size = terminal_size().unwrap_or((80, 24));
        let mut renderer = Renderer::new(io::stdout(), Theme::default(), size);
        let _session =
            TerminalSession::new(false, MouseCapture::Disabled).map_err(CliError::IoError)?;

        let mut terminal_rx = spawn_terminal_event_task();
        let mut form = build_form();
        let form_result = run_form(&mut form, &mut renderer, &mut terminal_rx).await?;
        let Some(form_values) = form_result else {
            renderer.clear_screen().map_err(CliError::IoError)?;
            println!("Cancelled.");
            return Ok(());
        };

        renderer.clear_screen().map_err(CliError::IoError)?;
        let mut selector = ModelSelector::new(
            build_model_entries().await,
            "model".to_string(),
            Some("anthropic:claude-sonnet-4-5"),
            None,
        );

        let Some(model) =
            run_model_selector(&mut selector, &mut renderer, &mut terminal_rx).await?
        else {
            renderer.clear_screen().map_err(CliError::IoError)?;
            println!("Cancelled.");
            return Ok(());
        };

        renderer.clear_screen().map_err(CliError::IoError)?;
        (form_values, model)
    };

    let input = WizardInput::from_form_and_model(&form_values, model);
    scaffold(&project_root, &input)?;
    Ok(())
}

async fn build_model_entries() -> Vec<ModelEntry> {
    let discovered = discover_local_models().await;
    LlmModel::all()
        .iter()
        .cloned()
        .chain(discovered)
        .map(|m| ModelEntry {
            value: m.to_string(),
            name: format!("{} / {}", m.provider_display_name(), m.display_name()),
            reasoning_levels: m.reasoning_levels().to_vec(),
        })
        .collect()
}

/// Values extracted from the wizard.
struct WizardInput {
    name: String,
    description: String,
    model: String,
    servers: Vec<String>,
}

impl WizardInput {
    fn from_form_and_model(json: &Value, model: String) -> Self {
        let name = json["name"].as_str().unwrap_or("Default").to_string();
        let description = json["description"]
            .as_str()
            .unwrap_or("Default coding agent")
            .to_string();
        let servers = json["servers"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Self {
            name,
            description,
            model,
            servers,
        }
    }
}

fn build_form() -> Form {
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

    Form::new(
        "Initialize a new Aether project".to_string(),
        vec![
            FormField {
                name: "name".to_string(),
                label: "Agent Name".to_string(),
                description: None,
                required: true,
                kind: FormFieldKind::Text(TextField::new("Default".to_string())),
            },
            FormField {
                name: "description".to_string(),
                label: "Description".to_string(),
                description: None,
                required: true,
                kind: FormFieldKind::Text(TextField::new("Default coding agent".to_string())),
            },
            FormField {
                name: "servers".to_string(),
                label: "MCP Servers".to_string(),
                description: None,
                required: true,
                kind: FormFieldKind::MultiSelect(MultiSelect::new(
                    server_options,
                    vec![true, true, true, false, true, true],
                )),
            },
        ],
    )
}

async fn run_form<W: io::Write>(
    form: &mut Form,
    renderer: &mut Renderer<W>,
    terminal_rx: &mut UnboundedReceiver<CrosstermEvent>,
) -> Result<Option<Value>, CliError> {
    renderer
        .render_frame(|ctx| form.render(ctx))
        .map_err(CliError::IoError)?;

    loop {
        let Some(event) = terminal_rx.recv().await else {
            return Ok(None);
        };
        if let CrosstermEvent::Resize(c, r) = &event {
            renderer.on_resize((*c, *r));
        }
        if let Ok(tui_event) = Event::try_from(event) {
            if let Some(msg) = form
                .on_event(&tui_event)
                .await
                .and_then(|msgs| msgs.into_iter().next())
            {
                match msg {
                    FormMessage::Submit => return Ok(Some(form.to_json())),
                    FormMessage::Close => return Ok(None),
                }
            }
            renderer
                .render_frame(|ctx| form.render(ctx))
                .map_err(CliError::IoError)?;
        }
    }
}

async fn run_model_selector<W: io::Write>(
    selector: &mut ModelSelector,
    renderer: &mut Renderer<W>,
    terminal_rx: &mut UnboundedReceiver<CrosstermEvent>,
) -> Result<Option<String>, CliError> {
    renderer
        .render_frame(|ctx| {
            selector.update_viewport(ctx.size.height as usize);
            selector.render(ctx)
        })
        .map_err(CliError::IoError)?;

    loop {
        let Some(event) = terminal_rx.recv().await else {
            return Ok(None);
        };
        if let CrosstermEvent::Resize(c, r) = &event {
            renderer.on_resize((*c, *r));
        }
        if let Ok(tui_event) = Event::try_from(event) {
            if let Some(msg) = selector
                .on_event(&tui_event)
                .await
                .and_then(|msgs| msgs.into_iter().next())
            {
                match msg {
                    ModelSelectorMessage::Done(changes) => {
                        let model = changes
                            .into_iter()
                            .find(|c| c.config_id == "model")
                            .map(|c| c.new_value);
                        return Ok(model);
                    }
                }
            }
            renderer
                .render_frame(|ctx| {
                    selector.update_viewport(ctx.size.height as usize);
                    selector.render(ctx)
                })
                .map_err(CliError::IoError)?;
        }
    }
}

/// Write the project scaffold files, skipping any that already exist.
fn scaffold(project_root: &Path, input: &WizardInput) -> Result<(), CliError> {
    std::fs::create_dir_all(project_root).map_err(CliError::IoError)?;

    write_if_absent(&project_root.join(".aether/SYSTEM.md"), SYSTEM_MD_TEMPLATE)?;
    write_if_absent(
        &project_root.join(".aether/mcp.json"),
        &build_mcp_json(input),
    )?;
    write_if_absent(&project_root.join("AGENTS.md"), &build_agents_md(input))?;
    write_if_absent(
        &project_root.join(".aether/settings.json"),
        &build_settings_json(input),
    )?;

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

fn build_settings_json(input: &WizardInput) -> String {
    let value = serde_json::json!({
        "prompts": [".aether/SYSTEM.md", "AGENTS.md"],
        "mcpServers": ".aether/mcp.json",
        "agents": [{
            "name": input.name,
            "description": input.description,
            "model": input.model,
            "userInvocable": true,
            "agentInvocable": true,
            "prompts": []
        }]
    });
    serde_json::to_string_pretty(&value).expect("settings serialization cannot fail")
}

fn build_mcp_json(input: &WizardInput) -> String {
    let mut servers = serde_json::Map::new();
    for server in &input.servers {
        let mut entry = serde_json::Map::new();
        entry.insert("type".to_string(), serde_json::json!("in-memory"));
        if server == "skills" {
            entry.insert(
                "args".to_string(),
                serde_json::json!(["--dir", "$HOME/.aether"]),
            );
        }
        servers.insert(server.clone(), Value::Object(entry));
    }
    let value = serde_json::json!({ "servers": servers });
    serde_json::to_string_pretty(&value).expect("mcp serialization cannot fail")
}

fn build_agents_md(input: &WizardInput) -> String {
    format!(
        "# {}\n\n{}\n\nYou are an expert coding assistant.\n",
        input.name, input.description
    )
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
            servers: vec![
                "coding".to_string(),
                "skills".to_string(),
                "tasks".to_string(),
            ],
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
        let input = WizardInput {
            model: "invalid:nope".to_string(),
            ..default_input()
        };
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
        let input = WizardInput {
            servers: vec!["coding".to_string(), "lsp".to_string()],
            ..default_input()
        };
        scaffold(dir.path(), &input).unwrap();

        let raw = RawMcpConfig::from_json_file(&dir.path().join(".aether/mcp.json")).unwrap();
        assert_eq!(raw.servers.len(), 2);
        assert!(raw.servers.contains_key("coding"));
        assert!(raw.servers.contains_key("lsp"));
        assert!(!raw.servers.contains_key("tasks"));
    }

    #[tokio::test]
    async fn build_model_entries_has_items() {
        let items = build_model_entries().await;
        assert!(!items.is_empty());
    }

    #[tokio::test]
    async fn build_model_entries_includes_default() {
        let items = build_model_entries().await;
        assert!(
            items
                .iter()
                .any(|e| e.value == "anthropic:claude-sonnet-4-5")
        );
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
}
