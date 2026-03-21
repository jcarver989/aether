use acp_utils::config_meta::{ConfigOptionMeta, SelectOptionMeta};
use acp_utils::config_option_id::ConfigOptionId;
use aether_core::agent_spec::AgentSpec;
use agent_client_protocol::{self as acp, SessionConfigOption, SessionConfigOptionCategory};
use llm::ReasoningEffort;
use llm::catalog::LlmModel;
use llm::oauth::OAuthCredentialStore;
use std::collections::{BTreeMap, HashSet};

fn needs_oauth_login(model: &LlmModel) -> bool {
    model
        .oauth_provider_id()
        .is_some_and(|id| !OAuthCredentialStore::has_credential(id))
}

pub(crate) fn unavailable_reason(model: &LlmModel) -> String {
    if needs_oauth_login(model) {
        return "Needs login".to_string();
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
/// Display names use "Provider: `ModelName`" format.
/// Fully-unavailable providers are collapsed into a single summary line.
struct ProviderGroup<'a> {
    models: Vec<&'a LlmModel>,
    available_count: usize,
}

pub(crate) fn build_model_config_option(
    available: &[LlmModel],
    current_model: &str,
    all_models: &[LlmModel],
) -> SessionConfigOption {
    let available_models: HashSet<String> = available.iter().map(ToString::to_string).collect();

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
            let reason = unavailable_reason(group.models[0]);
            options.push(acp::SessionConfigSelectOption::new(value, name).description(reason));
        } else {
            // Mixed or fully available — list each model individually
            for m in &group.models {
                let value = m.to_string();
                let is_available = available_models.contains(&value);
                let needs_login = needs_oauth_login(m);
                let name = if is_available && !needs_login {
                    format!("{display}: {}", m.display_name())
                } else if needs_login {
                    format!("{display}: {} (needs login)", m.display_name(),)
                } else {
                    format!("{display}: {} (unavailable)", m.display_name())
                };
                let mut option = acp::SessionConfigSelectOption::new(value, name);
                let levels = m.reasoning_levels();
                if !levels.is_empty() {
                    let meta = SelectOptionMeta {
                        reasoning_levels: levels.to_vec(),
                    };
                    option = option.meta(meta.into_meta());
                }
                if is_available && !needs_login {
                    options.push(option);
                } else {
                    options.push(option.description(unavailable_reason(m)));
                }
            }
        }
    }

    let meta = ConfigOptionMeta { multi_select: true };

    SessionConfigOption::select(
        ConfigOptionId::Model.as_str(),
        "Model",
        current_model.to_string(),
        options,
    )
    .category(SessionConfigOptionCategory::Model)
    .meta(meta.into_meta())
}

fn build_reasoning_effort_config_option(
    current_effort: Option<ReasoningEffort>,
    levels: &[ReasoningEffort],
) -> Option<SessionConfigOption> {
    if levels.is_empty() {
        return None;
    }

    let current = current_effort.map_or("none".to_string(), |e| e.as_str().to_string());

    let mut options = vec![acp::SessionConfigSelectOption::new("none", "None")];
    options.extend(levels.iter().map(|e| {
        let value = e.as_str();
        let mut label = value.to_string();
        label[..1].make_ascii_uppercase();
        acp::SessionConfigSelectOption::new(value, label)
    }));

    Some(
        SessionConfigOption::select(
            ConfigOptionId::ReasoningEffort.as_str(),
            "Reasoning Effort",
            current,
            options,
        )
        .category(SessionConfigOptionCategory::ThoughtLevel),
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ValidatedMode {
    pub(crate) name: String,
    pub(crate) model: String,
    pub(crate) reasoning_effort: Option<ReasoningEffort>,
}

pub(crate) fn validated_modes_from_specs(
    specs: &[AgentSpec],
    available: &[LlmModel],
) -> Vec<ValidatedMode> {
    specs
        .iter()
        .filter(|spec| spec.exposure.user_invocable)
        .filter_map(|spec| {
            let model = spec.model.clone();
            if !model_exists(available, &model) {
                return None;
            }

            Some(ValidatedMode {
                name: spec.name.clone(),
                model,
                reasoning_effort: spec.reasoning_effort,
            })
        })
        .collect()
}

pub(crate) fn build_mode_config_option_from_modes(
    validated_modes: &[ValidatedMode],
    selected_mode: Option<&str>,
) -> Option<SessionConfigOption> {
    if validated_modes.is_empty() {
        return None;
    }

    let options: Vec<_> = validated_modes
        .iter()
        .map(|mode| acp::SessionConfigSelectOption::new(mode.name.clone(), mode.name.clone()))
        .collect();

    let current = selected_mode
        .filter(|selected| validated_modes.iter().any(|mode| mode.name == *selected))
        .map(ToOwned::to_owned)
        .or_else(|| validated_modes.first().map(|mode| mode.name.clone()))?;

    Some(
        SessionConfigOption::select(ConfigOptionId::Mode.as_str(), "Mode", current, options)
            .category(SessionConfigOptionCategory::Mode),
    )
}

pub(crate) fn resolve_mode_from_modes(
    validated_modes: &[ValidatedMode],
    mode_name: &str,
) -> Option<(String, Option<ReasoningEffort>)> {
    validated_modes
        .iter()
        .find(|mode| mode.name == mode_name)
        .map(|mode| (mode.model.clone(), mode.reasoning_effort))
}

pub(crate) fn mode_name_for_state_from_modes(
    validated_modes: &[ValidatedMode],
    model: &str,
    reasoning_effort: Option<ReasoningEffort>,
) -> Option<String> {
    validated_modes
        .iter()
        .find(|mode| mode.model == model && mode.reasoning_effort == reasoning_effort)
        .map(|mode| mode.name.clone())
}

pub(crate) fn build_config_options_from_modes(
    validated_modes: &[ValidatedMode],
    available: &[LlmModel],
    selected_mode: Option<&str>,
    current_model: &str,
    reasoning_effort: Option<ReasoningEffort>,
    all_models: &[LlmModel],
) -> Vec<SessionConfigOption> {
    let mut options = Vec::new();

    if let Some(mode_option) = build_mode_config_option_from_modes(validated_modes, selected_mode) {
        options.push(mode_option);
    }

    options.push(build_model_config_option(
        available,
        current_model,
        all_models,
    ));

    let levels = intersect_reasoning_levels(current_model);

    if let Some(opt) = build_reasoning_effort_config_option(reasoning_effort, &levels) {
        options.push(opt);
    }

    options
}

/// Compute the intersection of reasoning levels across all selected models.
/// If any model doesn't support reasoning, the intersection naturally becomes empty.
fn intersect_reasoning_levels(current_model: &str) -> Vec<ReasoningEffort> {
    let mut models = current_model
        .split(',')
        .map(str::trim)
        .filter_map(|m| m.parse::<LlmModel>().ok());

    let Some(first) = models.next() else {
        return Vec::new();
    };

    let mut result: Vec<ReasoningEffort> = first.reasoning_levels().to_vec();
    for m in models {
        result.retain(|level| m.reasoning_levels().contains(level));
    }
    result
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
    use aether_core::agent_spec::{AgentSpecExposure, ToolFilter};
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

    fn test_specs_with_modes() -> Vec<AgentSpec> {
        vec![
            AgentSpec {
                name: "Planner".to_string(),
                description: "planner".to_string(),
                model: "anthropic:claude-sonnet-4-5".to_string(),
                reasoning_effort: Some(ReasoningEffort::High),
                prompts: vec![],
                mcp_config_path: None,
                exposure: AgentSpecExposure::both(),
                tools: ToolFilter::default(),
            },
            AgentSpec {
                name: "Coder".to_string(),
                description: "coder".to_string(),
                model: "deepseek:deepseek-chat".to_string(),
                reasoning_effort: None,
                prompts: vec![],
                mcp_config_path: None,
                exposure: AgentSpecExposure::both(),
                tools: ToolFilter::default(),
            },
        ]
    }

    #[test]
    fn build_mode_config_option_from_modes_has_mode_category() {
        let specs = test_specs_with_modes();
        let available_models = test_models();
        let validated_modes = validated_modes_from_specs(&specs, &available_models);

        let option = build_mode_config_option_from_modes(&validated_modes, Some("Planner"))
            .expect("mode option should exist");

        assert_eq!(option.id.0.as_ref(), "mode");
        assert_eq!(option.category, Some(SessionConfigOptionCategory::Mode));
    }

    #[test]
    fn resolve_mode_from_modes_rejects_unknown_mode() {
        let specs = test_specs_with_modes();
        let modes = validated_modes_from_specs(&specs, &test_models());

        let resolved = resolve_mode_from_modes(&modes, "Unknown");
        assert!(resolved.is_none());
    }

    #[test]
    fn mode_name_for_state_from_modes_matches_valid_tuple() {
        let specs = test_specs_with_modes();
        let available_models = test_models();
        let validated_modes = validated_modes_from_specs(&specs, &available_models);

        let selected = mode_name_for_state_from_modes(
            &validated_modes,
            "anthropic:claude-sonnet-4-5",
            Some(ReasoningEffort::High),
        );

        assert_eq!(selected.as_deref(), Some("Planner"));
    }

    #[test]
    fn build_config_options_from_modes_includes_mode_option_when_configured() {
        let specs = test_specs_with_modes();
        let available_models = test_models();
        let validated_modes = validated_modes_from_specs(&specs, &available_models);

        let options = build_config_options_from_modes(
            &validated_modes,
            &available_models,
            Some("Planner"),
            "anthropic:claude-sonnet-4-5",
            Some(ReasoningEffort::High),
            LlmModel::all(),
        );

        let mode_option = options
            .iter()
            .find(|option| option.id.0.as_ref() == "mode")
            .expect("mode option should exist");
        assert_eq!(mode_option.id.0.as_ref(), "mode");
    }

    #[test]
    fn build_config_options_from_modes_returns_single_model_option() {
        let models = test_models();
        let opts = build_config_options_from_modes(
            &[],
            &models,
            None,
            "deepseek:deepseek-chat",
            None,
            LlmModel::all(),
        );

        assert_eq!(opts.len(), 1);

        let model_option = opts
            .iter()
            .find(|option| option.id.0.as_ref() == "model")
            .expect("model option should exist");

        let SessionConfigKind::Select(ref model_select) = model_option.kind else {
            panic!("Expected Select kind");
        };
        assert_eq!(
            model_select.current_value.0.as_ref(),
            "deepseek:deepseek-chat"
        );

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
        let opt =
            build_model_config_option(&models, "anthropic:claude-sonnet-4-5", LlmModel::all());
        let meta = ConfigOptionMeta::from_meta(opt.meta.as_ref());
        assert!(meta.multi_select);
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
        let opt =
            build_model_config_option(&models, "anthropic:claude-sonnet-4-5", LlmModel::all());

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
    fn build_config_options_from_modes_includes_reasoning_for_reasoning_model() {
        let available_models = test_models();
        let specs = test_specs_with_modes();
        let validated_modes = validated_modes_from_specs(&specs, &available_models);
        let opts = build_config_options_from_modes(
            &validated_modes,
            &available_models,
            None,
            "anthropic:claude-opus-4-6",
            Some(ReasoningEffort::High),
            LlmModel::all(),
        );

        assert!(opts.len() >= 2, "Expected model + reasoning options");
        let reasoning_opt = opts.iter().find(|o| o.id.0.as_ref() == "reasoning_effort");
        assert!(
            reasoning_opt.is_some(),
            "Expected reasoning_effort option for reasoning model"
        );
        let reasoning_opt = reasoning_opt.unwrap();
        let SessionConfigKind::Select(ref select) = reasoning_opt.kind else {
            panic!("Expected Select kind");
        };
        assert_eq!(select.current_value.0.as_ref(), "high");
    }

    #[test]
    fn build_config_options_from_modes_hides_reasoning_for_non_reasoning_model() {
        let available_models = test_models();
        let specs = test_specs_with_modes();
        let validated_modes = validated_modes_from_specs(&specs, &available_models);
        let opts = build_config_options_from_modes(
            &validated_modes,
            &available_models,
            None,
            "deepseek:deepseek-chat",
            None,
            LlmModel::all(),
        );
        assert!(
            !opts.iter().any(|o| o.id.0.as_ref() == "reasoning_effort"),
            "Non-reasoning model should not have reasoning_effort option"
        );
    }

    #[test]
    fn reasoning_option_removed_when_switching_to_non_reasoning_model() {
        let available_models = test_models();
        let specs = test_specs_with_modes();
        let validated_modes = validated_modes_from_specs(&specs, &available_models);

        let opts_with = build_config_options_from_modes(
            &validated_modes,
            &available_models,
            None,
            "anthropic:claude-opus-4-6",
            Some(ReasoningEffort::High),
            LlmModel::all(),
        );
        assert!(
            opts_with
                .iter()
                .any(|o| o.id.0.as_ref() == "reasoning_effort"),
            "reasoning_effort should be present for claude-opus-4-6"
        );

        let opts_without = build_config_options_from_modes(
            &validated_modes,
            &available_models,
            None,
            "deepseek:deepseek-chat",
            None,
            LlmModel::all(),
        );
        assert!(
            !opts_without
                .iter()
                .any(|o| o.id.0.as_ref() == "reasoning_effort"),
            "reasoning_effort should NOT be present for deepseek-chat"
        );
    }

    #[test]
    fn mixed_provider_lists_models_individually() {
        // test_models() has Gemini25Pro available, so Gemini is "mixed"
        let models = test_models();
        let opt =
            build_model_config_option(&models, "anthropic:claude-sonnet-4-5", LlmModel::all());

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
