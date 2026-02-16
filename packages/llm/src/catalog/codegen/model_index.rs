use super::super::raw::ModelsDevData;
use super::{ModelInfo, PROVIDERS, ProviderModels};

pub(super) fn build_provider_models(data: &ModelsDevData) -> Result<ProviderModels, String> {
    let mut provider_models = ProviderModels::new();

    for cfg in PROVIDERS {
        let provider_data = data
            .get(cfg.dev_id)
            .ok_or_else(|| format!("Provider '{}' not found in models.dev data", cfg.dev_id))?;

        let mut models: Vec<ModelInfo> = provider_data
            .models
            .values()
            .filter(|m| m.tool_call == Some(true))
            .filter(|m| !is_alias(&m.id))
            .map(|m| ModelInfo {
                variant_name: model_id_to_variant(&m.id),
                model_id: m.id.clone(),
                display_name: m.name.clone(),
                context_window: m.limit.as_ref().map_or(0, |l| l.context),
            })
            .collect();

        models.sort_by(|a, b| a.model_id.cmp(&b.model_id));
        provider_models.insert(cfg.dev_id, models);
    }

    Ok(provider_models)
}

/// Returns true for "latest" alias IDs that just point to another model
fn is_alias(id: &str) -> bool {
    id.ends_with("-latest")
}

/// Convert a model ID like "claude-sonnet-4-5-20250929" into a PascalCase variant name.
/// Treats `-`, `.`, `/`, and `:` as word separators.
fn model_id_to_variant(id: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;

    for ch in id.chars() {
        if ch == '-' || ch == '.' || ch == '/' || ch == ':' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }

    // If the variant starts with a digit, prefix with underscore
    if result.starts_with(|c: char| c.is_ascii_digit()) {
        result.insert(0, '_');
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_id_to_variant() {
        assert_eq!(
            model_id_to_variant("claude-sonnet-4-5-20250929"),
            "ClaudeSonnet4520250929"
        );
        assert_eq!(model_id_to_variant("gemini-2.5-flash"), "Gemini25Flash");
        assert_eq!(model_id_to_variant("deepseek-chat"), "DeepseekChat");
        assert_eq!(model_id_to_variant("glm-4.5"), "Glm45");
    }

    #[test]
    fn test_model_id_to_variant_with_slash_and_colon() {
        assert_eq!(
            model_id_to_variant("anthropic/claude-opus-4.6"),
            "AnthropicClaudeOpus46"
        );
        assert_eq!(
            model_id_to_variant("openai/gpt-5.1-codex-max"),
            "OpenaiGpt51CodexMax"
        );
        assert_eq!(
            model_id_to_variant("deepseek/deepseek-r1:free"),
            "DeepseekDeepseekR1Free"
        );
    }

    #[test]
    fn test_is_alias() {
        assert!(is_alias("claude-sonnet-4-5-latest"));
        assert!(is_alias("claude-3-7-sonnet-latest"));
        assert!(!is_alias("claude-sonnet-4-5-20250929"));
    }
}
