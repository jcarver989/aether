use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;
use std::path::Path;

type ModelsDevData = HashMap<String, ProviderData>;

#[derive(Debug, Deserialize)]
struct ProviderData {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    name: String,
    #[serde(default)]
    #[allow(dead_code)]
    env: Vec<String>,
    #[serde(default)]
    models: HashMap<String, ModelData>,
}

#[derive(Debug, Deserialize)]
struct ModelData {
    id: String,
    name: String,
    #[serde(default)]
    tool_call: Option<bool>,
    #[serde(default)]
    #[allow(dead_code)]
    reasoning: Option<bool>,
    #[serde(default)]
    #[allow(dead_code)]
    cost: Option<CostData>,
    #[serde(default)]
    limit: Option<LimitData>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CostData {
    #[serde(default)]
    input: f64,
    #[serde(default)]
    output: f64,
}

#[derive(Debug, Deserialize)]
struct LimitData {
    #[serde(default)]
    context: u32,
    #[serde(default)]
    #[allow(dead_code)]
    output: u32,
}

/// Provider configuration for codegen (catalog providers with known model lists)
struct ProviderConfig {
    /// models.dev provider ID (e.g. "google")
    dev_id: &'static str,
    /// Our Rust enum name (e.g. "Gemini")
    enum_name: &'static str,
    /// Our internal provider name used for parsing (e.g. "gemini")
    parser_name: &'static str,
    /// Env var our code actually checks (None for providers with complex credential chains)
    env_var: Option<&'static str>,
}

/// Dynamic provider — model name is user-supplied at runtime, no fixed enum
struct DynamicProviderConfig {
    /// Rust variant name in `LlmModel` (e.g. "Ollama")
    enum_name: &'static str,
    /// Parser name used in "provider:model" strings (e.g. "ollama")
    parser_name: &'static str,
}

const PROVIDERS: &[ProviderConfig] = &[
    ProviderConfig {
        dev_id: "anthropic",
        enum_name: "Anthropic",
        parser_name: "anthropic",
        env_var: Some("ANTHROPIC_API_KEY"),
    },
    ProviderConfig {
        dev_id: "deepseek",
        enum_name: "DeepSeek",
        parser_name: "deepseek",
        env_var: Some("DEEPSEEK_API_KEY"),
    },
    ProviderConfig {
        dev_id: "google",
        enum_name: "Gemini",
        parser_name: "gemini",
        env_var: Some("GEMINI_API_KEY"),
    },
    ProviderConfig {
        dev_id: "moonshotai",
        enum_name: "Moonshot",
        parser_name: "moonshot",
        env_var: Some("MOONSHOT_API_KEY"),
    },
    ProviderConfig {
        dev_id: "openrouter",
        enum_name: "OpenRouter",
        parser_name: "openrouter",
        env_var: Some("OPENROUTER_API_KEY"),
    },
    ProviderConfig {
        dev_id: "zai",
        enum_name: "ZAi",
        parser_name: "zai",
        env_var: Some("ZAI_API_KEY"),
    },
    ProviderConfig {
        dev_id: "amazon-bedrock",
        enum_name: "Bedrock",
        parser_name: "bedrock",
        env_var: None,
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

    let provider_models = build_provider_models(&data)?;
    let ctx = CodegenCtx { provider_models };
    Ok(emit_generated_source(&ctx))
}

fn build_provider_models(data: &ModelsDevData) -> Result<ProviderModels, String> {
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

/// Convert a model ID like "claude-sonnet-4-5-20250929" into a `PascalCase` variant name.
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

fn emit_generated_source(ctx: &CodegenCtx) -> String {
    let mut out = String::with_capacity(64_000);
    emit_header(&mut out);
    emit_provider_enums(&mut out, &ctx.provider_models);
    emit_llm_model_enum(&mut out);
    emit_from_impls(&mut out);
    emit_llm_model_impl(&mut out, &ctx.provider_models);
    emit_display_impl(&mut out);
    emit_fromstr_impl(&mut out, &ctx.provider_models);
    out
}

fn emit_header(out: &mut String) {
    pushln(
        out,
        "// Auto-generated from models.dev — do not edit manually",
    );
    pushln(out, "// Regenerated automatically by build.rs");
    blank(out);
    pushln(out, "use std::borrow::Cow;");
    pushln(out, "use std::sync::LazyLock;");
    blank(out);
}

fn emit_provider_enums(out: &mut String, provider_models: &ProviderModels) {
    for cfg in PROVIDERS {
        pushln(out, "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]");
        pushln(out, format!("pub enum {}Model {{", cfg.enum_name));
        for model in &provider_models[cfg.dev_id] {
            pushln(out, format!("    {},", model.variant_name));
        }
        pushln(out, "}");
        blank(out);
    }
}

fn emit_llm_model_enum(out: &mut String) {
    pushln(out, "/// A model from a specific provider");
    pushln(out, "#[derive(Debug, Clone, PartialEq, Eq, Hash)]");
    pushln(out, "pub enum LlmModel {");
    for cfg in PROVIDERS {
        pushln(
            out,
            format!("    {provider}({provider}Model),", provider = cfg.enum_name),
        );
    }
    for dyn_cfg in DYNAMIC_PROVIDERS {
        pushln(out, format!("    {}(String),", dyn_cfg.enum_name));
    }
    pushln(out, "}");
    blank(out);
}

fn emit_from_impls(out: &mut String) {
    for cfg in PROVIDERS {
        pushln(
            out,
            format!("impl From<{}Model> for LlmModel {{", cfg.enum_name),
        );
        pushln(
            out,
            format!(
                "    fn from(m: {}Model) -> Self {{ LlmModel::{}(m) }}",
                cfg.enum_name, cfg.enum_name
            ),
        );
        pushln(out, "}");
        blank(out);
    }
}

fn emit_llm_model_impl(out: &mut String, provider_models: &ProviderModels) {
    pushln(out, "impl LlmModel {");
    emit_model_id_method(out, provider_models);
    emit_display_name_method(out, provider_models);
    emit_provider_method(out);
    emit_context_window_method(out, provider_models);
    emit_required_env_var_method(out);
    emit_all_method(out, provider_models);
    pushln(out, "}");
    blank(out);
}

fn emit_model_id_method(out: &mut String, provider_models: &ProviderModels) {
    pushln(
        out,
        "    /// Raw model ID (e.g. \"claude-opus-4-6\", \"llama3.2\")",
    );
    pushln(out, "    pub fn model_id(&self) -> Cow<'static, str> {");
    pushln(out, "        match self {");
    emit_static_model_arms(out, provider_models, |cfg, model| {
        format!(
            "            LlmModel::{}({}Model::{}) => Cow::Borrowed(\"{}\"),",
            cfg.enum_name, cfg.enum_name, model.variant_name, model.model_id
        )
    });
    emit_dynamic_provider_arms(out, |dyn_cfg| {
        format!(
            "            LlmModel::{}(s) => Cow::Owned(s.clone()),",
            dyn_cfg.enum_name
        )
    });
    pushln(out, "        }");
    pushln(out, "    }");
    blank(out);
}

fn emit_display_name_method(out: &mut String, provider_models: &ProviderModels) {
    pushln(
        out,
        "    /// Human-readable display name (e.g. \"Claude Opus 4.6\")",
    );
    pushln(out, "    pub fn display_name(&self) -> Cow<'static, str> {");
    pushln(out, "        match self {");
    emit_static_model_arms(out, provider_models, |cfg, model| {
        let escaped = escape_rust_string(&model.display_name);
        format!(
            "            LlmModel::{}({}Model::{}) => Cow::Borrowed(\"{}\"),",
            cfg.enum_name, cfg.enum_name, model.variant_name, escaped
        )
    });
    emit_dynamic_provider_arms(out, |dyn_cfg| {
        format!(
            "            LlmModel::{name}(s) => Cow::Owned(format!(\"{name} {{}}\", s)),",
            name = dyn_cfg.enum_name
        )
    });
    pushln(out, "        }");
    pushln(out, "    }");
    blank(out);
}

fn emit_provider_method(out: &mut String) {
    pushln(out, "    /// Provider identifier (e.g. \"anthropic\")");
    pushln(out, "    pub fn provider(&self) -> &'static str {");
    pushln(out, "        match self {");
    emit_provider_arms(
        out,
        |cfg| {
            format!(
                "            LlmModel::{}(_) => \"{}\",",
                cfg.enum_name, cfg.parser_name
            )
        },
        |dyn_cfg| {
            format!(
                "            LlmModel::{}(_) => \"{}\",",
                dyn_cfg.enum_name, dyn_cfg.parser_name
            )
        },
    );
    pushln(out, "        }");
    pushln(out, "    }");
    blank(out);
}

fn emit_context_window_method(out: &mut String, provider_models: &ProviderModels) {
    pushln(
        out,
        "    /// Context window size in tokens (None for dynamic providers)",
    );
    pushln(out, "    pub fn context_window(&self) -> Option<u32> {");
    pushln(out, "        match self {");
    emit_static_model_arms(out, provider_models, |cfg, model| {
        format!(
            "            LlmModel::{}({}Model::{}) => Some({}),",
            cfg.enum_name, cfg.enum_name, model.variant_name, model.context_window
        )
    });
    emit_dynamic_provider_arms(out, |dyn_cfg| {
        format!("            LlmModel::{}(_) => None,", dyn_cfg.enum_name)
    });
    pushln(out, "        }");
    pushln(out, "    }");
    blank(out);
}

fn emit_required_env_var_method(out: &mut String) {
    pushln(
        out,
        "    /// Required env var for this model's provider (None for local providers)",
    );
    pushln(
        out,
        "    pub fn required_env_var(&self) -> Option<&'static str> {",
    );
    pushln(out, "        match self {");
    emit_provider_arms(
        out,
        |cfg| match cfg.env_var {
            Some(var) => format!(
                "            LlmModel::{}(_) => Some(\"{}\"),",
                cfg.enum_name, var
            ),
            None => format!("            LlmModel::{}(_) => None,", cfg.enum_name),
        },
        |dyn_cfg| format!("            LlmModel::{}(_) => None,", dyn_cfg.enum_name),
    );
    pushln(out, "        }");
    pushln(out, "    }");
    blank(out);
}

fn emit_all_method(out: &mut String, provider_models: &ProviderModels) {
    pushln(
        out,
        "    /// All catalog models (excludes dynamic providers)",
    );
    pushln(out, "    pub fn all() -> &'static [LlmModel] {");
    pushln(
        out,
        "        static ALL: LazyLock<Vec<LlmModel>> = LazyLock::new(|| vec![",
    );
    emit_static_model_arms(out, provider_models, |cfg, model| {
        format!(
            "            LlmModel::{}({}Model::{}),",
            cfg.enum_name, cfg.enum_name, model.variant_name
        )
    });
    pushln(out, "        ]);");
    pushln(out, "        &ALL");
    pushln(out, "    }");
}

fn emit_display_impl(out: &mut String) {
    pushln(out, "impl std::fmt::Display for LlmModel {");
    pushln(
        out,
        "    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {",
    );
    pushln(out, "        f.write_str(&self.display_name())");
    pushln(out, "    }");
    pushln(out, "}");
    blank(out);
}

fn emit_fromstr_impl(out: &mut String, provider_models: &ProviderModels) {
    pushln(out, "impl std::str::FromStr for LlmModel {");
    pushln(out, "    type Err = String;");
    blank(out);
    pushln(
        out,
        "    /// Parse a \"provider:model\" string into an LlmModel",
    );
    pushln(out, "    fn from_str(s: &str) -> Result<Self, Self::Err> {");
    pushln(
        out,
        "        let (provider_str, model_str) = s.split_once(':').unwrap_or((s, \"\"));",
    );
    pushln(out, "        match provider_str {");
    for cfg in PROVIDERS {
        emit_fromstr_provider_block(out, cfg, &provider_models[cfg.dev_id]);
    }
    emit_dynamic_provider_arms(out, |dyn_cfg| {
        format!(
            "            \"{}\" => Ok(LlmModel::{}(model_str.to_string())),",
            dyn_cfg.parser_name, dyn_cfg.enum_name
        )
    });
    pushln(
        out,
        "            _ => Err(format!(\"Unknown provider: '{}'\", provider_str)),",
    );
    pushln(out, "        }");
    pushln(out, "    }");
    pushln(out, "}");
}

fn emit_fromstr_provider_block(out: &mut String, cfg: &ProviderConfig, models: &[ModelInfo]) {
    pushln(out, format!("            \"{}\" => {{", cfg.parser_name));
    pushln(out, "                match model_str {");
    for model in models {
        pushln(
            out,
            format!(
                "                    \"{}\" => Ok(LlmModel::{}({}Model::{})),",
                model.model_id, cfg.enum_name, cfg.enum_name, model.variant_name
            ),
        );
    }
    pushln(
        out,
        format!(
            "                    _ => Err(format!(\"Unknown {} model: '{{}}'\", model_str)),",
            cfg.parser_name
        ),
    );
    pushln(out, "                }");
    pushln(out, "            }");
}

fn emit_static_model_arms<F>(out: &mut String, provider_models: &ProviderModels, mut arm_for: F)
where
    F: FnMut(&ProviderConfig, &ModelInfo) -> String,
{
    for cfg in PROVIDERS {
        for model in &provider_models[cfg.dev_id] {
            pushln(out, arm_for(cfg, model));
        }
    }
}

fn emit_dynamic_provider_arms<F>(out: &mut String, mut arm_for: F)
where
    F: FnMut(&DynamicProviderConfig) -> String,
{
    for dyn_cfg in DYNAMIC_PROVIDERS {
        pushln(out, arm_for(dyn_cfg));
    }
}

fn emit_provider_arms<F, G>(out: &mut String, mut provider_arm: F, mut dynamic_arm: G)
where
    F: FnMut(&ProviderConfig) -> String,
    G: FnMut(&DynamicProviderConfig) -> String,
{
    for cfg in PROVIDERS {
        pushln(out, provider_arm(cfg));
    }
    for dyn_cfg in DYNAMIC_PROVIDERS {
        pushln(out, dynamic_arm(dyn_cfg));
    }
}

fn escape_rust_string(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('"', "\\\"")
}

fn pushln(out: &mut String, line: impl AsRef<str>) {
    writeln!(out, "{}", line.as_ref()).expect("writing to String should not fail");
}

fn blank(out: &mut String) {
    pushln(out, "");
}

// ── Tests ────────────────────────────────────────────────────────────────

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
