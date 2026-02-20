use super::mappers::{map_messages, map_tools};
use super::streaming::process_anthropic_stream;
use super::types::Request;
use crate::provider::{LlmResponseStream, ProviderFactory, StreamingModelProvider};
use crate::{Context, LlmError, Result};
use async_stream;
use futures::StreamExt;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use reqwest::{Client, header};
use std::env;
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
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
    api_key: Option<String>,
}

impl AnthropicProvider {
    pub fn new(api_key: Option<String>) -> Result<Self> {
        let client = build_client()?;

        Ok(Self {
            client,
            model: "claude-sonnet-4-5-20250929".to_string(),
            base_url: Some("https://api.anthropic.com".to_string()),
            temperature: None,
            max_tokens: 16_384,
            enable_prompt_caching: true,
            api_key,
        })
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

        if let Some(system) = system_prompt {
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

    fn get_api_key(&self) -> Result<String> {
        if let Some(key) = &self.api_key {
            return Ok(key.clone());
        }

        if let Ok(api_key) = env::var("ANTHROPIC_API_KEY") {
            return Ok(api_key);
        }

        Err(LlmError::MissingApiKey(
            "No Anthropic credentials found. Set ANTHROPIC_API_KEY environment variable."
                .to_string(),
        ))
    }

    fn build_headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let api_key = self.get_api_key()?;
        headers.insert("x-api-key", HeaderValue::from_str(&api_key)?);
        Ok(headers)
    }

    async fn send_request(
        &self,
        request: Request,
        headers: header::HeaderMap,
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
    fn from_env() -> Result<Self> {
        Self::new(None)
    }

    fn with_model(self, model: &str) -> Self {
        self.with_model(model)
    }
}

impl StreamingModelProvider for AnthropicProvider {
    fn stream_response<'a>(&self, context: &Context) -> LlmResponseStream {
        let provider = self.clone();
        let context = context.clone();

        Box::pin(async_stream::stream! {
            let headers = match provider.build_headers() {
                Ok(result) => result,
                Err(e) => {
                    yield Err(e);
                    return;
                }
            };

            let request = match provider.build_request(&context) {
                Ok(req) => req,
                Err(e) => {
                    yield Err(e);
                    return;
                }
            };

            let stream = match provider.send_request(request, headers).await {
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

fn build_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| LlmError::HttpClientCreation(e.to_string()))
}

fn should_redact_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "authorization"
        || lower == "x-api-key"
        || lower.contains("secret")
        || lower.contains("token")
}

fn format_headers(headers: &header::HeaderMap) -> String {
    let mut parts = Vec::new();
    for (name, value) in headers {
        let name_str = name.as_str();
        let value_str = if should_redact_header(name_str) {
            "<redacted>".to_string()
        } else {
            value.to_str().unwrap_or("<non-utf8>").to_string()
        };
        parts.push(format!("{name_str}={value_str}"));
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChatMessage;
    use crate::ToolDefinition;
    use crate::providers::anthropic::types::{SystemContent, SystemContentBlock};
    use crate::types::IsoString;
    use reqwest::header::AUTHORIZATION;

    fn create_test_provider() -> AnthropicProvider {
        AnthropicProvider::new(Some("test-api-key".to_string()))
            .unwrap()
            .with_model("claude-sonnet-4-5-20250929")
            .with_temperature(0.7)
            .with_max_tokens(1000)
            .with_prompt_caching(false)
    }

    #[test]
    fn test_provider_creation() {
        let provider = AnthropicProvider::new(Some("test-api-key".to_string()));
        assert!(provider.is_ok());
    }

    #[test]
    fn build_headers_uses_api_key() {
        let provider = AnthropicProvider::new(Some("test-api-key".to_string())).unwrap();
        let headers = provider.build_headers().expect("headers");
        assert_eq!(
            headers
                .get("x-api-key")
                .and_then(|value| value.to_str().ok()),
            Some("test-api-key")
        );
        assert!(headers.get(AUTHORIZATION).is_none());
        assert!(headers.get("anthropic-beta").is_none());
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
        let provider = AnthropicProvider::new(Some("test-api-key".to_string())).unwrap(); // Caching is enabled by default

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
        let provider = AnthropicProvider::new(Some("test-api-key".to_string()))
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
        let provider = AnthropicProvider::new(Some("test-api-key".to_string())).unwrap();
        assert_eq!(
            provider.display_name(),
            "Anthropic (claude-sonnet-4-5-20250929)"
        );
    }

    #[test]
    fn format_headers_redacts_x_api_key() {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_static("sk-secret-123"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let formatted = format_headers(&headers);
        assert!(formatted.contains("x-api-key=<redacted>"));
        assert!(formatted.contains("content-type=application/json"));
        assert!(!formatted.contains("sk-secret-123"));
    }

    #[test]
    fn format_headers_redacts_authorization() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer token123"));

        let formatted = format_headers(&headers);
        assert!(formatted.contains("authorization=<redacted>"));
        assert!(!formatted.contains("token123"));
    }

    #[test]
    fn format_headers_redacts_secret_and_token_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-client-secret", HeaderValue::from_static("mysecret"));
        headers.insert("x-auth-token", HeaderValue::from_static("mytoken"));
        headers.insert("accept", HeaderValue::from_static("text/plain"));

        let formatted = format_headers(&headers);
        assert!(formatted.contains("x-client-secret=<redacted>"));
        assert!(formatted.contains("x-auth-token=<redacted>"));
        assert!(formatted.contains("accept=text/plain"));
        assert!(!formatted.contains("mysecret"));
        assert!(!formatted.contains("mytoken"));
    }
}
