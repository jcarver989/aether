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
    /// Unique provider key used in `provider_models` map (e.g. "codex")
    dev_id: &'static str,
    /// models.dev provider ID to read models from (defaults to `dev_id` when `None`)
    source_dev_id: Option<&'static str>,
    /// Only include models whose ID passes this filter (None = include all)
    model_filter: Option<fn(&str) -> bool>,
    /// Our Rust enum name (e.g. "Gemini")
    enum_name: &'static str,
    /// Our internal provider name used for parsing (e.g. "gemini")
    parser_name: &'static str,
    /// Human-readable provider name (e.g. "AWS Bedrock")
    display_name: &'static str,
    /// Env var our code actually checks (None for providers with complex credential chains)
    env_var: Option<&'static str>,
    /// OAuth provider ID for providers that require OAuth login (e.g. "codex")
    oauth_provider_id: Option<&'static str>,
}

impl ProviderConfig {
    /// Shorthand for providers with default `source_dev_id`, `model_filter`, and `oauth_provider_id`.
    const fn standard(
        dev_id: &'static str,
        enum_name: &'static str,
        parser_name: &'static str,
        display_name: &'static str,
        env_var: Option<&'static str>,
    ) -> Self {
        Self {
            dev_id,
            source_dev_id: None,
            model_filter: None,
            enum_name,
            parser_name,
            display_name,
            env_var,
            oauth_provider_id: None,
        }
    }

    /// The models.dev key to look up in the JSON data.
    fn json_key(&self) -> &'static str {
        self.source_dev_id.unwrap_or(self.dev_id)
    }
}

/// Dynamic provider — model name is user-supplied at runtime, no fixed enum
#[allow(clippy::struct_field_names)]
struct DynamicProviderConfig {
    /// Rust variant name in `LlmModel` (e.g. "Ollama")
    enum_name: &'static str,
    /// Parser name used in "provider:model" strings (e.g. "ollama")
    parser_name: &'static str,
    /// Human-readable provider name (e.g. "Ollama")
    display_name: &'static str,
}

const PROVIDERS: &[ProviderConfig] = &[
    ProviderConfig::standard(
        "anthropic",
        "Anthropic",
        "anthropic",
        "Anthropic",
        Some("ANTHROPIC_API_KEY"),
    ),
    ProviderConfig {
        dev_id: "codex",
        source_dev_id: Some("openai"),
        model_filter: Some(|id| id.contains("codex")),
        enum_name: "Codex",
        parser_name: "codex",
        display_name: "Codex",
        env_var: None,
        oauth_provider_id: Some("codex"),
    },
    ProviderConfig::standard(
        "deepseek",
        "DeepSeek",
        "deepseek",
        "DeepSeek",
        Some("DEEPSEEK_API_KEY"),
    ),
    ProviderConfig::standard(
        "google",
        "Gemini",
        "gemini",
        "Gemini",
        Some("GEMINI_API_KEY"),
    ),
    ProviderConfig::standard(
        "moonshotai",
        "Moonshot",
        "moonshot",
        "Moonshot",
        Some("MOONSHOT_API_KEY"),
    ),
    ProviderConfig::standard(
        "openrouter",
        "OpenRouter",
        "openrouter",
        "OpenRouter",
        Some("OPENROUTER_API_KEY"),
    ),
    ProviderConfig::standard("zai", "ZAi", "zai", "ZAI", Some("ZAI_API_KEY")),
    ProviderConfig::standard("amazon-bedrock", "Bedrock", "bedrock", "AWS Bedrock", None),
];

const DYNAMIC_PROVIDERS: &[DynamicProviderConfig] = &[
    DynamicProviderConfig {
        enum_name: "Ollama",
        parser_name: "ollama",
        display_name: "Ollama",
    },
    DynamicProviderConfig {
        enum_name: "LlamaCpp",
        parser_name: "llamacpp",
        display_name: "LlamaCpp",
    },
];

#[derive(Debug, Clone)]
struct ModelInfo {
    variant_name: String,
    model_id: String,
    display_name: String,
    context_window: u32,
    supports_reasoning: bool,
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
        let json_key = cfg.json_key();
        let provider_data = data
            .get(json_key)
            .ok_or_else(|| format!("Provider '{json_key}' not found in models.dev data"))?;

        let mut models: Vec<ModelInfo> = provider_data
            .models
            .values()
            .filter(|m| m.tool_call == Some(true))
            .filter(|m| !is_alias(&m.id))
            .filter(|m| cfg.model_filter.is_none_or(|f| f(&m.id)))
            .map(|m| ModelInfo {
                variant_name: model_id_to_variant(&m.id),
                model_id: m.id.clone(),
                display_name: m.name.clone(),
                context_window: m.limit.as_ref().map_or(0, |l| l.context),
                supports_reasoning: m.reasoning.unwrap_or(false),
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
    emit_provider_impls(&mut out, &ctx.provider_models);
    emit_llm_model_enum(&mut out);
    emit_from_impls(&mut out);
    emit_llm_model_impl(&mut out);
    emit_display_impl(&mut out);
    emit_fromstr_impl(&mut out);
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

fn emit_provider_impls(out: &mut String, provider_models: &ProviderModels) {
    for cfg in PROVIDERS {
        let models = &provider_models[cfg.dev_id];
        let enum_name = format!("{}Model", cfg.enum_name);

        pushln(out, format!("impl {enum_name} {{"));

        // model_id — each model has a unique ID, no grouping needed
        pushln(out, "    #[allow(clippy::too_many_lines)]");
        pushln(out, "    fn model_id(self) -> &'static str {");
        pushln(out, "        match self {");
        for model in models {
            pushln(
                out,
                format!(
                    "            Self::{} => \"{}\",",
                    model.variant_name, model.model_id
                ),
            );
        }
        pushln(out, "        }");
        pushln(out, "    }");
        blank(out);

        // display_name — group variants that share the same name
        pushln(out, "    #[allow(clippy::too_many_lines)]");
        pushln(out, "    fn display_name(self) -> &'static str {");
        pushln(out, "        match self {");
        emit_grouped_arms(
            out,
            models,
            |m| escape_rust_string(&m.display_name),
            |name| format!("\"{name}\""),
        );
        pushln(out, "        }");
        pushln(out, "    }");
        blank(out);

        // context_window — group variants that share the same value
        pushln(out, "    fn context_window(self) -> u32 {");
        pushln(out, "        match self {");
        emit_grouped_arms(
            out,
            models,
            |m| m.context_window.to_string(),
            |val| format_number(val.parse::<u32>().unwrap()),
        );
        pushln(out, "        }");
        pushln(out, "    }");
        blank(out);

        // supports_reasoning — group variants that share the same value
        pushln(out, "    pub fn supports_reasoning(self) -> bool {");
        pushln(out, "        match self {");
        emit_grouped_arms(
            out,
            models,
            |m| m.supports_reasoning.to_string(),
            std::string::ToString::to_string,
        );
        pushln(out, "        }");
        pushln(out, "    }");
        blank(out);

        // ALL constant
        pushln(out, format!("    const ALL: &[{enum_name}] = &["));
        for model in models {
            pushln(out, format!("        Self::{},", model.variant_name));
        }
        pushln(out, "    ];");

        pushln(out, "}");
        blank(out);

        // FromStr for provider model
        pushln(out, format!("impl std::str::FromStr for {enum_name} {{"));
        pushln(out, "    type Err = String;");
        blank(out);
        pushln(out, "    #[allow(clippy::too_many_lines)]");
        pushln(out, "    fn from_str(s: &str) -> Result<Self, Self::Err> {");
        pushln(out, "        match s {");
        for model in models {
            pushln(
                out,
                format!(
                    "            \"{}\" => Ok(Self::{}),",
                    model.model_id, model.variant_name
                ),
            );
        }
        pushln(
            out,
            format!(
                "            _ => Err(format!(\"Unknown {} model: '{{s}}'\")),",
                cfg.parser_name
            ),
        );
        pushln(out, "        }");
        pushln(out, "    }");
        pushln(out, "}");
        blank(out);
    }
}

/// Emit match arms grouped by value to avoid clippy `match_same_arms`.
///
/// `key_fn` extracts a grouping key from each model (e.g. `context_window` as string).
/// `fmt_val` formats the key into the match arm's RHS.
fn emit_grouped_arms(
    out: &mut String,
    models: &[ModelInfo],
    key_fn: impl Fn(&ModelInfo) -> String,
    fmt_val: impl Fn(&str) -> String,
) {
    // Group variants by value, preserving insertion order via BTreeMap
    let mut groups: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for model in models {
        groups
            .entry(key_fn(model))
            .or_default()
            .push(&model.variant_name);
    }

    for (key, variants) in &groups {
        let rhs = fmt_val(key);
        if variants.len() == 1 {
            pushln(out, format!("            Self::{} => {rhs},", variants[0]));
        } else {
            let patterns: Vec<String> = variants.iter().map(|v| format!("Self::{v}")).collect();
            pushln(
                out,
                format!("            {} => {rhs},", patterns.join(" | ")),
            );
        }
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

fn emit_llm_model_impl(out: &mut String) {
    pushln(out, "impl LlmModel {");
    emit_llm_model_id(out);
    emit_llm_display_name(out);
    emit_llm_provider(out);
    emit_llm_provider_display_name(out);
    emit_llm_context_window(out);
    emit_llm_required_env_var(out);
    emit_llm_oauth_provider_id(out);
    emit_llm_supports_reasoning(out);
    emit_llm_all(out);
    pushln(out, "}");
    blank(out);
}

fn emit_llm_model_id(out: &mut String) {
    pushln(
        out,
        "    /// Raw model ID (e.g. `claude-opus-4-6`, `llama3.2`)",
    );
    pushln(out, "    pub fn model_id(&self) -> Cow<'static, str> {");
    pushln(out, "        match self {");
    for cfg in PROVIDERS {
        pushln(
            out,
            format!(
                "            Self::{}(m) => Cow::Borrowed(m.model_id()),",
                cfg.enum_name
            ),
        );
    }
    pushln(
        out,
        format!(
            "            {} => Cow::Owned(s.clone()),",
            dynamic_pattern_with_binding("s")
        ),
    );
    pushln(out, "        }");
    pushln(out, "    }");
    blank(out);
}

fn emit_llm_display_name(out: &mut String) {
    pushln(
        out,
        "    /// Human-readable display name (e.g. `Claude Opus 4.6`)",
    );
    pushln(out, "    pub fn display_name(&self) -> Cow<'static, str> {");
    pushln(out, "        match self {");
    for cfg in PROVIDERS {
        pushln(
            out,
            format!(
                "            Self::{}(m) => Cow::Borrowed(m.display_name()),",
                cfg.enum_name
            ),
        );
    }
    for dyn_cfg in DYNAMIC_PROVIDERS {
        pushln(
            out,
            format!(
                "            Self::{}(s) => Cow::Owned(format!(\"{} {{s}}\")),",
                dyn_cfg.enum_name, dyn_cfg.enum_name
            ),
        );
    }
    pushln(out, "        }");
    pushln(out, "    }");
    blank(out);
}

fn emit_llm_provider(out: &mut String) {
    pushln(out, "    /// Provider identifier (e.g. `anthropic`)");
    pushln(out, "    pub fn provider(&self) -> &'static str {");
    pushln(out, "        match self {");
    for cfg in PROVIDERS {
        pushln(
            out,
            format!(
                "            Self::{}(_) => \"{}\",",
                cfg.enum_name, cfg.parser_name
            ),
        );
    }
    for dyn_cfg in DYNAMIC_PROVIDERS {
        pushln(
            out,
            format!(
                "            Self::{}(_) => \"{}\",",
                dyn_cfg.enum_name, dyn_cfg.parser_name
            ),
        );
    }
    pushln(out, "        }");
    pushln(out, "    }");
    blank(out);
}

fn emit_llm_provider_display_name(out: &mut String) {
    pushln(
        out,
        "    /// Human-readable provider name (e.g. `AWS Bedrock`)",
    );
    pushln(
        out,
        "    pub fn provider_display_name(&self) -> &'static str {",
    );
    pushln(out, "        match self {");
    for cfg in PROVIDERS {
        pushln(
            out,
            format!(
                "            Self::{}(_) => \"{}\",",
                cfg.enum_name, cfg.display_name
            ),
        );
    }
    for dyn_cfg in DYNAMIC_PROVIDERS {
        pushln(
            out,
            format!(
                "            Self::{}(_) => \"{}\",",
                dyn_cfg.enum_name, dyn_cfg.display_name
            ),
        );
    }
    pushln(out, "        }");
    pushln(out, "    }");
    blank(out);
}

fn emit_llm_context_window(out: &mut String) {
    pushln(
        out,
        "    /// Context window size in tokens (None for dynamic providers)",
    );
    pushln(out, "    pub fn context_window(&self) -> Option<u32> {");
    pushln(out, "        match self {");
    for cfg in PROVIDERS {
        pushln(
            out,
            format!(
                "            Self::{}(m) => Some(m.context_window()),",
                cfg.enum_name
            ),
        );
    }
    pushln(
        out,
        format!("            {} => None,", dynamic_pattern_with_binding("_")),
    );
    pushln(out, "        }");
    pushln(out, "    }");
    blank(out);
}

fn emit_llm_required_env_var(out: &mut String) {
    pushln(
        out,
        "    /// Required env var for this model's provider (None for local providers)",
    );
    pushln(
        out,
        "    pub fn required_env_var(&self) -> Option<&'static str> {",
    );
    pushln(out, "        match self {");
    let mut none_arms: Vec<String> = Vec::new();
    for cfg in PROVIDERS {
        match cfg.env_var {
            Some(var) => pushln(
                out,
                format!(
                    "            Self::{}(_) => Some(\"{}\"),",
                    cfg.enum_name, var
                ),
            ),
            None => none_arms.push(format!("Self::{}(_)", cfg.enum_name)),
        }
    }
    for dyn_cfg in DYNAMIC_PROVIDERS {
        none_arms.push(format!("Self::{}(_)", dyn_cfg.enum_name));
    }
    pushln(
        out,
        format!("            {} => None,", none_arms.join(" | ")),
    );
    pushln(out, "        }");
    pushln(out, "    }");
    blank(out);
}

fn emit_llm_oauth_provider_id(out: &mut String) {
    pushln(
        out,
        "    /// OAuth provider ID if this model requires OAuth login (e.g. `\"codex\"`)",
    );
    pushln(
        out,
        "    pub fn oauth_provider_id(&self) -> Option<&'static str> {",
    );
    pushln(out, "        match self {");
    let mut none_arms: Vec<String> = Vec::new();
    for cfg in PROVIDERS {
        match cfg.oauth_provider_id {
            Some(id) => pushln(
                out,
                format!(
                    "            Self::{}(_) => Some(\"{}\"),",
                    cfg.enum_name, id
                ),
            ),
            None => none_arms.push(format!("Self::{}(_)", cfg.enum_name)),
        }
    }
    for dyn_cfg in DYNAMIC_PROVIDERS {
        none_arms.push(format!("Self::{}(_)", dyn_cfg.enum_name));
    }
    pushln(
        out,
        format!("            {} => None,", none_arms.join(" | ")),
    );
    pushln(out, "        }");
    pushln(out, "    }");
    blank(out);
}

fn emit_llm_supports_reasoning(out: &mut String) {
    pushln(
        out,
        "    /// Whether this model supports reasoning/extended thinking",
    );
    pushln(out, "    pub fn supports_reasoning(&self) -> bool {");
    pushln(out, "        match self {");
    for cfg in PROVIDERS {
        pushln(
            out,
            format!(
                "            Self::{}(m) => m.supports_reasoning(),",
                cfg.enum_name
            ),
        );
    }
    pushln(
        out,
        format!(
            "            {} => false,",
            dynamic_pattern_with_binding("_")
        ),
    );
    pushln(out, "        }");
    pushln(out, "    }");
    blank(out);
}

fn emit_llm_all(out: &mut String) {
    pushln(
        out,
        "    /// All catalog models (excludes dynamic providers)",
    );
    pushln(out, "    pub fn all() -> &'static [LlmModel] {");
    pushln(
        out,
        "        static ALL: LazyLock<Vec<LlmModel>> = LazyLock::new(|| {",
    );
    pushln(out, "            let mut v = Vec::new();");
    for cfg in PROVIDERS {
        pushln(
            out,
            format!(
                "            v.extend({}Model::ALL.iter().copied().map(LlmModel::{}));",
                cfg.enum_name, cfg.enum_name
            ),
        );
    }
    pushln(out, "            v");
    pushln(out, "        });");
    pushln(out, "        &ALL");
    pushln(out, "    }");
}

fn emit_display_impl(out: &mut String) {
    pushln(out, "impl std::fmt::Display for LlmModel {");
    pushln(
        out,
        "    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {",
    );
    pushln(
        out,
        "        write!(f, \"{}:{}\", self.provider(), self.model_id())",
    );
    pushln(out, "    }");
    pushln(out, "}");
    blank(out);
}

fn emit_fromstr_impl(out: &mut String) {
    pushln(out, "impl std::str::FromStr for LlmModel {");
    pushln(out, "    type Err = String;");
    blank(out);
    pushln(
        out,
        "    /// Parse a `provider:model` string into an `LlmModel`",
    );
    pushln(out, "    fn from_str(s: &str) -> Result<Self, Self::Err> {");
    pushln(
        out,
        "        let (provider_str, model_str) = s.split_once(':').unwrap_or((s, \"\"));",
    );
    pushln(out, "        match provider_str {");
    for cfg in PROVIDERS {
        pushln(
            out,
            format!(
                "            \"{}\" => model_str.parse::<{}Model>().map(Self::{}),",
                cfg.parser_name, cfg.enum_name, cfg.enum_name
            ),
        );
    }
    for dyn_cfg in DYNAMIC_PROVIDERS {
        pushln(
            out,
            format!(
                "            \"{}\" => Ok(Self::{}(model_str.to_string())),",
                dyn_cfg.parser_name, dyn_cfg.enum_name
            ),
        );
    }
    pushln(
        out,
        "            _ => Err(format!(\"Unknown provider: '{provider_str}'\")),",
    );
    pushln(out, "        }");
    pushln(out, "    }");
    pushln(out, "}");
}

/// Build a combined `|` pattern for all dynamic providers with a binding variable.
/// e.g. `Self::Ollama(s) | Self::LlamaCpp(s)` or `Self::Ollama(_) | Self::LlamaCpp(_)`
fn dynamic_pattern_with_binding(binding: &str) -> String {
    DYNAMIC_PROVIDERS
        .iter()
        .map(|d| format!("Self::{}({binding})", d.enum_name))
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Format a number with underscore separators (e.g. `200000` → `200_000`).
fn format_number(n: u32) -> String {
    let s = n.to_string();
    if s.len() <= 4 {
        return s;
    }
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            result.push('_');
        }
        result.push(ch);
    }
    result
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

        let source = generate_from_value(&data);
        // Provider-level FromStr: sorted model IDs
        let a_model = "\"a-model\" => Ok(Self::AModel),";
        let b_model = "\"b-model\" => Ok(Self::BModel),";
        let a_pos = source.find(a_model).expect("a-model parse arm");
        let b_pos = source.find(b_model).expect("b-model parse arm");
        assert!(a_pos < b_pos);
        // Aliases and non-tool-call models are excluded
        assert!(!source.contains("AlphaLatest"));
        assert!(!source.contains("NoTools"));
    }

    #[test]
    fn generate_contains_core_sections() {
        let source = generate_from_value(&minimal_models_dev_json());
        assert!(source.contains("pub enum LlmModel {"));
        assert!(source.contains("impl std::str::FromStr for LlmModel {"));
        assert!(source.contains("impl std::fmt::Display for LlmModel {"));
        assert!(source.contains("pub fn required_env_var(&self) -> Option<&'static str> {"));
    }

    #[test]
    fn generate_contains_dynamic_provider_arms() {
        let source = generate_from_value(&minimal_models_dev_json());
        assert!(source.contains("\"ollama\" => Ok(Self::Ollama(model_str.to_string())),"));
        assert!(source.contains("\"llamacpp\" => Ok(Self::LlamaCpp(model_str.to_string())),"));
        // Dynamic providers are combined with | for None-returning arms
        assert!(source.contains("Self::Ollama(_) | Self::LlamaCpp(_) => None,"));
    }

    #[test]
    fn generate_codex_is_catalog_provider() {
        let source = generate_from_value(&minimal_models_dev_json());
        // Codex is a catalog provider, not dynamic
        assert!(source.contains("pub enum CodexModel {"));
        assert!(source.contains("\"codex\" => model_str.parse::<CodexModel>().map(Self::Codex),"));
        assert!(source.contains("Self::Codex(m) => Some(m.context_window()),"));
    }

    #[test]
    fn generate_oauth_provider_id_for_codex() {
        let source = generate_from_value(&minimal_models_dev_json());
        // Codex models return Some("codex") for oauth_provider_id
        assert!(source.contains("Self::Codex(_) => Some(\"codex\"),"));
        // Non-OAuth providers return None
        assert!(source.contains("pub fn oauth_provider_id(&self) -> Option<&'static str>"));
    }

    #[test]
    fn generate_delegates_to_provider_impls() {
        let source = generate_from_value(&minimal_models_dev_json());
        // LlmModel delegates to per-provider methods
        assert!(source.contains("Self::Anthropic(m) => Cow::Borrowed(m.model_id()),"));
        assert!(source.contains("Self::Anthropic(m) => Some(m.context_window()),"));
        // Provider-level FromStr is used by LlmModel::FromStr
        assert!(source.contains(
            "\"anthropic\" => model_str.parse::<AnthropicModel>().map(Self::Anthropic),"
        ));
    }

    #[test]
    fn generate_formats_large_numbers_with_separators() {
        let mut data = minimal_models_dev_json();
        let root = data.as_object_mut().expect("root object");
        let anthropic = root
            .get_mut("anthropic")
            .and_then(Value::as_object_mut)
            .expect("anthropic provider");

        anthropic.insert(
            "models".to_string(),
            json!({
                "big-model": {
                    "id": "big-model",
                    "name": "Big Model",
                    "tool_call": true,
                    "limit": {"context": 200_000, "output": 0}
                }
            }),
        );

        let source = generate_from_value(&data);
        assert!(source.contains("200_000"));
        assert!(!source.contains("200000"));
    }

    #[test]
    fn generate_groups_identical_match_arms() {
        let mut data = minimal_models_dev_json();
        let root = data.as_object_mut().expect("root object");
        let anthropic = root
            .get_mut("anthropic")
            .and_then(Value::as_object_mut)
            .expect("anthropic provider");

        anthropic.insert(
            "models".to_string(),
            json!({
                "model-a": {
                    "id": "model-a",
                    "name": "Same Name",
                    "tool_call": true,
                    "limit": {"context": 100_000, "output": 0}
                },
                "model-b": {
                    "id": "model-b",
                    "name": "Same Name",
                    "tool_call": true,
                    "limit": {"context": 100_000, "output": 0}
                }
            }),
        );

        let source = generate_from_value(&data);
        // Both context_window and display_name should combine arms
        assert!(source.contains("Self::ModelA | Self::ModelB => 100_000,"));
        assert!(source.contains("Self::ModelA | Self::ModelB => \"Same Name\","));
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

    #[test]
    fn generate_contains_supports_reasoning() {
        let mut data = minimal_models_dev_json();
        let root = data.as_object_mut().expect("root object");
        let anthropic = root
            .get_mut("anthropic")
            .and_then(Value::as_object_mut)
            .expect("anthropic provider");

        anthropic.insert(
            "models".to_string(),
            json!({
                "thinker": {
                    "id": "thinker",
                    "name": "Thinker",
                    "tool_call": true,
                    "reasoning": true,
                    "limit": {"context": 1000, "output": 0}
                },
                "fast": {
                    "id": "fast",
                    "name": "Fast",
                    "tool_call": true,
                    "reasoning": false,
                    "limit": {"context": 1000, "output": 0}
                }
            }),
        );

        let source = generate_from_value(&data);
        // Provider enum should have supports_reasoning method
        assert!(source.contains("pub fn supports_reasoning(self) -> bool {"));
        // LlmModel should delegate to provider
        assert!(source.contains("pub fn supports_reasoning(&self) -> bool {"));
        assert!(source.contains("Self::Thinker => true,"));
        assert!(source.contains("Self::Fast => false,"));
        // Dynamic providers return false
        assert!(source.contains("Self::Ollama(_) | Self::LlamaCpp(_) => false,"));
    }

    fn generate_from_value(data: &Value) -> String {
        let tmp = NamedTempFile::new().expect("temp file");
        let json = serde_json::to_string(data).expect("serialize fixture");
        std::fs::write(tmp.path(), json).expect("write fixture");
        generate(tmp.path()).expect("codegen succeeds")
    }

    fn minimal_models_dev_json() -> Value {
        let mut root = serde_json::Map::new();
        for cfg in PROVIDERS {
            let json_key = cfg.json_key();
            root.entry(json_key.to_string()).or_insert_with(|| {
                json!({
                    "id": json_key,
                    "name": json_key,
                    "env": [],
                    "models": {}
                })
            });
        }
        Value::Object(root)
    }
}
