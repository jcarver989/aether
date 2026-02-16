use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Known LLM provider backends
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelProvider {
    Anthropic,
    DeepSeek,
    Gemini,
    Moonshot,
    OpenRouter,
    Ollama,
    ZAi,
    LlamaCpp,
    /// Custom/unknown provider name registered at runtime
    #[serde(untagged)]
    Other(String),
}

impl fmt::Display for ModelProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelProvider::Anthropic => write!(f, "anthropic"),
            ModelProvider::DeepSeek => write!(f, "deepseek"),
            ModelProvider::Gemini => write!(f, "gemini"),
            ModelProvider::Moonshot => write!(f, "moonshot"),
            ModelProvider::OpenRouter => write!(f, "openrouter"),
            ModelProvider::Ollama => write!(f, "ollama"),
            ModelProvider::ZAi => write!(f, "zai"),
            ModelProvider::LlamaCpp => write!(f, "llamacpp"),
            ModelProvider::Other(name) => write!(f, "{name}"),
        }
    }
}

impl FromStr for ModelProvider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "anthropic" => Ok(ModelProvider::Anthropic),
            "deepseek" => Ok(ModelProvider::DeepSeek),
            "gemini" => Ok(ModelProvider::Gemini),
            "moonshot" => Ok(ModelProvider::Moonshot),
            "openrouter" => Ok(ModelProvider::OpenRouter),
            "ollama" => Ok(ModelProvider::Ollama),
            "zai" => Ok(ModelProvider::ZAi),
            "llamacpp" => Ok(ModelProvider::LlamaCpp),
            _ => Ok(ModelProvider::Other(s.to_string())),
        }
    }
}

/// A provider paired with a specific model name
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProviderModel {
    pub provider: ModelProvider,
    pub model: String,
}

impl ProviderModel {
    pub fn new(provider: ModelProvider, model: impl Into<String>) -> Self {
        Self {
            provider,
            model: model.into(),
        }
    }
}

impl fmt::Display for ProviderModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.provider, self.model)
    }
}

impl FromStr for ProviderModel {
    type Err = String;

    /// Parse a "provider:model" string (e.g. "anthropic:claude-sonnet-4-5-20250514")
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (provider_str, model) = s
            .split_once(':')
            .map(|(p, m)| (p, m.to_string()))
            .unwrap_or((s, String::new()));
        let provider: ModelProvider = provider_str.parse()?;
        Ok(Self { provider, model })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_provider_display_roundtrips_through_from_str() {
        let providers = [
            ModelProvider::Anthropic,
            ModelProvider::DeepSeek,
            ModelProvider::Gemini,
            ModelProvider::Moonshot,
            ModelProvider::OpenRouter,
            ModelProvider::Ollama,
            ModelProvider::ZAi,
            ModelProvider::LlamaCpp,
        ];
        for provider in providers {
            let s = provider.to_string();
            let parsed: ModelProvider = s.parse().unwrap();
            assert_eq!(parsed, provider);
        }
    }

    #[test]
    fn test_model_provider_from_str_unknown() {
        let result: ModelProvider = "custom".parse().unwrap();
        assert_eq!(result, ModelProvider::Other("custom".to_string()));
    }

    #[test]
    fn test_provider_model_from_str() {
        let pm: ProviderModel = "anthropic:claude-3.5-sonnet".parse().unwrap();
        assert_eq!(pm.provider, ModelProvider::Anthropic);
        assert_eq!(pm.model, "claude-3.5-sonnet");
    }

    #[test]
    fn test_provider_model_from_str_no_model() {
        let pm: ProviderModel = "llamacpp".parse().unwrap();
        assert_eq!(pm.provider, ModelProvider::LlamaCpp);
        assert_eq!(pm.model, "");
    }

    #[test]
    fn test_provider_model_display() {
        let pm = ProviderModel::new(ModelProvider::Ollama, "llama3.2");
        assert_eq!(pm.to_string(), "ollama:llama3.2");
    }

    #[test]
    fn test_provider_model_serde_roundtrip() {
        let pm = ProviderModel::new(ModelProvider::Anthropic, "claude-3.5-sonnet");
        let json = serde_json::to_string(&pm).unwrap();
        let parsed: ProviderModel = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, pm);
    }
}
