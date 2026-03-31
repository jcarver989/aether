use std::fmt;

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextOverflowError {
    pub provider: String,
    pub model: Option<String>,
    pub requested_tokens: Option<u32>,
    pub max_tokens: Option<u32>,
    pub message: String,
}

impl ContextOverflowError {
    pub fn new(
        provider: impl Into<String>,
        model: Option<String>,
        requested_tokens: Option<u32>,
        max_tokens: Option<u32>,
        message: impl Into<String>,
    ) -> Self {
        Self { provider: provider.into(), model, requested_tokens, max_tokens, message: message.into() }
    }
}

impl fmt::Display for ContextOverflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let model = self.model.as_deref().unwrap_or("unknown-model");
        match (self.requested_tokens, self.max_tokens) {
            (Some(requested), Some(max)) => write!(
                f,
                "{} (provider={}, model={}, requested={}, max={})",
                self.message, self.provider, model, requested, max
            ),
            _ => write!(f, "{} (provider={}, model={})", self.message, self.provider, model),
        }
    }
}

#[doc = include_str!("docs/llm_error.md")]
#[derive(Debug, Error, Clone)]
pub enum LlmError {
    /// Environment variable not set or invalid
    #[error("{0} environment variable not set")]
    MissingApiKey(String),
    /// Invalid API key format
    #[error("Invalid API key: {0}")]
    InvalidApiKey(String),
    /// HTTP client creation failed
    #[error("Failed to create HTTP client: {0}")]
    HttpClientCreation(String),
    /// API request failed
    #[error("API request failed: {0}")]
    ApiRequest(String),
    /// API returned an error response
    #[error("API error: {0}")]
    ApiError(String),
    /// API rejected the request because the prompt exceeded the model's context window.
    #[error("Context overflow: {0}")]
    ContextOverflow(ContextOverflowError),
    /// IO error while reading stream
    #[error("IO error reading stream: {0}")]
    IoError(String),
    /// JSON parsing/serialization error
    #[error("JSON parsing error: {0}")]
    JsonParsing(String),
    /// Tool parameter parsing error
    #[error("Failed to parse tool parameters for {tool_name}: {error}")]
    ToolParameterParsing { tool_name: String, error: String },
    /// OAuth authentication error
    #[error("OAuth error: {0}")]
    OAuthError(String),
    /// The message contained only content types this provider doesn't support
    #[error("Unsupported content: {0}")]
    UnsupportedContent(String),
    /// Generic error for other cases
    #[error("{0}")]
    Other(String),
}

impl From<reqwest::Error> for LlmError {
    fn from(error: reqwest::Error) -> Self {
        LlmError::ApiRequest(error.to_string())
    }
}

impl From<serde_json::Error> for LlmError {
    fn from(error: serde_json::Error) -> Self {
        LlmError::JsonParsing(error.to_string())
    }
}

impl From<std::io::Error> for LlmError {
    fn from(error: std::io::Error) -> Self {
        LlmError::IoError(error.to_string())
    }
}

impl From<reqwest::header::InvalidHeaderValue> for LlmError {
    fn from(error: reqwest::header::InvalidHeaderValue) -> Self {
        LlmError::InvalidApiKey(error.to_string())
    }
}

impl From<async_openai::error::OpenAIError> for LlmError {
    fn from(error: async_openai::error::OpenAIError) -> Self {
        LlmError::ApiError(error.to_string())
    }
}

#[cfg(feature = "oauth")]
impl From<crate::oauth::OAuthError> for LlmError {
    fn from(error: crate::oauth::OAuthError) -> Self {
        LlmError::OAuthError(error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, LlmError>;
