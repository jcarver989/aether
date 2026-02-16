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
    use llm::LlmModel;

    #[test]
    fn test_llm_model_from_str_roundtrip() {
        let models = [
            "anthropic:claude-opus-4-6",
            "deepseek:deepseek-chat",
            "gemini:gemini-2.5-flash",
            "ollama:llama3.2",
            "llamacpp:",
        ];
        for input in models {
            let model: LlmModel = input.parse().unwrap();
            let round_tripped = format!("{}:{}", model.provider(), model.model_id());
            assert_eq!(round_tripped, input);
        }
    }

    #[test]
    fn test_llm_model_from_str_unknown_provider() {
        let result: Result<LlmModel, _> = "custom:foo".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_llm_model_dynamic_providers() {
        let ollama: LlmModel = "ollama:llama3.2".parse().unwrap();
        assert_eq!(ollama, LlmModel::Ollama("llama3.2".to_string()));

        let llamacpp: LlmModel = "llamacpp".parse().unwrap();
        assert_eq!(llamacpp, LlmModel::LlamaCpp(String::new()));
    }
}
