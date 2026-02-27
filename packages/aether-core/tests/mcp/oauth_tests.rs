use aether_core::mcp::{McpSpawnResult, mcp};
use futures::future::BoxFuture;
use mcp_utils::client::oauth::{BrowserOAuthHandler, OAuthCallback, OAuthError, OAuthHandler};
use mcp_utils::client::{ElicitationRequest, McpServerConfig};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use std::sync::Arc;
use tokio::sync::mpsc;

struct FakeOAuthHandler {
    callback: OAuthCallback,
    redirect_uri: String,
}

impl FakeOAuthHandler {
    fn new(code: &str, state: &str) -> Self {
        Self {
            callback: OAuthCallback {
                code: code.to_string(),
                state: state.to_string(),
            },
            redirect_uri: "http://127.0.0.1:0/oauth2callback".to_string(),
        }
    }
}

impl OAuthHandler for FakeOAuthHandler {
    fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    fn authorize(&self, _auth_url: &str) -> BoxFuture<'_, Result<OAuthCallback, OAuthError>> {
        let callback = self.callback.clone();
        Box::pin(async move { Ok(callback) })
    }
}

struct CancellingOAuthHandler;

impl OAuthHandler for CancellingOAuthHandler {
    fn redirect_uri(&self) -> &str {
        "http://127.0.0.1:0/oauth2callback"
    }

    fn authorize(&self, _auth_url: &str) -> BoxFuture<'_, Result<OAuthCallback, OAuthError>> {
        Box::pin(async { Err(OAuthError::UserCancelled) })
    }
}

#[tokio::test]
async fn fake_oauth_handler_returns_configured_callback() {
    let handler = FakeOAuthHandler::new("test_code", "test_state");
    let result = handler.authorize("https://example.com/auth").await;
    let callback = result.unwrap();
    assert_eq!(callback.code, "test_code");
    assert_eq!(callback.state, "test_state");
}

#[tokio::test]
async fn cancelling_handler_returns_user_cancelled() {
    let handler = CancellingOAuthHandler;
    let result = handler.authorize("https://example.com/auth").await;
    assert!(matches!(result, Err(OAuthError::UserCancelled)));
}

#[tokio::test]
async fn builder_with_oauth_handler_spawns_successfully() {
    let handler = FakeOAuthHandler::new("code", "state");

    let McpSpawnResult {
        tool_definitions,
        instructions,
        elicitation_rx: _,
        ..
    } = mcp()
        .with_oauth_handler(handler)
        .with_servers(vec![])
        .spawn()
        .await
        .unwrap();

    assert!(tool_definitions.is_empty());
    assert!(instructions.is_empty());
}

#[tokio::test]
async fn http_server_without_handler_returns_error() {
    let (elicitation_tx, _elicitation_rx) = mpsc::channel::<ElicitationRequest>(50);
    let mut manager = mcp_utils::client::McpManager::new(elicitation_tx, None);

    let config = StreamableHttpClientTransportConfig {
        uri: "http://localhost:19999/mcp".into(),
        ..Default::default()
    };
    let result = manager
        .add_mcp(McpServerConfig::Http {
            name: "test_server".to_string(),
            config,
        })
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn http_server_with_handler_attempts_oauth_on_failure() {
    let handler = FakeOAuthHandler::new("test_code", "test_state");
    let (elicitation_tx, _elicitation_rx) = mpsc::channel::<ElicitationRequest>(50);
    let mut manager = mcp_utils::client::McpManager::new(elicitation_tx, Some(Arc::new(handler)));

    let config = StreamableHttpClientTransportConfig {
        uri: "http://localhost:19999/mcp".into(),
        ..Default::default()
    };
    let result = manager
        .add_mcp(McpServerConfig::Http {
            name: "test_oauth_server".to_string(),
            config,
        })
        .await;

    // Error should indicate OAuth was attempted, not just a plain connection failure
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("OAuth"),
        "Expected OAuth-related error, got: {err_msg}"
    );
}

#[tokio::test]
async fn add_mcps_continues_on_oauth_failure() {
    let handler = FakeOAuthHandler::new("code", "state");
    let (elicitation_tx, _elicitation_rx) = mpsc::channel::<ElicitationRequest>(50);
    let mut manager = mcp_utils::client::McpManager::new(elicitation_tx, Some(Arc::new(handler)));

    let configs = vec![
        McpServerConfig::Http {
            name: "failing_server_1".to_string(),
            config: StreamableHttpClientTransportConfig {
                uri: "http://localhost:19998/mcp".into(),
                ..Default::default()
            },
        },
        McpServerConfig::Http {
            name: "failing_server_2".to_string(),
            config: StreamableHttpClientTransportConfig {
                uri: "http://localhost:19997/mcp".into(),
                ..Default::default()
            },
        },
    ];

    let result = manager.add_mcps(configs).await;
    assert!(result.is_ok());
    assert!(manager.tool_definitions().is_empty());
}

#[tokio::test]
async fn browser_oauth_handler_callback_server() {
    let handler = BrowserOAuthHandler::new().unwrap();
    let callback_url = format!("{}?code=abc123&state=csrf_token", handler.redirect_uri());

    let handle = tokio::spawn(async move { handler.authorize("https://example.com/auth").await });

    // Give the callback server time to start
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let _response = client
        .get(&callback_url)
        .send()
        .await
        .expect("Failed to send callback request");

    let result = handle.await.unwrap();
    let callback = result.unwrap();
    assert_eq!(callback.code, "abc123");
    assert_eq!(callback.state, "csrf_token");
}

#[tokio::test]
async fn oauth_handler_is_dyn_compatible() {
    let handler: Arc<dyn OAuthHandler> = Arc::new(FakeOAuthHandler::new("code", "state"));
    let result = handler.authorize("https://example.com").await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().code, "code");
}
