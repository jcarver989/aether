include!(concat!(env!("OUT_DIR"), "/generated.rs"));

/// Returns models whose provider env var is set
pub fn available_models() -> Vec<LlmModel> {
    LlmModel::all()
        .iter()
        .filter(|m| {
            m.required_env_var()
                .is_none_or(|var| std::env::var(var).is_ok())
        })
        .cloned()
        .collect()
}
