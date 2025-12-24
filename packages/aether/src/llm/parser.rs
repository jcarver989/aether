use std::collections::HashMap;

use crate::llm::{
    LlmError, ProviderFactory, StreamingModelProvider,
    alloyed::AlloyedModelProvider,
    anthropic::AnthropicProvider,
    gemini::GeminiProvider,
    local::{llama_cpp::LlamaCppProvider, ollama::OllamaProvider},
    openrouter::OpenRouterProvider,
    z_ai::ZAiProvider,
};

/// Parser that turns a provider:model string (e.g. anthropic:claude-sonnet-4.5) into
/// a StreamingLlmProvider
///
pub struct ModelProviderParser {
    factories: HashMap<String, CreateProviderFn>,
}

impl ModelProviderParser {
    /// Create a new parser with custom provider factories
    pub fn new(factories: HashMap<String, CreateProviderFn>) -> Self {
        Self { factories }
    }
}

impl Default for ModelProviderParser {
    /// Create a parser with all built-in providers registered
    fn default() -> Self {
        Self::new(HashMap::new())
            .with_provider::<AnthropicProvider>("anthropic")
            .with_provider::<GeminiProvider>("gemini")
            .with_provider::<OpenRouterProvider>("openrouter")
            .with_provider::<OllamaProvider>("ollama")
            .with_provider::<ZAiProvider>("zai")
            .with_provider::<LlamaCppProvider>("llamacpp")
    }
}

impl ModelProviderParser {
    pub fn with_provider<P: ProviderFactory + StreamingModelProvider + 'static>(
        mut self,
        name: impl Into<String>,
    ) -> Self {
        self.factories.insert(
            name.into(),
            Box::new(|model| Ok(Box::new(P::from_env()?.with_model(model)))),
        );
        self
    }

    /// Parse a model specification string and create a provider instance
    ///
    /// Supports both single specs and comma-separated specs for alloyed providers.
    ///
    /// # Format
    ///
    /// - `"provider:model"` - Single provider (e.g., "anthropic:claude-3.5-sonnet")
    /// - `"provider1:model1,provider2:model2"` - Multiple providers create an AlloyedModelProvider
    ///
    pub fn parse(&self, models_str: &str) -> crate::llm::Result<Box<dyn StreamingModelProvider>> {
        let provider_model_pairs: Vec<&str> = models_str.split(',').map(|s| s.trim()).collect();
        if provider_model_pairs.is_empty() {
            return Err(LlmError::Other("No models provided".to_string()));
        }

        let mut providers = Vec::new();
        for pair in provider_model_pairs {
            let (provider_name, model) = if let Some((name, model)) = pair.split_once(':') {
                (name, model)
            } else {
                // Handle providers without model specification (e.g., "llamacpp")
                (pair, "")
            };

            let factory = self
                .factories
                .get(provider_name)
                .ok_or_else(|| LlmError::Other(format!("Unknown provider: {provider_name}")))?;

            providers.push(factory(model)?);
        }

        let provider: Box<dyn StreamingModelProvider> = if providers.len() == 1 {
            providers.into_iter().next().unwrap()
        } else {
            Box::new(AlloyedModelProvider::new(providers))
        };

        Ok(provider)
    }
}

/// Factory function type for creating model providers
///
/// Takes a model name and returns a boxed StreamingModelProvider
pub type CreateProviderFn =
    Box<dyn Fn(&str) -> crate::llm::Result<Box<dyn StreamingModelProvider>> + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_llamacpp() {
        let parser = ModelProviderParser::default();
        let result = parser.parse("llamacpp");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_anthropic() {
        let parser = ModelProviderParser::default();
        let result = parser.parse("anthropic:claude-3.5-sonnet");
        // Will fail without API key, but should parse successfully
        if let Err(e) = result {
            let err = e.to_string();
            assert!(
                err.contains("API") || err.contains("ANTHROPIC"),
                "Should fail on API key, not parsing"
            );
        }
    }

    #[test]
    fn test_parse_ollama() {
        let parser = ModelProviderParser::default();
        let result = parser.parse("ollama:llama3.2");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_openrouter() {
        let parser = ModelProviderParser::default();
        let result = parser.parse("openrouter:google/gemma-2-9b-it:free");
        // Will fail without API key, but should parse successfully
        if let Err(e) = result {
            let err = e.to_string();
            assert!(
                err.contains("API") || err.contains("OPENROUTER"),
                "Should fail on API key, not parsing"
            );
        }
    }

    #[test]
    fn test_parse_gemini() {
        let parser = ModelProviderParser::default();
        let result = parser.parse("gemini:gemini-2.5-flash");
        // Will fail without API key, but should parse successfully
        if let Err(e) = result {
            let err = e.to_string();
            assert!(
                err.contains("API") || err.contains("GEMINI"),
                "Should fail on API key, not parsing"
            );
        }
    }

    #[test]
    fn test_parse_provider_without_model() {
        let parser = ModelProviderParser::default();
        let result = parser.parse("anthropic");
        // Will fail because ANTHROPIC_API_KEY is not set, but should parse successfully
        if let Err(e) = result {
            let err = e.to_string();
            assert!(
                err.contains("API") || err.contains("ANTHROPIC"),
                "Should fail on API key, not parsing. Got: {err}"
            );
        }
    }

    #[test]
    fn test_parse_unknown_provider() {
        let parser = ModelProviderParser::default();
        let result = parser.parse("unknown:model");
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("Unknown provider"));
        }
    }

    #[test]
    fn test_with_custom_provider() {
        let parser = ModelProviderParser::default().with_provider::<OllamaProvider>("custom");

        let result = parser.parse("custom:test-model");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_single_provider() {
        let parser = ModelProviderParser::default();
        let result = parser.parse("llamacpp");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_multiple_providers() {
        let parser = ModelProviderParser::default();
        let result = parser.parse("llamacpp,ollama:llama3.2");
        assert!(result.is_ok());
        // Should return AlloyedModelProvider
    }

    #[test]
    fn test_parse_with_spaces() {
        let parser = ModelProviderParser::default();
        let result = parser.parse("llamacpp , ollama:llama3.2");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parser_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ModelProviderParser>();
    }
}
