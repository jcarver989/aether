use crate::agent::NewArgs;
use crate::agent::new_agent_wizard::{
    NewAgentMode, NewAgentOutcome, NewAgentWizard, add_agent, available_prompt_files, detect_mcp_configs,
    run_wizard_loop, scaffold,
};
use crate::error::CliError;
use llm::LlmModel;
use llm::catalog::available_models;
use llm::providers::local::discovery::discover_local_models;
use std::collections::BTreeMap;
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

    if !model_entries.iter().any(|e| !e.is_disabled()) {
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
    let available: std::collections::HashSet<String> = available_models().iter().map(ToString::to_string).collect();

    let mut entries: Vec<ModelEntry> = available_models()
        .into_iter()
        .chain(discovered.iter().cloned())
        .map(|m| ModelEntry {
            value: m.to_string(),
            name: format!("{} / {}", m.provider_display_name(), m.display_name()),
            reasoning_levels: m.reasoning_levels().to_vec(),
            supports_image: m.supports_image(),
            supports_audio: m.supports_audio(),
            disabled_reason: None,
        })
        .collect();

    let mut unavailable_providers: BTreeMap<&str, (usize, &str, Option<&str>)> = BTreeMap::new();
    for m in LlmModel::all() {
        if available.contains(&m.to_string()) {
            continue;
        }
        let entry =
            unavailable_providers.entry(m.provider()).or_insert((0, m.provider_display_name(), m.required_env_var()));
        entry.0 += 1;
    }
    for (provider_key, (count, display, env_var)) in &unavailable_providers {
        let noun = if *count == 1 { "model" } else { "models" };
        let reason = env_var.map_or("provider is not configured".to_string(), |var| format!("set {var}"));
        entries.push(ModelEntry {
            value: format!("__unavailable:{provider_key}"),
            name: format!("{display} / {display} ({count} {noun})"),
            reasoning_levels: vec![],
            supports_image: false,
            supports_audio: false,
            disabled_reason: Some(reason),
        });
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_model_entries_includes_available() {
        let items = build_model_entries(&[]);
        for item in items.iter().filter(|e| !e.is_disabled()) {
            let model: LlmModel = item.value.parse().unwrap();
            assert!(
                model.required_env_var().is_none_or(|var| std::env::var(var).is_ok()),
                "model {} should be available",
                item.value
            );
        }
    }

    #[test]
    fn build_model_entries_includes_unavailable_providers() {
        let items = build_model_entries(&[]);
        let disabled: Vec<_> = items.iter().filter(|e| e.is_disabled()).collect();
        let available_providers: std::collections::HashSet<&str> =
            items.iter().filter(|e| !e.is_disabled()).filter_map(|e| e.value.split_once(':').map(|(p, _)| p)).collect();

        for entry in &disabled {
            assert!(entry.value.starts_with("__unavailable:"), "disabled entry should use __unavailable: prefix");
            assert!(entry.disabled_reason.is_some(), "disabled entry should have a reason");
            let provider = entry.value.strip_prefix("__unavailable:").unwrap();
            assert!(
                !available_providers.contains(provider),
                "disabled provider {provider} should not also be in available set"
            );
        }
    }
}
