use super::mappers::{map_messages, map_tools};
use super::streaming::process_anthropic_stream;
use super::types::{CacheControl, Request, SystemContent, SystemContentBlock};
use crate::auth::store::{load as load_credentials, save as save_credentials};
use crate::auth::{AuthError, OAuthTokens, ProviderCredentials, refresh as refresh_oauth};
use crate::llm::provider::{LlmResponseStream, ProviderFactory, StreamingModelProvider};
use crate::llm::{Context, LlmError, Result};
use async_stream;
use futures::StreamExt;
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, AUTHORIZATION, CONTENT_TYPE, HeaderValue};
use reqwest::{Client, header};
use std::env;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncBufReadExt;
use tokio::sync::Mutex;
use tokio_stream::wrappers::LinesStream;
use tokio_util::io::StreamReader;
use tracing::debug;

#[derive(Clone)]
pub struct AnthropicProvider {
    client: Client,
    model: String,
    base_url: Option<String>,
    temperature: Option<f32>,
    max_tokens: u32,
    enable_prompt_caching: bool,
    oauth_mode: bool,
    auth: Arc<Mutex<ProviderCredentials>>,
}

impl AnthropicProvider {
    pub fn new(auth: ProviderCredentials) -> Result<Self> {
        let client = build_client()?;
        let oauth_mode = matches!(auth, ProviderCredentials::OAuth { .. });

        Ok(Self {
            client,
            model: "claude-sonnet-4-5-20250929".to_string(),
            base_url: Some("https://api.anthropic.com".to_string()),
            temperature: None,
            max_tokens: 16_384,
            enable_prompt_caching: true,
            oauth_mode,
            auth: Arc::new(Mutex::new(auth)),
        })
    }

    pub fn new_with_api_key(api_key: String) -> Result<Self> {
        Self::new(ProviderCredentials::api_key(&api_key))
    }

    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }

    pub fn with_base_url(mut self, base_url: &str) -> Self {
        self.base_url = Some(base_url.to_string());
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn with_prompt_caching(mut self, enable: bool) -> Self {
        self.enable_prompt_caching = enable;
        self
    }

    pub(crate) fn build_request(&self, context: &Context) -> Result<Request> {
        let (system_prompt, messages) = map_messages(context.messages())?;
        let tools = if context.tools().is_empty() {
            None
        } else {
            Some(map_tools(context.tools())?)
        };

        let mut request = Request::new(self.model.clone(), messages)
            .with_max_tokens(self.max_tokens)
            .with_stream(true);

        if let Some(temp) = self.temperature {
            request = request.with_temperature(temp);
        }

        if self.oauth_mode {
            let mut blocks = vec![SystemContentBlock::Text {
                text: "You are Claude Code, Anthropic's official CLI for Claude.".to_string(),
                cache_control: Some(CacheControl::ephemeral()),
            }];

            if let Some(system) = system_prompt {
                blocks.push(SystemContentBlock::Text {
                    text: system,
                    cache_control: Some(CacheControl::ephemeral()),
                });
            }

            request.system = Some(SystemContent::Blocks(blocks));
        } else if let Some(system) = system_prompt {
            request = if self.enable_prompt_caching {
                request.with_system_cached(system)
            } else {
                request.with_system(system)
            };
        }

        if let Some(tools) = tools {
            request = request.with_tools(tools);
        }

        debug!("Built Anthropic request for model: {}", request.model);
        Ok(request)
    }

    async fn build_headers(&self) -> Result<header::HeaderMap> {
        let mut headers = header::HeaderMap::new();
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let auth_snapshot = { self.auth.lock().await.clone() };
        match auth_snapshot {
            ProviderCredentials::Api { key } => {
                headers.insert("x-api-key", HeaderValue::from_str(&key)?);
            }
            ProviderCredentials::OAuth {
                access,
                refresh,
                expires,
            } => {
                let tokens = if expires <= now_millis() {
                    self.refresh_oauth_tokens(&refresh).await?
                } else {
                    OAuthTokens {
                        access,
                        refresh,
                        expires,
                    }
                };

                headers.insert(
                    AUTHORIZATION,
                    HeaderValue::from_str(&format!("Bearer {}", tokens.access))?,
                );
                headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
                headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("*"));
                headers.insert("sec-fetch-mode", HeaderValue::from_static("cors"));
                let beta = merge_anthropic_beta(headers.get("anthropic-beta"))?;
                headers.insert("anthropic-beta", beta);
            }
        }

        Ok(headers)
    }

    async fn refresh_oauth_tokens(&self, refresh: &str) -> Result<OAuthTokens> {
        let tokens = refresh_oauth(refresh).await.map_err(auth_error_to_llm)?;

        let mut auth = self.auth.lock().await;
        *auth = ProviderCredentials::OAuth {
            access: tokens.access.clone(),
            refresh: tokens.refresh.clone(),
            expires: tokens.expires,
        };
        drop(auth);

        let mut store = load_credentials().map_err(auth_error_to_llm)?;
        store.providers.insert(
            "anthropic".to_string(),
            ProviderCredentials::OAuth {
                access: tokens.access.clone(),
                refresh: tokens.refresh.clone(),
                expires: tokens.expires,
            },
        );
        save_credentials(&store).map_err(auth_error_to_llm)?;

        Ok(tokens)
    }

    async fn send_request(
        &self,
        request: Request,
    ) -> Result<impl futures::Stream<Item = Result<String>>> {
        let base_url = self
            .base_url
            .as_deref()
            .unwrap_or("https://api.anthropic.com");
        let url = format!("{base_url}/v1/messages");

        debug!("Sending request to Anthropic API: {url}");
        debug!(
            "Anthropic request body: {}",
            serde_json::to_string(&request).unwrap_or_else(|_| "<failed to serialize>".to_string())
        );

        let headers = self.build_headers().await?;
        debug!("Anthropic request headers: {}", format_headers(&headers));
        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&request)
            .send()
            .await
            .map_err(|e| LlmError::ApiRequest(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(LlmError::ApiError(format!(
                "Anthropic API request failed with status {status}: {error_text}"
            )));
        }

        let stream = response.bytes_stream();
        let stream_reader =
            StreamReader::new(stream.map(|result| result.map_err(std::io::Error::other)));

        let lines_stream = LinesStream::new(tokio::io::BufReader::new(stream_reader).lines());

        let processed_stream =
            lines_stream.map(|result| result.map_err(|e| LlmError::IoError(e.to_string())));

        Ok(processed_stream)
    }
}

impl ProviderFactory for AnthropicProvider {
    fn from_env() -> std::result::Result<Self, Box<dyn std::error::Error>> {
        if let Ok(api_key) = env::var("ANTHROPIC_API_KEY") {
            return Self::new(ProviderCredentials::api_key(&api_key))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>);
        }

        let store = load_credentials().map_err(auth_error_to_boxed)?;
        if let Some(credentials) = store.providers.get("anthropic") {
            return Self::new(credentials.clone())
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>);
        }

        Err(Box::new(LlmError::Other(
            "No Anthropic credentials found. Run `wisp auth anthropic` or set ANTHROPIC_API_KEY."
                .to_string(),
        )))
    }

    fn with_model(self, model: &str) -> Self {
        self.with_model(model)
    }
}

impl StreamingModelProvider for AnthropicProvider {
    fn stream_response<'a>(&self, context: &Context) -> LlmResponseStream {
        let provider = self.clone();

        let request = match self.build_request(context) {
            Ok(req) => req,
            Err(e) => {
                return Box::pin(async_stream::stream! {
                    yield Err(e);
                });
            }
        };

        Box::pin(async_stream::stream! {
            let stream = match provider.send_request(request).await {
                Ok(stream) => stream,
                Err(e) => {
                    yield Err(e);
                    return;
                }
            };

            let mut anthropic_stream = Box::pin(process_anthropic_stream(stream));
            while let Some(result) = anthropic_stream.next().await {
                yield result;
            }
        })
    }

    fn display_name(&self) -> String {
        format!("Anthropic ({})", self.model)
    }
}

fn merge_anthropic_beta(existing: Option<&HeaderValue>) -> Result<HeaderValue> {
    const REQUIRED_BETAS: [&str; 3] = [
        "claude-code-20250219",
        "oauth-2025-04-20",
        "interleaved-thinking-2025-05-14",
    ];

    let mut values: Vec<String> = Vec::new();
    if let Some(value) = existing {
        let value_str = value.to_str().map_err(|e| LlmError::Other(e.to_string()))?;
        values.extend(
            value_str
                .split(',')
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty()),
        );
    }

    for beta in REQUIRED_BETAS {
        if !values.iter().any(|value| value == beta) {
            values.push(beta.to_string());
        }
    }

    HeaderValue::from_str(&values.join(", ")).map_err(Into::into)
}

fn build_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| LlmError::HttpClientCreation(e.to_string()))
}

fn format_headers(headers: &header::HeaderMap) -> String {
    let mut parts = Vec::new();
    for (name, value) in headers.iter() {
        let name_str = name.as_str();
        let value_str = if name_str.eq_ignore_ascii_case("authorization") {
            "<redacted>".to_string()
        } else {
            value.to_str().unwrap_or("<non-utf8>").to_string()
        };
        parts.push(format!("{name_str}={value_str}"));
    }
    parts.join(", ")
}

fn auth_error_to_llm(error: AuthError) -> LlmError {
    LlmError::Other(error.to_string())
}

fn auth_error_to_boxed(error: AuthError) -> Box<dyn std::error::Error> {
    Box::new(LlmError::Other(error.to_string()))
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::ChatMessage;
    use crate::llm::anthropic::types::{SystemContent, SystemContentBlock};
    use crate::llm::tools::ToolDefinition;
    use crate::types::IsoString;

    fn create_test_provider() -> AnthropicProvider {
        AnthropicProvider::new(ProviderCredentials::api_key("test-api-key"))
            .unwrap()
            .with_model("claude-sonnet-4-5-20250929")
            .with_temperature(0.7)
            .with_max_tokens(1000)
            .with_prompt_caching(false)
    }

    #[test]
    fn test_provider_creation() {
        let provider = AnthropicProvider::new(ProviderCredentials::api_key("test-api-key"));
        assert!(provider.is_ok());
    }

    #[tokio::test]
    async fn build_headers_uses_api_key() {
        let provider =
            AnthropicProvider::new(ProviderCredentials::api_key("test-api-key")).unwrap();
        let headers = provider.build_headers().await.expect("headers");
        assert_eq!(
            headers
                .get("x-api-key")
                .and_then(|value| value.to_str().ok()),
            Some("test-api-key")
        );
        assert!(headers.get(AUTHORIZATION).is_none());
        assert!(headers.get("anthropic-beta").is_none());
    }

    #[tokio::test]
    async fn build_headers_uses_oauth_bearer() {
        let auth = ProviderCredentials::OAuth {
            access: "access-token".to_string(),
            refresh: "refresh-token".to_string(),
            expires: now_millis() + 60_000,
        };
        let provider = AnthropicProvider::new(auth).unwrap();
        let headers = provider.build_headers().await.expect("headers");
        assert_eq!(
            headers
                .get(AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer access-token")
        );
        assert!(headers.get("x-api-key").is_none());
        let betas = headers
            .get("anthropic-beta")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        let betas: Vec<&str> = betas
            .split(',')
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .collect();
        assert!(betas.contains(&"oauth-2025-04-20"));
        assert!(betas.contains(&"claude-code-20250219"));
    }

    #[test]
    fn test_build_request_simple() {
        let provider = create_test_provider();

        let context = Context::new(
            vec![ChatMessage::User {
                content: "Hello".to_string(),
                timestamp: IsoString::now(),
            }],
            vec![],
        );

        let request = provider.build_request(&context).unwrap();
        assert_eq!(request.model, "claude-sonnet-4-5-20250929");
        assert_eq!(request.max_tokens, 1000);
        assert_eq!(request.messages.len(), 1);
        assert!(request.tools.is_none());
        assert!(request.stream);
    }

    #[test]
    fn test_build_request_with_system_and_tools() {
        let provider = create_test_provider();

        let context = Context::new(
            vec![
                ChatMessage::System {
                    content: "You are helpful".to_string(),
                    timestamp: IsoString::now(),
                },
                ChatMessage::User {
                    content: "Hello".to_string(),
                    timestamp: IsoString::now(),
                },
            ],
            vec![ToolDefinition {
                name: "search".to_string(),
                description: "Search for information".to_string(),
                parameters: r#"{"type": "object", "properties": {"query": {"type": "string"}}}"#
                    .to_string(),
                server: None,
            }],
        );

        let request = provider.build_request(&context).unwrap();
        if let Some(system) = &request.system {
            match system {
                SystemContent::Text(text) => {
                    assert_eq!(text, "You are helpful");
                }
                _ => panic!("Expected text system content"),
            }
        } else {
            panic!("Expected system prompt");
        }
        assert_eq!(request.messages.len(), 1); // Only user message, system becomes separate field
        assert!(request.tools.is_some());
        assert_eq!(request.tools.unwrap().len(), 1);
    }

    #[test]
    fn test_build_request_with_caching() {
        let provider =
            AnthropicProvider::new(ProviderCredentials::api_key("test-api-key")).unwrap(); // Caching is enabled by default

        let context = Context::new(
            vec![
                ChatMessage::System {
                    content: "Hello".to_string(),
                    timestamp: IsoString::now(),
                },
                ChatMessage::User {
                    content: "Hello".to_string(),
                    timestamp: IsoString::now(),
                },
            ],
            vec![ToolDefinition {
                name: "search".to_string(),
                description: "Search for information".to_string(),
                parameters: r#"{"type": "object", "properties": {"query": {"type": "string"}}}"#
                    .to_string(),
                server: None,
            }],
        );

        let request = provider.build_request(&context).unwrap();

        // With caching enabled, system prompt should be cached
        if let Some(system) = &request.system {
            match system {
                SystemContent::Blocks(blocks) => {
                    assert_eq!(blocks.len(), 1);
                    let SystemContentBlock::Text {
                        text,
                        cache_control,
                    } = &blocks[0];
                    assert_eq!(text, "Hello");
                    assert!(cache_control.is_some());
                }
                _ => panic!("Expected blocks system content for caching"),
            }
        } else {
            panic!("Expected system prompt");
        }

        // Tools should not have cache_control (they're automatically cached when system is cached)
        assert!(request.tools.is_some());
        let tools = request.tools.unwrap();
        assert!(tools[0].cache_control.is_none());
    }

    #[test]
    fn test_build_request_with_no_caching() {
        let provider = AnthropicProvider::new(ProviderCredentials::api_key("test-api-key"))
            .unwrap()
            .with_prompt_caching(false);

        let context = Context::new(
            vec![
                ChatMessage::System {
                    content: "Hello".to_string(),
                    timestamp: IsoString::now(),
                },
                ChatMessage::User {
                    content: "Hello".to_string(),
                    timestamp: IsoString::now(),
                },
            ],
            vec![],
        );

        let request = provider.build_request(&context).unwrap();

        // With caching disabled, system prompt should be simple text
        if let Some(system) = &request.system {
            match system {
                SystemContent::Text(text) => {
                    assert_eq!(text, "Hello");
                }
                _ => panic!("Expected text system content when caching disabled"),
            }
        } else {
            panic!("Expected system prompt");
        }
    }

    #[test]
    fn test_anthropic_provider_display_name() {
        let provider = create_test_provider();
        assert_eq!(
            provider.display_name(),
            "Anthropic (claude-sonnet-4-5-20250929)"
        );
    }

    #[test]
    fn test_anthropic_provider_display_name_default() {
        let provider =
            AnthropicProvider::new(ProviderCredentials::api_key("test-api-key")).unwrap();
        assert_eq!(
            provider.display_name(),
            "Anthropic (claude-sonnet-4-5-20250929)"
        );
    }
}
