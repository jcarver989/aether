use llm::StreamingModelProvider;

/// Message from the user to the agent.
pub enum UserMessage {
    Text { content: String },
    Cancel,
    SwitchModel(Box<dyn StreamingModelProvider>),
}

impl std::fmt::Debug for UserMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserMessage::Text { content } => {
                f.debug_struct("Text").field("content", content).finish()
            }
            UserMessage::Cancel => write!(f, "Cancel"),
            UserMessage::SwitchModel(provider) => f
                .debug_tuple("SwitchModel")
                .field(&provider.display_name())
                .finish(),
        }
    }
}

impl UserMessage {
    pub fn text(content: &str) -> Self {
        UserMessage::Text {
            content: content.to_string(),
        }
    }
}

impl From<&str> for UserMessage {
    fn from(value: &str) -> Self {
        UserMessage::Text {
            content: value.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use llm::{ModelProvider, ProviderModel};

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
