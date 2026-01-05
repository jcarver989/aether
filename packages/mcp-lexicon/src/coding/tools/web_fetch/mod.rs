mod http_client;

#[cfg(test)]
pub use http_client::FakeHttpClient;
pub use http_client::{
    DEFAULT_TIMEOUT_MS, HttpClient, HttpResponse, MAX_CONTENT_LENGTH, MAX_TIMEOUT_MS,
    ReqwestClient, WebFetchInput, WebFetchOutput,
};

use htmd::convert;
use reqwest::Url;
use std::time::Duration;

use crate::coding::display_meta::ToolDisplayMeta;
use crate::coding::error::WebFetchError;

/// HTTP client for fetching web content and converting to markdown.
#[derive(Debug, Clone)]
pub struct WebFetcher<C: HttpClient = ReqwestClient> {
    client: C,
}

impl Default for WebFetcher<ReqwestClient> {
    fn default() -> Self {
        Self::new()
    }
}

impl WebFetcher<ReqwestClient> {
    /// Creates a new WebFetcher with a preconfigured reqwest client
    pub fn new() -> Self {
        Self {
            client: ReqwestClient::new(),
        }
    }
}

impl<C: HttpClient> WebFetcher<C> {
    /// Creates a WebFetcher with a custom HTTP client (useful for testing)
    pub fn with_client(client: C) -> Self {
        Self { client }
    }

    /// Fetches web content and converts it to markdown
    pub async fn fetch(&self, args: WebFetchInput) -> Result<WebFetchOutput, WebFetchError> {
        let url = normalize_url(&args.url)?;

        let timeout_ms = args
            .timeout
            .map(|t| t.min(MAX_TIMEOUT_MS))
            .unwrap_or(DEFAULT_TIMEOUT_MS);

        let response = self
            .client
            .fetch(&url, Duration::from_millis(timeout_ms))
            .await?;

        let (title, markdown) = html_to_markdown(&response.body);
        let (content, truncated) = if markdown.len() > MAX_CONTENT_LENGTH {
            (truncate_str(&markdown, MAX_CONTENT_LENGTH), true)
        } else {
            (markdown, false)
        };

        let display_meta =
            ToolDisplayMeta::web_fetch(url.clone(), title.clone(), Some(content.len() as u64));

        Ok(WebFetchOutput {
            content,
            final_url: response.final_url,
            status_code: response.status_code,
            truncated,
            title,
            _meta: display_meta.into_meta(),
        })
    }
}

fn normalize_url(url: &str) -> Result<String, WebFetchError> {
    let url = if url.starts_with("http://") {
        url.replacen("http://", "https://", 1)
    } else if !url.starts_with("https://") {
        format!("https://{url}")
    } else {
        url.to_string()
    };

    Url::parse(&url)
        .map(|u| u.to_string())
        .map_err(|e| WebFetchError::InvalidUrl(e.to_string()))
}

fn html_to_markdown(html: &str) -> (Option<String>, String) {
    let lower = html.to_lowercase();
    let title = if let (Some(start), Some(end)) = (lower.find("<title>"), lower.find("</title>")) {
        if start < end {
            let title_start = start + 7;
            Some(html[title_start..end].trim().to_string())
        } else {
            None
        }
    } else {
        None
    };

    let content = convert(html).unwrap_or_else(|_| html.to_string());
    (title, content)
}

fn truncate_str(content: &str, max_len: usize) -> String {
    if content.len() <= max_len {
        return content.to_string();
    }

    let truncated = &content[..max_len];

    if let Some(last_para) = truncated.rfind("\n\n") {
        format!("{}\n\n[Content truncated...]", &truncated[..last_para])
    } else if let Some(last_newline) = truncated.rfind('\n') {
        format!("{}\n\n[Content truncated...]", &truncated[..last_newline])
    } else {
        format!("{truncated}\n\n[Content truncated...]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_url_adds_https() {
        assert_eq!(
            normalize_url("example.com").unwrap(),
            "https://example.com/"
        );
    }

    #[test]
    fn test_normalize_url_upgrades_http() {
        assert_eq!(
            normalize_url("http://example.com").unwrap(),
            "https://example.com/"
        );
    }

    #[test]
    fn test_normalize_url_preserves_https() {
        assert_eq!(
            normalize_url("https://example.com").unwrap(),
            "https://example.com/"
        );
    }

    #[test]
    fn test_normalize_url_invalid() {
        assert!(normalize_url("not a valid url!!!").is_err());
    }

    #[test]
    fn test_html_to_markdown_with_title() {
        let html = "<html><head><title>Test Page</title></head><body></body></html>";
        let (title, _) = html_to_markdown(html);
        assert_eq!(title, Some("Test Page".to_string()));
    }

    #[test]
    fn test_html_to_markdown_missing_title() {
        let html = "<html><head></head><body></body></html>";
        let (title, _) = html_to_markdown(html);
        assert_eq!(title, None);
    }

    #[test]
    fn test_truncate_markdown_short() {
        let content = "Short content";
        assert_eq!(truncate_str(content, 100), "Short content");
    }

    #[test]
    fn test_truncate_markdown_at_paragraph() {
        let content = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
        let result = truncate_str(content, 35);
        assert!(result.contains("First paragraph."));
        assert!(result.contains("[Content truncated...]"));
        assert!(!result.contains("Third paragraph"));
    }

    #[test]
    fn test_html_to_markdown_basic() {
        let html = "<h1>Title</h1><p>Content paragraph.</p>";
        let (_, markdown) = html_to_markdown(html);
        assert!(markdown.contains("Title"));
        assert!(markdown.contains("Content paragraph"));
    }

    #[tokio::test]
    async fn test_fake_client_returns_configured_response() {
        let fake = FakeHttpClient::new().with_html(
            "https://example.com/",
            "<html><head><title>Example</title></head><body><h1>Hello</h1></body></html>",
        );

        let fetcher = WebFetcher::with_client(fake.clone());
        let result = fetcher
            .fetch(WebFetchInput {
                url: "example.com".to_string(),
                prompt: None,
                timeout: None,
            })
            .await
            .unwrap();

        assert_eq!(result.status_code, 200);
        assert_eq!(result.title, Some("Example".to_string()));
        assert!(result.content.contains("Hello"));
        assert_eq!(fake.fetch_count(), 1);
    }

    #[tokio::test]
    async fn test_fake_client_tracks_fetch_history() {
        let fake = FakeHttpClient::new()
            .with_html("https://example.com/page1", "<h1>Page 1</h1>")
            .with_html("https://example.com/page2", "<h1>Page 2</h1>");

        let fetcher = WebFetcher::with_client(fake.clone());

        fetcher
            .fetch(WebFetchInput {
                url: "https://example.com/page1".to_string(),
                prompt: None,
                timeout: None,
            })
            .await
            .unwrap();

        fetcher
            .fetch(WebFetchInput {
                url: "https://example.com/page2".to_string(),
                prompt: None,
                timeout: None,
            })
            .await
            .unwrap();

        assert_eq!(fake.fetch_count(), 2);
        assert_eq!(
            fake.fetched_urls(),
            vec![
                "https://example.com/page1".to_string(),
                "https://example.com/page2".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn test_fake_client_missing_url_returns_error() {
        let fake = FakeHttpClient::new();
        let fetcher = WebFetcher::with_client(fake);

        let result = fetcher
            .fetch(WebFetchInput {
                url: "https://not-configured.com/".to_string(),
                prompt: None,
                timeout: None,
            })
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fake_client_with_default_response() {
        let fake = FakeHttpClient::new().with_default(HttpResponse {
            final_url: "https://fallback.com/".to_string(),
            status_code: 404,
            body: "<h1>Not Found</h1>".to_string(),
        });

        let fetcher = WebFetcher::with_client(fake);
        let result = fetcher
            .fetch(WebFetchInput {
                url: "https://any-url.com/".to_string(),
                prompt: None,
                timeout: None,
            })
            .await
            .unwrap();

        assert_eq!(result.status_code, 404);
    }
}
