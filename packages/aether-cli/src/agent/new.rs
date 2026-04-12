use crate::agent::NewArgs;
use crate::agent::new_agent_wizard::{
    NewAgentMode, NewAgentOutcome, NewAgentWizard, add_agent, available_prompt_files, detect_mcp_configs,
    run_wizard_loop, scaffold,
};
use crate::error::CliError;
use llm::LlmModel;
use llm::catalog::available_models;
use llm::providers::local::discovery::discover_local_models;
use std::io;
use tui::{MouseCapture, TerminalConfig, TerminalRuntime, Theme, terminal_size};
use wisp::components::model_selector::ModelEntry;

pub async fn run_new(args: NewArgs) -> Result<NewAgentOutcome, CliError> {
    let project_root = args.path.canonicalize().unwrap_or(args.path);
    let settings_path = project_root.join(".aether/settings.json");
    let is_existing = settings_path.is_file();
    let mode = if is_existing { NewAgentMode::AddAgentToExistingProject } else { NewAgentMode::ScaffoldProject };

    let discovery_handle = tokio::spawn(discover_local_models());

    let size = terminal_size().unwrap_or((80, 24));
    let mut terminal = TerminalRuntime::new(
        io::stdout(),
        Theme::default(),
        size,
        TerminalConfig { bracketed_paste: false, mouse_capture: MouseCapture::Enabled },
    )
    .map_err(CliError::IoError)?;

    let discovered = discovery_handle.await.unwrap_or_default();
    let model_entries = build_model_entries(&discovered);

    if model_entries.is_empty() {
        terminal.clear_screen().map_err(CliError::IoError)?;
        println!("No providers detected. Set an API key environment variable and try again.");
        return Ok(NewAgentOutcome::Cancelled);
    }

    let prompt_options = available_prompt_files(&mode, &project_root);
    let mcp_configs = detect_mcp_configs(&project_root);
    let mut wizard = NewAgentWizard::new(mode, model_entries, &prompt_options, &mcp_configs);

    let outcome = run_wizard_loop(&mut wizard, &mut terminal).await?;
    terminal.clear_screen().map_err(CliError::IoError)?;

    if matches!(outcome, NewAgentOutcome::Cancelled) {
        println!("Cancelled.");
        return Ok(NewAgentOutcome::Cancelled);
    }

    let draft = wizard.into_draft();

    if is_existing {
        add_agent(&settings_path, &draft)?;
    } else {
        scaffold(&project_root, &draft)?;
    }

    Ok(NewAgentOutcome::Applied)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_model_entries_includes_available() {
        let items = build_model_entries(&[]);
        for item in &items {
            let model: LlmModel = item.value.parse().unwrap();
            assert!(
                model.required_env_var().is_none_or(|var| std::env::var(var).is_ok()),
                "model {} should be available",
                item.value
            );
        }
    }
}
