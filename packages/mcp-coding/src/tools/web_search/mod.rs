pub mod search_client;

pub use search_client::{BraveSearchClient, RawSearchResult, SearchClient, SearchParams};

#[cfg(test)]
pub use search_client::FakeSearchClient;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::display_meta::ToolDisplayMeta;
use crate::error::WebSearchError;

const DEFAULT_COUNT: u32 = 10;
const MAX_COUNT: u32 = 20;

/// Input parameters for web_search tool
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchInput {
    /// The search query
    pub query: String,

    /// Maximum number of results (default: 10, max: 20)
    pub count: Option<u32>,

    /// Only include results from these domains
    pub allowed_domains: Option<Vec<String>>,

    /// Exclude results from these domains
    pub blocked_domains: Option<Vec<String>>,
}

/// Output from web_search tool
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchOutput {
    /// Search results
    pub results: Vec<SearchResult>,

    /// The original query
    pub query: String,

    /// Display metadata for human-friendly rendering
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _meta: Option<serde_json::Value>,
}

/// Individual search result
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    /// Result title
    pub title: String,

    /// Result URL
    pub url: String,

    /// Result snippet/description
    pub snippet: String,
}

/// Web searcher that performs searches and filters results
#[derive(Debug, Clone)]
pub struct WebSearcher<C: SearchClient> {
    client: C,
}

impl<C: SearchClient> WebSearcher<C> {
    /// Creates a new WebSearcher with the given client
    pub fn with_client(client: C) -> Self {
        Self { client }
    }

    /// Performs a web search with the given parameters
    pub async fn search(&self, args: WebSearchInput) -> Result<WebSearchOutput, WebSearchError> {
        let query = args.query.trim();

        if query.is_empty() {
            return Err(WebSearchError::InvalidQuery(
                "Search query cannot be empty".to_string(),
            ));
        }

        let count = args.count.unwrap_or(DEFAULT_COUNT).min(MAX_COUNT);

        let params = SearchParams {
            query: query.to_string(),
            count,
        };

        let mut results = self.client.search(params).await?;

        // Ensure we don't return more than requested
        if results.len() > count as usize {
            results.truncate(count as usize);
        }

        // Apply domain filters if provided
        if let Some(ref allowed) = args.allowed_domains {
            results = filter_allowed_domains(results, allowed);
        }

        if let Some(ref blocked) = args.blocked_domains {
            results = filter_blocked_domains(results, blocked);
        }

        // Convert to output format
        let results: Vec<SearchResult> = results
            .into_iter()
            .map(|r| SearchResult {
                title: r.title,
                url: r.url.clone(),
                snippet: r.description,
            })
            .collect();

        let display_meta = ToolDisplayMeta::web_search(query.to_string(), results.len());

        Ok(WebSearchOutput {
            results,
            query: query.to_string(),
            _meta: display_meta.into_meta(),
        })
    }
}

impl WebSearcher<BraveSearchClient> {
    /// Creates a new WebSearcher with Brave Search API
    pub fn try_new() -> Result<Self, WebSearchError> {
        let client = BraveSearchClient::new()?;
        Ok(Self::with_client(client))
    }

    /// Creates a new WebSearcher with a custom Brave API key
    pub fn with_api_key(api_key: String) -> Self {
        let client = BraveSearchClient::with_api_key(api_key);
        Self::with_client(client)
    }
}

impl Default for WebSearcher<BraveSearchClient> {
    fn default() -> Self {
        Self::with_api_key(
            std::env::var("BRAVE_SEARCH_API_KEY").expect("BRAVE_SEARCH_API_KEY must be set"),
        )
    }
}

fn filter_allowed_domains(
    results: Vec<RawSearchResult>,
    allowed_domains: &[String],
) -> Vec<RawSearchResult> {
    let allowed_set: HashSet<String> = allowed_domains.iter().cloned().collect();

    results
        .into_iter()
        .filter(|r| {
            if let Some(host) = extract_domain(&r.url) {
                return allowed_set.iter().any(|domain| host.ends_with(domain));
            }
            false
        })
        .collect()
}

fn filter_blocked_domains(
    results: Vec<RawSearchResult>,
    blocked_domains: &[String],
) -> Vec<RawSearchResult> {
    let blocked_set: HashSet<String> = blocked_domains.iter().cloned().collect();

    results
        .into_iter()
        .filter(|r| {
            if let Some(host) = extract_domain(&r.url) {
                return !blocked_set.iter().any(|domain| host.ends_with(domain));
            }
            true
        })
        .collect()
}

/// Extract domain/hostname from a URL
fn extract_domain(url: &str) -> Option<String> {
    // Remove protocol
    let url = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);

    // Remove path and query string
    let domain = url.split('/').next()?;

    // Remove port if present
    let domain = domain.split(':').next()?;

    Some(domain.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_count_limit() {
        let args = WebSearchInput {
            query: "test".to_string(),
            count: Some(100),
            allowed_domains: None,
            blocked_domains: None,
        };
        let _ = args; // Just ensure it compiles
    }

    fn make_result(url: &str) -> RawSearchResult {
        RawSearchResult {
            title: "Test".to_string(),
            url: url.to_string(),
            description: "Test description".to_string(),
        }
    }

    #[test]
    fn test_filter_allowed_domains() {
        let results = vec![
            make_result("https://example.com/page"),
            make_result("https://test.org/page"),
            make_result("https://sub.example.com/page"),
        ];

        let filtered = filter_allowed_domains(results, &["example.com".to_string()]);

        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().any(|r| r.url.contains("example.com")));
    }

    #[test]
    fn test_filter_blocked_domains() {
        let results = vec![
            make_result("https://example.com/page"),
            make_result("https://test.org/page"),
            make_result("https://allowed.com/page"),
        ];

        let filtered = filter_blocked_domains(results, &["example.com".to_string()]);

        assert_eq!(filtered.len(), 2);
        assert!(!filtered.iter().any(|r| r.url.contains("example.com")));
    }

    #[test]
    fn test_filter_allowed_and_blocked() {
        let results = vec![
            make_result("https://example.com/page"),
            make_result("https://test.org/page"),
            make_result("https://allowed.com/page"),
        ];

        let filtered = filter_allowed_domains(
            results,
            &["example.com".to_string(), "test.org".to_string()],
        );
        let filtered = filter_blocked_domains(filtered, &["test.org".to_string()]);

        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].url.contains("example.com"));
    }

    #[tokio::test]
    async fn test_empty_query_returns_error() {
        let fake = FakeSearchClient::new();
        let searcher = WebSearcher::with_client(fake);

        let result = searcher
            .search(WebSearchInput {
                query: "   ".to_string(),
                count: None,
                allowed_domains: None,
                blocked_domains: None,
            })
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            WebSearchError::InvalidQuery(_)
        ));
    }

    #[tokio::test]
    async fn test_count_limiting() {
        let fake = FakeSearchClient::new().with_results(
            "test query",
            (0..30)
                .map(|i| RawSearchResult {
                    title: format!("Result {i}"),
                    url: format!("https://example.com/{i}"),
                    description: format!("Description {i}"),
                })
                .collect(),
        );

        let searcher = WebSearcher::with_client(fake);

        let output = searcher
            .search(WebSearchInput {
                query: "test query".to_string(),
                count: Some(15),
                allowed_domains: None,
                blocked_domains: None,
            })
            .await
            .unwrap();

        assert_eq!(output.results.len(), 15);
    }

    #[tokio::test]
    async fn test_domain_filtering_in_web_searcher() {
        let fake = FakeSearchClient::new().with_results(
            "test query",
            vec![
                make_result("https://example.com/page1"),
                make_result("https://blocked.com/page2"),
                make_result("https://example.com/page3"),
            ],
        );

        let searcher = WebSearcher::with_client(fake);

        let output = searcher
            .search(WebSearchInput {
                query: "test query".to_string(),
                count: None,
                allowed_domains: None,
                blocked_domains: Some(vec!["blocked.com".to_string()]),
            })
            .await
            .unwrap();

        assert_eq!(output.results.len(), 2);
        assert!(!output.results.iter().any(|r| r.url.contains("blocked.com")));
    }

    #[tokio::test]
    async fn test_searcher_tracks_results() {
        let fake = FakeSearchClient::new().with_results(
            "rust programming",
            vec![RawSearchResult {
                title: "Rust Language".to_string(),
                url: "https://rust-lang.org".to_string(),
                description: "A systems language".to_string(),
            }],
        );

        let searcher = WebSearcher::with_client(fake);

        let output = searcher
            .search(WebSearchInput {
                query: "rust programming".to_string(),
                count: None,
                allowed_domains: None,
                blocked_domains: None,
            })
            .await
            .unwrap();

        assert_eq!(output.results.len(), 1);
        assert_eq!(output.results[0].title, "Rust Language");
        assert_eq!(output.results[0].snippet, "A systems language");
    }
}
