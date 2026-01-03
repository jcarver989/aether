//! Integration tests for web search tool

use mcp_lexicon::coding::tools::web_search::{
    search_client::{FakeSearchClient, RawSearchResult},
    WebSearchInput, WebSearcher,
};

/// Test with domain filtering
#[tokio::test]
async fn test_search_with_domain_filter() {
    let fake = FakeSearchClient::new().with_results(
        "test query",
        vec![
            RawSearchResult {
                title: "Example Page".to_string(),
                url: "https://example.com/page".to_string(),
                description: "Example description".to_string(),
            },
            RawSearchResult {
                title: "Blocked Page".to_string(),
                url: "https://blocked.com/page".to_string(),
                description: "Blocked description".to_string(),
            },
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

    assert_eq!(output.results.len(), 1);
    assert!(output.results[0].title == "Example Page");
}

/// Test that search results are properly formatted
#[tokio::test]
async fn test_search_result_formatting() {
    let fake = FakeSearchClient::new().with_results(
        "formatting test",
        vec![RawSearchResult {
            title: "Test Title".to_string(),
            url: "https://test.com/page".to_string(),
            description: "Test description with <b>HTML</b> tags".to_string(),
        }],
    );

    let searcher = WebSearcher::with_client(fake);

    let output = searcher
        .search(WebSearchInput {
            query: "formatting test".to_string(),
            count: None,
            allowed_domains: None,
            blocked_domains: None,
        })
        .await
        .unwrap();

    assert_eq!(output.results.len(), 1);
    assert_eq!(output.results[0].title, "Test Title");
    assert_eq!(output.results[0].url, "https://test.com/page");
    // The description should be passed through as-is
    assert!(output.results[0].snippet.contains("HTML"));
}

/// Test that count parameter is respected
#[tokio::test]
async fn test_search_respects_count_parameter() {
    let fake = FakeSearchClient::new().with_results(
        "count test",
        (0..100)
            .map(|i| RawSearchResult {
                title: format!("Result {}", i),
                url: format!("https://example.com/{}", i),
                description: format!("Description {}", i),
            })
            .collect(),
    );

    let searcher = WebSearcher::with_client(fake);

    // Request only 15 results
    let output = searcher
        .search(WebSearchInput {
            query: "count test".to_string(),
            count: Some(15),
            allowed_domains: None,
            blocked_domains: None,
        })
        .await
        .unwrap();

    assert_eq!(output.results.len(), 15);
}

/// Test that allowed_domains filter works
#[tokio::test]
async fn test_search_allowed_domains() {
    let fake = FakeSearchClient::new().with_results(
        "domain test",
        vec![
            RawSearchResult {
                title: "Example Page".to_string(),
                url: "https://example.com/page".to_string(),
                description: "Example".to_string(),
            },
            RawSearchResult {
                title: "Other Page".to_string(),
                url: "https://other.com/page".to_string(),
                description: "Other".to_string(),
            },
        ],
    );

    let searcher = WebSearcher::with_client(fake);

    let output = searcher
        .search(WebSearchInput {
            query: "domain test".to_string(),
            count: None,
            allowed_domains: Some(vec!["example.com".to_string()]),
            blocked_domains: None,
        })
        .await
        .unwrap();

    assert_eq!(output.results.len(), 1);
    assert!(output.results[0].url.contains("example.com"));
}

/// Test that empty query returns error
#[tokio::test]
async fn test_search_empty_query_error() {
    let fake = FakeSearchClient::new();
    let searcher = WebSearcher::with_client(fake);

    let result = searcher
        .search(WebSearchInput {
            query: "".to_string(),
            count: None,
            allowed_domains: None,
            blocked_domains: None,
        })
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("empty"));
}
