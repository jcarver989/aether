use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub mcp_servers: HashMap<String, McpServerConfig>,
    pub provider: ProviderType,
    pub model: String,
    pub openrouter_api_key: Option<String>,
    pub ollama_base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub url: String,
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    OpenRouter,
    Ollama,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct McpConfig {
    pub servers: HashMap<String, McpServerConfig>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Configuration file not found: {0}")]
    FileNotFound(String),
    #[error("Invalid JSON in configuration file: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("IO error reading configuration: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Missing required environment variable: {0}")]
    MissingEnvVar(String),
    #[error("Invalid provider type: {0}")]
    InvalidProvider(String),
}

impl Config {
    pub fn load() -> Result<Self> {
        let mcp_config = Self::load_mcp_config("mcp.json")?;
        let provider = Self::get_provider_from_env()?;
        let model = Self::get_model_from_env(&provider)?;
        let openrouter_api_key = env::var("OPENROUTER_API_KEY").ok();
        let ollama_base_url = env::var("OLLAMA_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());

        Ok(Config {
            mcp_servers: mcp_config.servers,
            provider,
            model,
            openrouter_api_key,
            ollama_base_url,
        })
    }

    pub fn from_file(path: &Path) -> Result<Self> {
        let mcp_config = Self::load_mcp_config(path)?;
        let provider = Self::get_provider_from_env()?;
        let model = Self::get_model_from_env(&provider)?;
        let openrouter_api_key = env::var("OPENROUTER_API_KEY").ok();
        let ollama_base_url = env::var("OLLAMA_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());

        Ok(Config {
            mcp_servers: mcp_config.servers,
            provider,
            model,
            openrouter_api_key,
            ollama_base_url,
        })
    }

    fn load_mcp_config<P: AsRef<Path>>(path: P) -> Result<McpConfig, ConfigError> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(ConfigError::FileNotFound(path.display().to_string()));
        }

        let content = fs::read_to_string(path)?;
        let config: McpConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    fn get_provider_from_env() -> Result<ProviderType, ConfigError> {
        let provider_str = env::var("DEFAULT_PROVIDER").unwrap_or_else(|_| "openrouter".to_string());
        match provider_str.to_lowercase().as_str() {
            "openrouter" => Ok(ProviderType::OpenRouter),
            "ollama" => Ok(ProviderType::Ollama),
            _ => Err(ConfigError::InvalidProvider(provider_str)),
        }
    }

    fn get_model_from_env(provider: &ProviderType) -> Result<String, ConfigError> {
        env::var("DEFAULT_MODEL").or_else(|_: env::VarError| {
            match provider {
                ProviderType::OpenRouter => Ok("anthropic/claude-3.5-sonnet".to_string()),
                ProviderType::Ollama => Ok("llama2".to_string()),
            }
        }).map_err(|_: env::VarError| ConfigError::MissingEnvVar("DEFAULT_MODEL".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn create_test_mcp_config() -> String {
        serde_json::json!({
            "servers": {
                "filesystem": {
                    "url": "http://localhost:3000/mcp/filesystem",
                    "headers": {}
                },
                "git": {
                    "url": "http://localhost:3001/mcp/git",
                    "headers": {
                        "Authorization": "Bearer token123"
                    }
                }
            }
        }).to_string()
    }

    #[test]
    fn test_load_mcp_config_success() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_mcp.json");
        fs::write(&file_path, create_test_mcp_config()).unwrap();

        let config = Config::load_mcp_config(&file_path).unwrap();
        assert_eq!(config.servers.len(), 2);
        assert!(config.servers.contains_key("filesystem"));
        assert!(config.servers.contains_key("git"));
        
        let fs_config = &config.servers["filesystem"];
        assert_eq!(fs_config.url, "http://localhost:3000/mcp/filesystem");
        assert_eq!(fs_config.headers.len(), 0);
    }

    #[test]
    fn test_load_mcp_config_file_not_found() {
        let result = Config::load_mcp_config("nonexistent.json");
        assert!(matches!(result, Err(ConfigError::FileNotFound(_))));
    }

    #[test]
    fn test_load_mcp_config_invalid_json() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("invalid.json");
        fs::write(&file_path, "invalid json").unwrap();

        let result = Config::load_mcp_config(&file_path);
        assert!(matches!(result, Err(ConfigError::InvalidJson(_))));
    }

    #[test]
    fn test_get_provider_from_env_default() {
        // Save and restore env
        let original = std::env::var("DEFAULT_PROVIDER").ok();
        std::env::remove_var("DEFAULT_PROVIDER");
        
        let provider = Config::get_provider_from_env().unwrap();
        assert_eq!(provider, ProviderType::OpenRouter);
        
        // Restore original value
        if let Some(val) = original {
            std::env::set_var("DEFAULT_PROVIDER", val);
        }
    }

    #[test]
    fn test_get_provider_from_env_ollama() {
        // Save and restore env
        let original = std::env::var("DEFAULT_PROVIDER").ok();
        std::env::set_var("DEFAULT_PROVIDER", "ollama");
        
        let provider = Config::get_provider_from_env().unwrap();
        assert_eq!(provider, ProviderType::Ollama);
        
        // Restore original value
        if let Some(val) = original {
            std::env::set_var("DEFAULT_PROVIDER", val);
        } else {
            std::env::remove_var("DEFAULT_PROVIDER");
        }
    }

    #[test]
    fn test_get_provider_from_env_invalid() {
        // Save and restore env
        let original = std::env::var("DEFAULT_PROVIDER").ok();
        std::env::set_var("DEFAULT_PROVIDER", "invalid");
        
        let result = Config::get_provider_from_env();
        assert!(matches!(result, Err(ConfigError::InvalidProvider(_))));
        
        // Restore original value
        if let Some(val) = original {
            std::env::set_var("DEFAULT_PROVIDER", val);
        } else {
            std::env::remove_var("DEFAULT_PROVIDER");
        }
    }

    #[test]
    fn test_get_model_from_env_defaults() {
        // Save and restore env
        let original = std::env::var("DEFAULT_MODEL").ok();
        std::env::remove_var("DEFAULT_MODEL");
        
        let openrouter_model = Config::get_model_from_env(&ProviderType::OpenRouter).unwrap();
        assert_eq!(openrouter_model, "anthropic/claude-3.5-sonnet");
        
        let ollama_model = Config::get_model_from_env(&ProviderType::Ollama).unwrap();
        assert_eq!(ollama_model, "llama2");
        
        // Restore original value
        if let Some(val) = original {
            std::env::set_var("DEFAULT_MODEL", val);
        }
    }

    #[test]
    fn test_get_model_from_env_custom() {
        // Save and restore env
        let original = std::env::var("DEFAULT_MODEL").ok();
        std::env::set_var("DEFAULT_MODEL", "custom-model");
        
        let model = Config::get_model_from_env(&ProviderType::OpenRouter).unwrap();
        assert_eq!(model, "custom-model");
        
        // Restore original value
        if let Some(val) = original {
            std::env::set_var("DEFAULT_MODEL", val);
        } else {
            std::env::remove_var("DEFAULT_MODEL");
        }
    }
}