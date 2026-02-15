use crate::auth::{FileCredentialStore, credentials::ProviderCredential};
use crate::llm::openai_compatible::{build_chat_request, create_custom_stream};
use crate::llm::{
    Context, LlmError, LlmResponseStream, ProviderFactory, Result, StreamingModelProvider,
};
use async_stream::stream;
use futures::StreamExt;
use std::env::var;

pub const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/openai/";
const PROVIDER_NAME: &str = "gemini";

#[derive(Clone)]
pub struct GeminiProvider {
    store: FileCredentialStore,
    model: String,
}

impl GeminiProvider {
    pub fn new(store: FileCredentialStore) -> Result<Self> {
        Ok(Self {
            store,
            model: String::new(),
        })
    }

    async fn get_api_key(&self) -> Result<String> {
        if let Ok(api_key) = var("GEMINI_API_KEY") {
            return Ok(api_key);
        }

        let credential = self
            .store
            .get_provider(PROVIDER_NAME)
            .await
            .map_err(|e| LlmError::Other(e.to_string()))?
            .ok_or_else(|| {
                LlmError::MissingApiKey(
                    "GEMINI_API_KEY not set and no credentials found. Set the environment variable or store an API key."
                        .to_string(),
                )
            })?;

        match credential {
            ProviderCredential::ApiKey { key } => Ok(key),
        }
    }

    fn build_openai_client(
        api_key: &str,
    ) -> async_openai::Client<async_openai::config::OpenAIConfig> {
        let config = async_openai::config::OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base(GEMINI_API_BASE);
        async_openai::Client::with_config(config)
    }
}

impl ProviderFactory for GeminiProvider {
    fn from_env() -> Result<Self> {
        let store = FileCredentialStore::new().map_err(|e| LlmError::Other(e.to_string()))?;
        Self::new(store)
    }

    fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }
}

impl StreamingModelProvider for GeminiProvider {
    fn stream_response(&self, context: &Context) -> LlmResponseStream {
        let provider = self.clone();
        let context = context.clone();

        Box::pin(stream! {
            let api_key = match provider.get_api_key().await {
                Ok(key) => key,
                Err(e) => {
                    yield Err(e);
                    return;
                }
            };

            tracing::info!("Using Gemini API with API key (OpenAI-compatible endpoint)");
            let client = Self::build_openai_client(&api_key);
            let request = build_chat_request(&provider.model, &context);
            let mut inner_stream = create_custom_stream(&client, request);

            while let Some(result) = inner_stream.next().await {
                yield result;
            }
        })
    }

    fn display_name(&self) -> String {
        format!("Gemini ({})", self.model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_display_name() {
        let store = FileCredentialStore::with_path(std::path::PathBuf::from("/tmp/test"));
        let provider = GeminiProvider::new(store)
            .unwrap()
            .with_model("gemini-2.0-flash");

        assert_eq!(provider.display_name(), "Gemini (gemini-2.0-flash)");
    }
}
