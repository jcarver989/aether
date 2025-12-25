use async_stream::stream;
use futures::StreamExt;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use serde_json::json;
use std::env::var;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_util::io::StreamReader;

use crate::auth::{FileCredentialStore, credentials::ProviderCredential, google as google_auth};
use crate::llm::openai_compatible::{build_chat_request, create_custom_stream};
use crate::llm::{
    Context, LlmError, LlmResponseStream, ProviderFactory, Result, StreamingModelProvider,
};

use super::mappers::build_codeassist_request;
use super::streaming::process_codeassist_stream;

pub const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/openai/";

/// CodeAssist API endpoint (for OAuth auth)
const CODEASSIST_ENDPOINT: &str = "https://cloudcode-pa.googleapis.com";
const CODEASSIST_API_VERSION: &str = "v1internal";
const PROVIDER_NAME: &str = "gemini";

/// Response from loadCodeAssist endpoint
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoadCodeAssistResponse {
    cloudaicompanion_project: Option<String>,
}

/// Credential type with the actual credential value
enum CredentialInfo {
    ApiKey(String),
    OAuth(String),
}

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

    /// Get credential info, determining whether it's an API key or OAuth token
    async fn get_credential_info(&self) -> Result<CredentialInfo> {
        if let Ok(api_key) = var("GEMINI_API_KEY") {
            return Ok(CredentialInfo::ApiKey(api_key));
        }

        let credential = self
            .store
            .get_provider(PROVIDER_NAME)
            .await
            .map_err(|e| LlmError::Other(e.to_string()))?
            .ok_or_else(|| {
                LlmError::MissingApiKey(
                    "GEMINI_API_KEY not set and no OAuth credentials found. Run 'aether auth login --provider gemini' to authenticate.".to_string()
                )
            })?;

        match credential {
            ProviderCredential::ApiKey { key } => Ok(CredentialInfo::ApiKey(key)),
            ProviderCredential::OAuth {
                access_token,
                refresh_token,
                expires_at,
            } => {
                let token = if expires_at <= now_millis() {
                    // Token expired, refresh it
                    let tokens = google_auth::refresh(&refresh_token)
                        .await
                        .map_err(|e| LlmError::Other(format!("Failed to refresh token: {e}")))?;

                    // Store the refreshed tokens
                    let new_credential = ProviderCredential::oauth(
                        tokens.access.clone(),
                        tokens.refresh.clone(),
                        tokens.expires,
                    );
                    self.store
                        .set_provider(PROVIDER_NAME, new_credential)
                        .await
                        .map_err(|e| LlmError::Other(e.to_string()))?;

                    tokens.access
                } else {
                    access_token
                };

                Ok(CredentialInfo::OAuth(token))
            }
        }
    }

    /// Build OpenAI-compatible client for API key auth
    fn build_openai_client(
        api_key: &str,
    ) -> async_openai::Client<async_openai::config::OpenAIConfig> {
        let config = async_openai::config::OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base(GEMINI_API_BASE);
        async_openai::Client::with_config(config)
    }

    /// Load the managed project ID from CodeAssist API
    async fn load_managed_project(access_token: &str) -> Result<Option<String>> {
        let url = format!(
            "{}/{}:loadCodeAssist",
            CODEASSIST_ENDPOINT, CODEASSIST_API_VERSION
        );

        let body = json!({
            "metadata": json!({
                        "ideType": "IDE_UNSPECIFIED",
                        "platform": "PLATFORM_UNSPECIFIED",
                        "pluginType": "GEMINI"
                    })
        });

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header(AUTHORIZATION, format!("Bearer {}", access_token))
            .header(CONTENT_TYPE, "application/json")
            .header(USER_AGENT, "google-api-nodejs-client/9.15.1")
            .header("X-Goog-Api-Client", "gl-node/22.17.0")
            .header(
                "Client-Metadata",
                "ideType=IDE_UNSPECIFIED,platform=PLATFORM_UNSPECIFIED,pluginType=GEMINI",
            )
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::ApiRequest(format!("loadCodeAssist failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            tracing::warn!("loadCodeAssist returned {}: {}", status, text);
            return Ok(None);
        }

        let payload: LoadCodeAssistResponse = response.json().await.map_err(|e| {
            LlmError::Other(format!("Failed to parse loadCodeAssist response: {e}"))
        })?;

        tracing::info!(
            "Loaded managed project: {:?}",
            payload.cloudaicompanion_project
        );
        Ok(payload.cloudaicompanion_project)
    }

    /// Send request to CodeAssist endpoint for OAuth auth
    async fn send_codeassist_request(
        &self,
        context: &Context,
        access_token: &str,
        project_id: Option<&str>,
    ) -> Result<impl futures::Stream<Item = Result<String>>> {
        let mut request_body = build_codeassist_request(&self.model, context);
        // Set project ID if available
        request_body.project = project_id.map(String::from);

        let url = format!(
            "{}/{}:streamGenerateContent?alt=sse",
            CODEASSIST_ENDPOINT, CODEASSIST_API_VERSION
        );

        tracing::debug!("CodeAssist request URL: {}", url);
        tracing::info!("CodeAssist request model: {}", request_body.model);
        // Print full request body at WARN level for debugging
        tracing::warn!(
            "CodeAssist request body:\n{}",
            serde_json::to_string_pretty(&request_body).unwrap_or_default()
        );

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .map_err(|e| LlmError::Other(format!("Failed to create HTTP client: {e}")))?;

        let response = client
            .post(&url)
            .header(AUTHORIZATION, format!("Bearer {}", access_token))
            .header(CONTENT_TYPE, "application/json")
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .header(
                reqwest::header::USER_AGENT,
                "google-api-nodejs-client/9.15.1",
            )
            .header("X-Goog-Api-Client", "gl-node/22.17.0")
            .header(
                "Client-Metadata",
                "ideType=IDE_UNSPECIFIED,platform=PLATFORM_UNSPECIFIED,pluginType=GEMINI",
            )
            .json(&request_body)
            .send()
            .await
            .map_err(|e| LlmError::ApiRequest(format!("CodeAssist request failed: {e}")))?;

        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();
        let content_length = response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());

        tracing::debug!(
            "CodeAssist response: status={}, content-type={}, content-length={:?}",
            status,
            content_type,
            content_length
        );

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            // Log the full error for debugging
            tracing::error!(
                "CodeAssist API error ({}): {}",
                status,
                &error_text[..error_text.len().min(2000)]
            );
            return Err(LlmError::ApiError(format!(
                "CodeAssist API error ({}): {}",
                status,
                &error_text[..error_text.len().min(500)]
            )));
        }

        // Check for SSE content type - the server should return text/event-stream for streaming
        // If it returns JSON or other content types with a defined length, the body is not streaming
        if content_length == Some(0) {
            return Err(LlmError::ApiError(
                "CodeAssist API returned empty response body".to_string(),
            ));
        }

        // If Content-Type is application/json with a Content-Length, it's likely an error response
        // embedded in the body with 200 status (Google APIs sometimes do this)
        if content_type.contains("application/json") && content_length.is_some() {
            let body = response.text().await.unwrap_or_default();
            tracing::warn!("CodeAssist returned JSON instead of SSE stream: {}", body);
            return Err(LlmError::ApiError(format!(
                "CodeAssist API error (unexpected JSON response): {}",
                body
            )));
        }

        // Convert response body stream to lines
        let byte_stream = response.bytes_stream();
        let stream_reader = StreamReader::new(
            byte_stream.map(|r| r.map_err(|e| std::io::Error::other(e.to_string()))),
        );
        let reader = BufReader::new(stream_reader);
        let lines = tokio_stream::wrappers::LinesStream::new(reader.lines());
        let processed = lines.map(|r| r.map_err(|e| LlmError::Other(e.to_string())));

        Ok(processed)
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
            let credential_info = match provider.get_credential_info().await {
                Ok(info) => info,
                Err(e) => {
                    yield Err(e);
                    return;
                }
            };

            match credential_info {
                CredentialInfo::ApiKey(api_key) => {
                    // Use OpenAI-compatible endpoint
                    tracing::info!("Using Gemini API with API key (OpenAI-compatible endpoint)");
                    let client = Self::build_openai_client(&api_key);
                    let request = build_chat_request(&provider.model, &context);
                    let mut inner_stream = create_custom_stream(&client, request);

                    while let Some(result) = inner_stream.next().await {
                        yield result;
                    }
                }
                CredentialInfo::OAuth(access_token) => {
                    // Use CodeAssist endpoint
                    tracing::info!("Using Gemini API with OAuth (CodeAssist endpoint)");

                    // First, load the managed project ID
                    let project_id = match Self::load_managed_project(&access_token).await {
                        Ok(Some(id)) => Some(id),
                        Ok(None) => {
                            tracing::warn!("No managed project found, proceeding without project ID");
                            None
                        }
                        Err(e) => {
                            tracing::warn!("Failed to load managed project: {}, proceeding without", e);
                            None
                        }
                    };

                    let raw_stream = match provider.send_codeassist_request(&context, &access_token, project_id.as_deref()).await {
                        Ok(s) => s,
                        Err(e) => {
                            yield Err(e);
                            return;
                        }
                    };

                    let mut codeassist_stream = Box::pin(process_codeassist_stream(raw_stream));
                    while let Some(result) = codeassist_stream.next().await {
                        yield result;
                    }
                }
            }
        })
    }

    fn display_name(&self) -> String {
        format!("Gemini ({})", self.model)
    }
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

    #[test]
    fn test_provider_display_name() {
        let store = FileCredentialStore::with_path(std::path::PathBuf::from("/tmp/test"));
        let provider = GeminiProvider::new(store)
            .unwrap()
            .with_model("gemini-2.0-flash");

        assert_eq!(provider.display_name(), "Gemini (gemini-2.0-flash)");
    }
}
