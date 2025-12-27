use mcp_lexicon::coding::error::WebFetchError;
use mcp_lexicon::coding::tools::web_fetch::{WebFetchInput, WebFetcher};

#[tokio::test]
async fn test_fetch_real_page() {
    let fetcher = WebFetcher::new();
    let result = fetcher
        .fetch(WebFetchInput {
            url: "https://httpbin.org/html".to_string(),
            prompt: None,
            timeout: Some(10_000),
        })
        .await
        .unwrap();

    assert_eq!(result.status_code, 200);
    assert!(!result.content.is_empty());
    assert!(!result.truncated);
    // httpbin.org/html returns a simple HTML page with Herman Melville text
    assert!(result.content.contains("Melville") || result.content.contains("Moby"));
}

#[tokio::test]
async fn test_fetch_with_redirect() {
    let fetcher = WebFetcher::new();
    // httpbin.org/redirect-to redirects to another page
    let result = fetcher
        .fetch(WebFetchInput {
            url: "https://httpbin.org/redirect-to?url=https://httpbin.org/html".to_string(),
            prompt: None,
            timeout: Some(10_000),
        })
        .await
        .unwrap();

    assert_eq!(result.status_code, 200);
    // Final URL should be the redirected destination
    assert!(result.final_url.contains("httpbin.org/html"));
}

#[tokio::test]
async fn test_fetch_http_upgrades_to_https() {
    let fetcher = WebFetcher::new();
    let result = fetcher
        .fetch(WebFetchInput {
            url: "http://httpbin.org/html".to_string(),
            prompt: None,
            timeout: Some(10_000),
        })
        .await
        .unwrap();

    // Should have upgraded to HTTPS
    assert!(result.final_url.starts_with("https://"));
}

#[tokio::test]
async fn test_fetch_timeout() {
    let fetcher = WebFetcher::new();
    let result = fetcher
        .fetch(WebFetchInput {
            url: "https://httpbin.org/delay/5".to_string(),
            prompt: None,
            timeout: Some(1000), // 1 second timeout, but page takes 5 seconds
        })
        .await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), WebFetchError::Timeout(_)));
}

#[tokio::test]
async fn test_fetch_invalid_url() {
    let fetcher = WebFetcher::new();
    // Use a URL with invalid characters that can't be parsed
    let result = fetcher
        .fetch(WebFetchInput {
            url: "https://[invalid".to_string(),
            prompt: None,
            timeout: None,
        })
        .await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), WebFetchError::InvalidUrl(_)));
}

#[tokio::test]
async fn test_fetch_with_prompt() {
    let fetcher = WebFetcher::new();
    // The prompt is currently just for documentation, but we should handle it gracefully
    let result = fetcher
        .fetch(WebFetchInput {
            url: "https://httpbin.org/html".to_string(),
            prompt: Some("Extract the main heading".to_string()),
            timeout: Some(10_000),
        })
        .await
        .unwrap();

    assert_eq!(result.status_code, 200);
    assert!(!result.content.is_empty());
}

#[tokio::test]
async fn test_fetch_non_existent_host() {
    let fetcher = WebFetcher::new();
    let result = fetcher
        .fetch(WebFetchInput {
            url: "https://this-domain-definitely-does-not-exist-12345.com".to_string(),
            prompt: None,
            timeout: Some(5000),
        })
        .await;

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WebFetchError::RequestFailed(_)
    ));
}

#[tokio::test]
async fn test_fetcher_reusable() {
    // Test that a single WebFetcher can be reused for multiple requests
    let fetcher = WebFetcher::new();

    let result1 = fetcher
        .fetch(WebFetchInput {
            url: "https://httpbin.org/html".to_string(),
            prompt: None,
            timeout: Some(10_000),
        })
        .await
        .unwrap();

    let result2 = fetcher
        .fetch(WebFetchInput {
            url: "https://httpbin.org/robots.txt".to_string(),
            prompt: None,
            timeout: Some(10_000),
        })
        .await
        .unwrap();

    assert_eq!(result1.status_code, 200);
    assert_eq!(result2.status_code, 200);
}
