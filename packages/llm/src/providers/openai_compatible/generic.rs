use async_openai::{Client, config::OpenAIConfig};

use crate::provider::get_context_window;
use crate::{Context, LlmError, LlmResponseStream, StreamingModelProvider};

use super::{build_chat_request, create_custom_stream_generic};

/// Configuration for an OpenAI-compatible provider.
///
/// Each provider that uses the standard `build_chat_request → create_custom_stream_generic`
/// flow differs only in these constants.
pub struct ProviderConfig {
    pub api_base: &'static str,
    pub env_var: &'static str,
    pub default_model: &'static str,
    pub prefix: &'static str,
    pub display_name: &'static str,
}

pub const DEEPSEEK: ProviderConfig = ProviderConfig {
    api_base: "https://api.deepseek.com",
    env_var: "DEEPSEEK_API_KEY",
    default_model: "deepseek-chat",
    prefix: "deepseek",
    display_name: "DeepSeek",
};

pub const MOONSHOT: ProviderConfig = ProviderConfig {
    api_base: "https://api.moonshot.ai/v1",
    env_var: "MOONSHOT_API_KEY",
    default_model: "moonshot-v1-8k",
    prefix: "moonshot",
    display_name: "Moonshot",
};

pub const ZAI: ProviderConfig = ProviderConfig {
    api_base: "https://api.z.ai/api/coding/paas/v4",
    env_var: "ZAI_API_KEY",
    default_model: "GLM-4.6",
    prefix: "zai",
    display_name: "Z.ai",
};

/// A generic provider for APIs that are fully OpenAI-compatible.
///
/// Providers like DeepSeek, Moonshot, and Z.ai differ only in their API base URL,
/// environment variable name, default model, and display prefix. This struct
/// captures that pattern once instead of duplicating the same impl three times.
pub struct GenericOpenAiProvider {
    client: Client<OpenAIConfig>,
    model: String,
    config: &'static ProviderConfig,
}

impl GenericOpenAiProvider {
    pub fn from_env(config: &'static ProviderConfig) -> crate::Result<Self> {
        let api_key = std::env::var(config.env_var)
            .map_err(|_| LlmError::MissingApiKey(config.env_var.to_string()))?;
        Ok(Self::new(api_key, config))
    }

    pub fn new(api_key: String, config: &'static ProviderConfig) -> Self {
        let openai_config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base(config.api_base.to_string());

        Self {
            client: Client::with_config(openai_config),
            model: config.default_model.to_string(),
            config,
        }
    }

    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }
}

impl StreamingModelProvider for GenericOpenAiProvider {
    fn model(&self) -> Option<crate::LlmModel> {
        format!("{}:{}", self.config.prefix, self.model)
            .parse()
            .ok()
    }

    fn context_window(&self) -> Option<u32> {
        get_context_window(self.config.prefix, &self.model)
    }

    fn stream_response(&self, context: &Context) -> LlmResponseStream {
        let request = match build_chat_request(&self.model, context) {
            Ok(req) => req,
            Err(e) => return Box::pin(async_stream::stream! { yield Err(e); }),
        };
        create_custom_stream_generic(&self.client, request)
    }

    fn display_name(&self) -> String {
        format!("{} ({})", self.config.display_name, self.model)
    }
}
