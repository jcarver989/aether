use super::raw::ModelsDevData;
use std::collections::BTreeMap;
use std::path::Path;

mod emit;
mod model_index;

/// Provider configuration for codegen (catalog providers with known model lists)
struct ProviderConfig {
    /// models.dev provider ID (e.g. "google")
    dev_id: &'static str,
    /// Our Rust enum name (e.g. "Gemini")
    enum_name: &'static str,
    /// Our internal provider name used for parsing (e.g. "gemini")
    parser_name: &'static str,
    /// Env var our code actually checks
    env_var: &'static str,
}

/// Dynamic provider — model name is user-supplied at runtime, no fixed enum
struct DynamicProviderConfig {
    /// Rust variant name in LlmModel (e.g. "Ollama")
    enum_name: &'static str,
    /// Parser name used in "provider:model" strings (e.g. "ollama")
    parser_name: &'static str,
}

const PROVIDERS: &[ProviderConfig] = &[
    ProviderConfig {
        dev_id: "anthropic",
        enum_name: "Anthropic",
        parser_name: "anthropic",
        env_var: "ANTHROPIC_API_KEY",
    },
    ProviderConfig {
        dev_id: "deepseek",
        enum_name: "DeepSeek",
        parser_name: "deepseek",
        env_var: "DEEPSEEK_API_KEY",
    },
    ProviderConfig {
        dev_id: "google",
        enum_name: "Gemini",
        parser_name: "gemini",
        env_var: "GEMINI_API_KEY",
    },
    ProviderConfig {
        dev_id: "moonshotai",
        enum_name: "Moonshot",
        parser_name: "moonshot",
        env_var: "MOONSHOT_API_KEY",
    },
    ProviderConfig {
        dev_id: "openrouter",
        enum_name: "OpenRouter",
        parser_name: "openrouter",
        env_var: "OPENROUTER_API_KEY",
    },
    ProviderConfig {
        dev_id: "zai",
        enum_name: "ZAi",
        parser_name: "zai",
        env_var: "ZAI_API_KEY",
    },
];

const DYNAMIC_PROVIDERS: &[DynamicProviderConfig] = &[
    DynamicProviderConfig {
        enum_name: "Ollama",
        parser_name: "ollama",
    },
    DynamicProviderConfig {
        enum_name: "LlamaCpp",
        parser_name: "llamacpp",
    },
];

#[derive(Debug, Clone)]
struct ModelInfo {
    variant_name: String,
    model_id: String,
    display_name: String,
    context_window: u32,
}

type ProviderModels = BTreeMap<&'static str, Vec<ModelInfo>>;

struct CodegenCtx {
    provider_models: ProviderModels,
}

/// Run the codegen, returning the generated Rust source.
pub fn generate(models_json_path: &Path) -> Result<String, String> {
    let json_bytes = std::fs::read_to_string(models_json_path).map_err(|e| format!("read: {e}"))?;
    let data: ModelsDevData =
        serde_json::from_str(&json_bytes).map_err(|e| format!("parse: {e}"))?;

    let provider_models = model_index::build_provider_models(&data)?;
    let ctx = CodegenCtx { provider_models };
    Ok(emit::emit_generated_source(&ctx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use serde_json::json;
    use tempfile::NamedTempFile;

    #[test]
    fn generate_sorts_and_filters_models() {
        let mut data = minimal_models_dev_json();
        let root = data.as_object_mut().expect("root object");
        let anthropic = root
            .get_mut("anthropic")
            .and_then(Value::as_object_mut)
            .expect("anthropic provider");

        anthropic.insert(
            "models".to_string(),
            json!({
                "b-model": {
                    "id": "b-model",
                    "name": "B Model",
                    "tool_call": true,
                    "limit": {"context": 2000, "output": 0}
                },
                "a-model": {
                    "id": "a-model",
                    "name": "A Model",
                    "tool_call": true,
                    "limit": {"context": 1000, "output": 0}
                },
                "alpha-latest": {
                    "id": "alpha-latest",
                    "name": "Alias",
                    "tool_call": true,
                    "limit": {"context": 500, "output": 0}
                },
                "no-tools": {
                    "id": "no-tools",
                    "name": "No Tools",
                    "tool_call": false,
                    "limit": {"context": 500, "output": 0}
                }
            }),
        );

        let source = generate_from_value(data);
        let a_model = "\"a-model\" => Ok(LlmModel::Anthropic(AnthropicModel::AModel)),";
        let b_model = "\"b-model\" => Ok(LlmModel::Anthropic(AnthropicModel::BModel)),";
        let a_pos = source.find(a_model).expect("a-model parse arm");
        let b_pos = source.find(b_model).expect("b-model parse arm");
        assert!(a_pos < b_pos);
        assert!(!source.contains("AnthropicModel::AlphaLatest"));
        assert!(!source.contains("AnthropicModel::NoTools"));
    }

    #[test]
    fn generate_contains_core_sections() {
        let source = generate_from_value(minimal_models_dev_json());
        assert!(source.contains("pub enum LlmModel {"));
        assert!(source.contains("impl std::str::FromStr for LlmModel {"));
        assert!(source.contains("impl std::fmt::Display for LlmModel {"));
        assert!(source.contains("pub fn required_env_var(&self) -> Option<&'static str> {"));
    }

    #[test]
    fn generate_contains_dynamic_provider_arms() {
        let source = generate_from_value(minimal_models_dev_json());
        assert!(source.contains("\"ollama\" => Ok(LlmModel::Ollama(model_str.to_string())),"));
        assert!(source.contains("\"llamacpp\" => Ok(LlmModel::LlamaCpp(model_str.to_string())),"));
        assert!(source.contains("LlmModel::Ollama(_) => None,"));
        assert!(source.contains("LlmModel::LlamaCpp(_) => None,"));
    }

    fn generate_from_value(data: Value) -> String {
        let tmp = NamedTempFile::new().expect("temp file");
        let json = serde_json::to_string(&data).expect("serialize fixture");
        std::fs::write(tmp.path(), json).expect("write fixture");
        generate(tmp.path()).expect("codegen succeeds")
    }

    fn minimal_models_dev_json() -> Value {
        let mut root = serde_json::Map::new();
        for provider_id in PROVIDERS.iter().map(|p| p.dev_id) {
            root.insert(
                provider_id.to_string(),
                json!({
                    "id": provider_id,
                    "name": provider_id,
                    "env": [],
                    "models": {}
                }),
            );
        }
        Value::Object(root)
    }
}
