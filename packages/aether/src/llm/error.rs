use std::fmt;

#[derive(Debug)]
pub enum LlmError {
    /// Environment variable not set or invalid
    MissingApiKey(String),
    /// Invalid API key format
    InvalidApiKey(String),
    /// HTTP client creation failed
    HttpClientCreation(String),
    /// API request failed
    ApiRequest(String),
    /// API returned an error response
    ApiError(String),
    /// IO error while reading stream
    IoError(String),
    /// JSON parsing/serialization error
    JsonParsing(String),
    /// Tool parameter parsing error
    ToolParameterParsing { tool_name: String, error: String },
    /// Generic error for other cases
    Other(String),
}

impl fmt::Display for LlmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LlmError::MissingApiKey(var) => write!(f, "{} environment variable not set", var),
            LlmError::InvalidApiKey(msg) => write!(f, "Invalid API key: {}", msg),
            LlmError::HttpClientCreation(msg) => write!(f, "Failed to create HTTP client: {}", msg),
            LlmError::ApiRequest(msg) => write!(f, "API request failed: {}", msg),
            LlmError::ApiError(msg) => write!(f, "API error: {}", msg),
            LlmError::IoError(msg) => write!(f, "IO error reading stream: {}", msg),
            LlmError::JsonParsing(msg) => write!(f, "JSON parsing error: {}", msg),
            LlmError::ToolParameterParsing { tool_name, error } => {
                write!(f, "Failed to parse tool parameters for {}: {}", tool_name, error)
            }
            LlmError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for LlmError {}

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