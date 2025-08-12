use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Serialize, Deserialize, Type)]
pub struct AppConfig {
    pub llm_provider: String,
    pub api_key: Option<String>,
    pub model_name: String,
}

#[tauri::command]
#[specta::specta]
pub async fn get_config() -> Result<AppConfig, String> {
    // TODO: Load from actual config file
    Ok(AppConfig {
        llm_provider: "openrouter".to_string(),
        api_key: None,
        model_name: "anthropic/claude-3.5-sonnet".to_string(),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn update_config(config: AppConfig) -> Result<(), String> {
    // TODO: Save to actual config file
    println!("Config updated: {:?}", config);
    Ok(())
}