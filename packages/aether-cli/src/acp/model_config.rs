use agent_client_protocol::{self as acp, SessionConfigOption, SessionConfigOptionCategory};
use llm::catalog::{self, LlmModel};
use llm::oauth::OAuthCredentialStore;
use std::collections::{BTreeMap, HashSet};

fn needs_oauth_login(model: &LlmModel, credential_ids: &HashSet<String>) -> bool {
    model
        .oauth_provider_id()
        .is_some_and(|id| !credential_ids.contains(id))
}

pub(crate) fn unavailable_reason(model: &LlmModel, credential_ids: &HashSet<String>) -> String {
    if needs_oauth_login(model, credential_ids) {
        let oauth_id = model.oauth_provider_id().unwrap_or("unknown");
        return format!("Needs login: run `aether auth {oauth_id}`");
    }
    model.required_env_var().map_or_else(
        || "Unavailable: provider is not configured".to_string(),
        |var| format!("Unavailable: set {var}"),
    )
}

pub(crate) fn model_exists(available: &[LlmModel], model_str: &str) -> bool {
    model_str
        .split(',')
        .map(str::trim)
        .all(|part| available.iter().any(|m| m.to_string() == part))
}

pub(crate) fn effective_model<'a>(
    active_model: &'a str,
    pending_model: Option<&'a str>,
) -> &'a str {
    pending_model.unwrap_or(active_model)
}

/// Build the "Model" select config option with all models from all providers.
/// Display names use "Provider / `ModelName`" format.
/// Fully-unavailable providers are collapsed into a single summary line.
struct ProviderGroup<'a> {
    models: Vec<&'a LlmModel>,
    available_count: usize,
}

pub(crate) fn build_model_config_option(
    available: &[LlmModel],
    current_model: &str,
) -> SessionConfigOption {
    let all_models = catalog::LlmModel::all();
    let available_models: HashSet<String> = available.iter().map(ToString::to_string).collect();
    let credential_ids = OAuthCredentialStore::credential_ids_sync();

    // Phase 1: Group models by provider, counting available models per provider
    let mut groups: BTreeMap<&str, ProviderGroup<'_>> = BTreeMap::new();
    for m in all_models {
        let value = m.to_string();
        let is_available = available_models.contains(&value);
        let group = groups.entry(m.provider()).or_insert_with(|| ProviderGroup {
            models: Vec::new(),
            available_count: 0,
        });
        group.models.push(m);
        if is_available {
            group.available_count += 1;
        }
    }

    // Phase 2: Emit options per group
    let mut options: Vec<acp::SessionConfigSelectOption> = Vec::new();
    for group in groups.values() {
        let display = group.models[0].provider_display_name();
        if group.available_count == 0 {
            // Fully unavailable — emit one collapsed entry
            let provider_key = group.models[0].provider();
            let count = group.models.len();
            let noun = if count == 1 { "model" } else { "models" };
            let name = format!("{display} ({count} {noun})");
            let value = format!("__unavailable:{provider_key}");
            let reason = unavailable_reason(group.models[0], &credential_ids);
            options.push(acp::SessionConfigSelectOption::new(value, name).description(reason));
        } else {
            // Mixed or fully available — list each model individually
            for m in &group.models {
                let value = m.to_string();
                let is_available = available_models.contains(&value);
                let needs_login = needs_oauth_login(m, &credential_ids);
                let name = if is_available && !needs_login {
                    format!("{display} / {}", m.display_name())
                } else if needs_login {
                    format!(
                        "{display} / {} (needs login — run `aether auth {}`)",
                        m.display_name(),
                        m.oauth_provider_id().unwrap_or("unknown")
                    )
                } else {
                    format!("{display} / {} (unavailable)", m.display_name())
                };
                let option = acp::SessionConfigSelectOption::new(value, name);
                if is_available && !needs_login {
                    options.push(option);
                } else {
                    options.push(option.description(unavailable_reason(m, &credential_ids)));
                }
            }
        }
    }

    let mut meta = serde_json::Map::new();
    meta.insert("multi_select".to_string(), serde_json::Value::Bool(true));

    SessionConfigOption::select("model", "Model", current_model.to_string(), options)
        .category(SessionConfigOptionCategory::Model)
        .meta(meta)
}

/// Build config options for the given state
pub(crate) fn build_config_options(
    available: &[LlmModel],
    current_model: &str,
) -> Vec<SessionConfigOption> {
    vec![build_model_config_option(available, current_model)]
}

/// Pick a default model from the available list.
/// Prefers Claude Sonnet 4.5 (latest alias), then first available.
pub(crate) fn pick_default_model(available: &[LlmModel]) -> Option<&LlmModel> {
    // Prefer claude-sonnet-4-5 (latest alias)
    available
        .iter()
        .find(|m| m.model_id() == "claude-sonnet-4-5")
        .or_else(|| available.first())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::{SessionConfigKind, SessionConfigSelectOptions};
    use llm::catalog::{AnthropicModel, DeepSeekModel, GeminiModel};

    fn test_models() -> Vec<LlmModel> {
        vec![
            LlmModel::Anthropic(AnthropicModel::ClaudeSonnet45),
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus46),
            LlmModel::DeepSeek(DeepSeekModel::DeepseekChat),
            LlmModel::Gemini(GeminiModel::Gemini25Pro),
        ]
    }

    #[test]
    fn build_model_config_option_includes_all_providers() {
        let models = test_models();
        let opt = build_model_config_option(&models, "anthropic:claude-sonnet-4-5");

        assert_eq!(opt.id.0.as_ref(), "model");

        let SessionConfigKind::Select(ref select) = opt.kind else {
            panic!("Expected Select kind");
        };

        let SessionConfigSelectOptions::Ungrouped(ref options) = select.options else {
            panic!("Expected Ungrouped options");
        };

        // Available providers list models individually
        assert!(options.iter().any(|o| o.value.0.starts_with("anthropic:")));
        assert!(options.iter().any(|o| o.value.0.starts_with("deepseek:")));
        assert!(options.iter().any(|o| o.value.0.starts_with("gemini:")));

        // Fully-unavailable providers are collapsed into sentinel entries
        assert!(
            options
                .iter()
                .any(|o| o.value.0.as_ref() == "__unavailable:moonshot")
        );
        assert!(
            options
                .iter()
                .any(|o| o.value.0.as_ref() == "__unavailable:openrouter")
        );
        assert!(
            options
                .iter()
                .any(|o| o.value.0.as_ref() == "__unavailable:zai")
        );
    }

    #[test]
    fn build_model_config_option_uses_provider_slash_model_display_names() {
        let models = test_models();
        let opt = build_model_config_option(&models, "anthropic:claude-sonnet-4-5");

        let SessionConfigKind::Select(ref select) = opt.kind else {
            panic!("Expected Select kind");
        };
        let SessionConfigSelectOptions::Ungrouped(ref options) = select.options else {
            panic!("Expected Ungrouped options");
        };

        // Available models should have "Provider / Model" format
        let sonnet = options
            .iter()
            .find(|o| o.value.0.as_ref() == "anthropic:claude-sonnet-4-5")
            .expect("expected anthropic sonnet option");
        assert!(
            sonnet.name.starts_with("Anthropic / "),
            "Expected 'Anthropic / ...' display name, got: {}",
            sonnet.name
        );
    }

    #[test]
    fn build_model_config_option_marks_unavailable_models_with_reason() {
        let models = test_models();
        let opt = build_model_config_option(&models, "anthropic:claude-sonnet-4-5");

        let SessionConfigKind::Select(ref select) = opt.kind else {
            panic!("Expected Select kind");
        };
        let SessionConfigSelectOptions::Ungrouped(ref options) = select.options else {
            panic!("Expected Ungrouped options");
        };

        let unavailable = options
            .iter()
            .find(|o| o.name.contains("unavailable"))
            .expect("expected at least one unavailable model option");
        assert!(unavailable.name.contains(" / "));
        assert!(
            unavailable
                .description
                .as_deref()
                .is_some_and(|d| d.starts_with("Unavailable:"))
        );
    }

    #[test]
    fn build_config_options_returns_single_model_option() {
        let models = test_models();
        let opts = build_config_options(&models, "deepseek:deepseek-chat");

        assert_eq!(opts.len(), 1);
        assert_eq!(opts[0].id.0.as_ref(), "model");

        let SessionConfigKind::Select(ref model_select) = opts[0].kind else {
            panic!("Expected Select kind");
        };
        assert_eq!(
            model_select.current_value.0.as_ref(),
            "deepseek:deepseek-chat"
        );

        // Should include models from all providers
        let SessionConfigSelectOptions::Ungrouped(ref model_options) = model_select.options else {
            panic!("Expected Ungrouped options");
        };
        assert!(
            model_options
                .iter()
                .any(|o| o.value.0.starts_with("anthropic:"))
        );
        assert!(
            model_options
                .iter()
                .any(|o| o.value.0.starts_with("deepseek:"))
        );
    }

    #[test]
    fn model_exists_accepts_known_model() {
        let models = test_models();
        assert!(model_exists(&models, "anthropic:claude-sonnet-4-5"));
        assert!(model_exists(&models, "deepseek:deepseek-chat"));
    }

    #[test]
    fn model_exists_rejects_unknown_model() {
        let models = test_models();
        assert!(!model_exists(&models, "anthropic:not-real"));
        assert!(!model_exists(&models, "mystery:some-model"));
    }

    #[test]
    fn model_exists_accepts_comma_separated_known_models() {
        let models = test_models();
        assert!(model_exists(
            &models,
            "anthropic:claude-sonnet-4-5,deepseek:deepseek-chat"
        ));
    }

    #[test]
    fn model_exists_rejects_comma_separated_with_unknown() {
        let models = test_models();
        assert!(!model_exists(
            &models,
            "anthropic:claude-sonnet-4-5,mystery:nope"
        ));
    }

    #[test]
    fn build_model_config_option_includes_multi_select_meta() {
        let models = test_models();
        let opt = build_model_config_option(&models, "anthropic:claude-sonnet-4-5");
        let meta = opt.meta.expect("meta should be set");
        assert_eq!(
            meta.get("multi_select"),
            Some(&serde_json::Value::Bool(true))
        );
    }

    #[test]
    fn effective_model_prefers_pending() {
        assert_eq!(
            effective_model(
                "anthropic:claude-sonnet-4-5",
                Some("deepseek:deepseek-chat")
            ),
            "deepseek:deepseek-chat"
        );
    }

    #[test]
    fn effective_model_falls_back_to_active() {
        assert_eq!(
            effective_model("anthropic:claude-sonnet-4-5", None),
            "anthropic:claude-sonnet-4-5"
        );
    }

    #[test]
    fn collapsed_entry_for_fully_unavailable_provider() {
        // test_models() has no Moonshot models available
        let models = test_models();
        let opt = build_model_config_option(&models, "anthropic:claude-sonnet-4-5");

        let SessionConfigKind::Select(ref select) = opt.kind else {
            panic!("Expected Select kind");
        };
        let SessionConfigSelectOptions::Ungrouped(ref options) = select.options else {
            panic!("Expected Ungrouped options");
        };

        let moonshot = options
            .iter()
            .find(|o| o.value.0.as_ref() == "__unavailable:moonshot")
            .expect("expected collapsed moonshot entry");

        // Name should be "Moonshot (N models)"
        assert!(
            moonshot.name.starts_with("Moonshot ("),
            "Expected 'Moonshot (N models)', got: {}",
            moonshot.name
        );
        assert!(moonshot.name.ends_with("models)"));

        // Description triggers is_disabled in TUI
        assert!(
            moonshot
                .description
                .as_deref()
                .is_some_and(|d| d.starts_with("Unavailable:"))
        );
    }

    #[test]
    fn mixed_provider_lists_models_individually() {
        // test_models() has Gemini25Pro available, so Gemini is "mixed"
        let models = test_models();
        let opt = build_model_config_option(&models, "anthropic:claude-sonnet-4-5");

        let SessionConfigKind::Select(ref select) = opt.kind else {
            panic!("Expected Select kind");
        };
        let SessionConfigSelectOptions::Ungrouped(ref options) = select.options else {
            panic!("Expected Ungrouped options");
        };

        // Should NOT have a collapsed entry for gemini
        assert!(
            !options
                .iter()
                .any(|o| o.value.0.as_ref() == "__unavailable:gemini"),
            "Gemini should not be collapsed when it has available models"
        );

        // Individual gemini models should still be listed
        assert!(
            options
                .iter()
                .any(|o| o.value.0.starts_with("gemini:") && !o.name.contains("unavailable"))
        );
        assert!(
            options
                .iter()
                .any(|o| o.value.0.starts_with("gemini:") && o.name.contains("unavailable"))
        );
    }
}
