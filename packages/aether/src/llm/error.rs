use thiserror::Error;

#[derive(Debug, Error)]
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
    /// IO error while reading stream
    #[error("IO error reading stream: {0}")]
    IoError(String),
    /// JSON parsing/serialization error
    #[error("JSON parsing error: {0}")]
    JsonParsing(String),
    /// Tool parameter parsing error
    #[error("Failed to parse tool parameters for {tool_name}: {error}")]
    ToolParameterParsing { tool_name: String, error: String },
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

pub type Result<T> = std::result::Result<T, LlmError>;
