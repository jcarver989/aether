use super::{
    CodegenCtx, DYNAMIC_PROVIDERS, DynamicProviderConfig, ModelInfo, PROVIDERS, ProviderConfig,
    ProviderModels,
};
use std::fmt::Write;

pub(super) fn emit_generated_source(ctx: &CodegenCtx) -> String {
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
    pushln(
        out,
        "// Run `cargo run --bin llm-catalog-codegen` to regenerate",
    );
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
