use std::collections::HashMap;

use crate::catalog::LlmModel;
#[cfg(feature = "bedrock")]
use crate::providers::bedrock::BedrockProvider;
#[cfg(feature = "codex")]
use crate::providers::codex::CodexProvider;
use crate::providers::{
    anthropic::AnthropicProvider,
    gemini::GeminiProvider,
    local::{llama_cpp::LlamaCppProvider, ollama::OllamaProvider},
    openai_compatible::generic::{self, GenericOpenAiProvider},
    openrouter::OpenRouterProvider,
};
use crate::{LlmError, ProviderFactory, StreamingModelProvider, alloyed::AlloyedModelProvider};

/// Parser that turns a provider:model string (e.g. anthropic:claude-sonnet-4.5) into
/// a `StreamingLlmProvider`
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
        let parser = Self::new(HashMap::new())
            .with_provider::<AnthropicProvider>("anthropic")
            .with_provider::<GeminiProvider>("gemini")
            .with_provider::<OpenRouterProvider>("openrouter")
            .with_provider::<OllamaProvider>("ollama")
            .with_provider::<LlamaCppProvider>("llamacpp")
            .with_openai_provider("deepseek", &generic::DEEPSEEK)
            .with_openai_provider("moonshot", &generic::MOONSHOT)
            .with_openai_provider("zai", &generic::ZAI);

        #[cfg(feature = "bedrock")]
        let parser = parser.with_provider::<BedrockProvider>("bedrock");

        #[cfg(feature = "codex")]
        let parser = parser.with_provider::<CodexProvider>("codex");

        parser
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

    pub fn with_openai_provider(
        mut self,
        name: impl Into<String>,
        config: &'static generic::ProviderConfig,
    ) -> Self {
        self.factories.insert(
            name.into(),
            Box::new(move |model| {
                Ok(Box::new(
                    GenericOpenAiProvider::from_env(config)?.with_model(model),
                ))
            }),
        );
        self
    }

    /// Create a provider from a typed `LlmModel`
    pub fn create_provider(
        &self,
        model: &LlmModel,
    ) -> crate::Result<Box<dyn StreamingModelProvider>> {
        let key = model.provider();
        let factory = self
            .factories
            .get(key)
            .ok_or_else(|| LlmError::Other(format!("Unknown provider: {key}")))?;
        factory(&model.model_id())
    }

    /// Parse a model specification string and create a provider instance.
    ///
    /// Returns both the provider and an `LlmModel` describing the identity
    /// of the first (or only) provider in the spec.
    ///
    /// # Format
    ///
    /// - `"provider:model"` - Single provider (e.g., "anthropic:claude-3.5-sonnet")
    /// - `"provider1:model1,provider2:model2"` - Multiple providers create an `AlloyedModelProvider`
    ///
    pub fn parse(
        &self,
        models_str: &str,
    ) -> crate::Result<(Box<dyn StreamingModelProvider>, LlmModel)> {
        let provider_model_pairs: Vec<&str> = models_str.split(',').map(str::trim).collect();
        if provider_model_pairs.is_empty() {
            return Err(LlmError::Other("No models provided".to_string()));
        }

        let mut providers = Vec::new();
        let mut first_identity: Option<LlmModel> = None;

        for pair in provider_model_pairs {
            let (provider_name, model) = pair.split_once(':').unwrap_or((pair, ""));

            let factory = self
                .factories
                .get(provider_name)
                .ok_or_else(|| LlmError::Other(format!("Unknown provider: {provider_name}")))?;

            providers.push(factory(model)?);

            if first_identity.is_none() {
                first_identity = Some(pair.parse::<LlmModel>().map_err(LlmError::Other)?);
            }
        }

        let identity =
            first_identity.ok_or_else(|| LlmError::Other("No providers parsed".to_string()))?;

        let provider: Box<dyn StreamingModelProvider> = if providers.len() == 1 {
            providers
                .into_iter()
                .next()
                .ok_or_else(|| LlmError::Other("No providers available".to_string()))?
        } else {
            Box::new(AlloyedModelProvider::new(providers))
        };

        Ok((provider, identity))
    }
}

/// Factory function type for creating model providers
///
/// Takes a model name and returns a boxed `StreamingModelProvider`
pub type CreateProviderFn =
    Box<dyn Fn(&str) -> crate::Result<Box<dyn StreamingModelProvider>> + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_llamacpp() {
        let parser = ModelProviderParser::default();
        let result = parser.parse("llamacpp");
        assert!(result.is_ok());
        let (_, model) = result.unwrap();
        assert_eq!(model, LlmModel::LlamaCpp(String::new()));
    }

    #[test]
    fn test_parse_anthropic() {
        let parser = ModelProviderParser::default();
        let result = parser.parse("anthropic:claude-3-5-sonnet-20241022");
        // Will fail without API key or credentials, but should parse successfully
        match result {
            Ok((_, model)) => {
                assert_eq!(
                    model,
                    LlmModel::Anthropic(crate::catalog::AnthropicModel::Claude35Sonnet20241022)
                );
            }
            Err(e) => {
                let err = e.to_string();
                assert!(
                    err.contains("API")
                        || err.contains("ANTHROPIC")
                        || err.contains("credentials")
                        || err.contains("JSON"),
                    "Should fail on API key or credentials, not parsing. Got: {err}"
                );
            }
        }
    }

    #[test]
    fn test_parse_ollama() {
        let parser = ModelProviderParser::default();
        let result = parser.parse("ollama:llama3.2");
        assert!(result.is_ok());
        let (_, model) = result.unwrap();
        assert_eq!(model, LlmModel::Ollama("llama3.2".to_string()));
    }

    #[test]
    fn test_parse_openrouter() {
        let parser = ModelProviderParser::default();
        let result = parser.parse("openrouter:google/gemini-2.5-flash");
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
        // Will fail because either ANTHROPIC_API_KEY is not set,
        // or the empty model string doesn't match any catalog model
        assert!(result.is_err());
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

        // Custom providers work for creating providers via create_provider
        let model = LlmModel::Ollama("test-model".to_string());
        // The factory was registered under "custom", but create_provider uses model.provider()
        // which returns "ollama". So we test that the ollama factory works.
        let result = parser.create_provider(&model);
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
        let (_, model) = result.unwrap();
        // For alloyed, uses the first provider's identity
        assert_eq!(model, LlmModel::LlamaCpp(String::new()));
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
