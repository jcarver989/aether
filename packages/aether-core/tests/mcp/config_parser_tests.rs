use mcp_utils::client::{McpServerConfig, ParseError, RawMcpConfig, ServerConfig};
use std::collections::HashMap;
use std::env;

async fn parse_configs(json: &str) -> Result<Vec<McpServerConfig>, ParseError> {
    RawMcpConfig::from_json(json)
        .unwrap()
        .into_configs(&HashMap::new())
        .await
}

async fn parse_one(json: &str) -> McpServerConfig {
    let configs = parse_configs(json).await.unwrap();
    assert_eq!(configs.len(), 1);
    configs.into_iter().next().unwrap()
}

fn server_json(name: &str, body: &str) -> String {
    format!(r#"{{ "servers": {{ "{name}": {body} }} }}"#)
}

macro_rules! with_env {
    ([$( ($k:expr, $v:expr) ),+ $(,)?], $body:expr) => {{
        unsafe { $( env::set_var($k, $v); )+ }
        let _result = $body;
        unsafe { $( env::remove_var($k); )+ }
        _result
    }};
}

fn assert_http(
    config: McpServerConfig,
    expected_name: &str,
    expected_url: &str,
) -> McpServerConfig {
    match &config {
        McpServerConfig::Server(ServerConfig::Http { name, config: c }) => {
            assert_eq!(name, expected_name);
            assert_eq!(c.uri.to_string(), expected_url);
        }
        other => panic!("Expected Http config, got {other:?}"),
    }
    config
}

#[tokio::test]
async fn test_parse_stdio_config() {
    let json = server_json(
        "githubMcp",
        r#"{
            "type": "stdio",
            "command": "npx",
            "args": ["-y", "@modelcontextprotocol/server-github"],
            "env": { "GITHUB_TOKEN": "$GITHUB_TOKEN" }
        }"#,
    );
    with_env!([("GITHUB_TOKEN", "test_token")], {
        match parse_one(&json).await {
            McpServerConfig::Server(ServerConfig::Stdio {
                name,
                command,
                args,
                env,
            }) => {
                assert_eq!(name, "githubMcp");
                assert_eq!(command, "npx");
                assert_eq!(args, vec!["-y", "@modelcontextprotocol/server-github"]);
                assert_eq!(env.get("GITHUB_TOKEN").unwrap(), "test_token");
            }
            other => panic!("Expected Stdio config, got {other:?}"),
        }
    });
}

#[tokio::test]
async fn test_parse_http_and_sse_configs() {
    // HTTP with auth header
    let json = server_json(
        "mcpMesh",
        r#"{
            "type": "http",
            "url": "http://localhost:3000/mcp",
            "headers": { "Authorization": "Bearer $API_TOKEN" }
        }"#,
    );
    let cfg = with_env!(
        [("API_TOKEN", "secret_token")],
        assert_http(
            parse_one(&json).await,
            "mcpMesh",
            "http://localhost:3000/mcp"
        )
    );
    if let McpServerConfig::Server(ServerConfig::Http { config: c, .. }) = cfg {
        assert_eq!(c.auth_header.as_ref().unwrap(), "Bearer secret_token");
    }

    // SSE maps to HTTP internally
    let json = server_json(
        "sseServer",
        r#"{ "type": "sse", "url": "http://localhost:4000/sse", "headers": {} }"#,
    );
    assert_http(
        parse_one(&json).await,
        "sseServer",
        "http://localhost:4000/sse",
    );
}

#[tokio::test]
async fn test_missing_env_var_error() {
    let json = server_json(
        "test",
        r#"{ "type": "stdio", "command": "$MISSING_VAR", "args": [] }"#,
    );
    match parse_configs(&json).await.unwrap_err() {
        ParseError::VarError(_) => (),
        other => panic!("Expected VarError, got {other:?}"),
    }
}

#[tokio::test]
async fn test_factory_not_found_error() {
    let json = server_json("test", r#"{ "type": "in-memory" }"#);
    match parse_configs(&json).await.unwrap_err() {
        ParseError::FactoryNotFound(name) => assert_eq!(name, "test"),
        other => panic!("Expected FactoryNotFound, got {other:?}"),
    }
}

#[tokio::test]
async fn test_multiple_servers() {
    let json = r#"{
        "servers": {
            "server1": { "type": "stdio", "command": "node", "args": ["server.js"] },
            "server2": {
                "type": "http",
                "url": "http://localhost:3000/mcp",
                "headers": { "Authorization": "$TOKEN" }
            }
        }
    }"#;
    with_env!([("TOKEN", "test")], {
        assert_eq!(parse_configs(json).await.unwrap().len(), 2);
    });
}

#[tokio::test]
async fn test_env_var_in_url() {
    let json = server_json(
        "test",
        r#"{ "type": "http", "url": "http://${HOST}:${PORT}/mcp" }"#,
    );
    with_env!([("HOST", "localhost"), ("PORT", "8080")], {
        assert_http(parse_one(&json).await, "test", "http://localhost:8080/mcp");
    });
}

#[tokio::test]
async fn test_parse_tool_proxy_config() {
    let json = r#"{
        "servers": {
            "proxy": {
                "type": "in-memory",
                "input": {
                    "servers": {
                        "github": {
                            "type": "stdio",
                            "command": "npx",
                            "args": ["-y", "@modelcontextprotocol/server-github"]
                        },
                        "sentry": { "type": "http", "url": "https://sentry.example.com/mcp" }
                    }
                }
            }
        }
    }"#;
    match parse_one(json).await {
        McpServerConfig::ToolProxy { name, servers } => {
            assert_eq!(name, "proxy");
            assert_eq!(servers.len(), 2);
            assert!(
                servers
                    .iter()
                    .any(|s| matches!(s, ServerConfig::Stdio { .. }))
            );
            assert!(
                servers
                    .iter()
                    .any(|s| matches!(s, ServerConfig::Http { .. }))
            );
        }
        other => panic!("Expected ToolProxy config, got {other:?}"),
    }
}

#[tokio::test]
async fn test_tool_proxy_rejects_nested_in_memory() {
    let cases = [
        (
            "bad",
            server_json(
                "outer",
                r#"{ "type": "in-memory", "input": { "servers": { "bad": { "type": "in-memory" } } } }"#,
            ),
        ),
        (
            "inner",
            server_json(
                "outer",
                r#"{ "type": "in-memory", "input": { "servers": { "inner": { "type": "in-memory", "input": { "servers": {} } } } } }"#,
            ),
        ),
    ];
    for (expected_name, json) in &cases {
        match parse_configs(json).await.unwrap_err() {
            ParseError::InvalidNestedConfig(msg) => {
                assert!(
                    msg.contains("in-memory"),
                    "msg should mention in-memory: {msg}"
                );
                assert!(
                    msg.contains(expected_name),
                    "msg should mention {expected_name}: {msg}"
                );
            }
            other => panic!("Expected InvalidNestedConfig, got {other:?}"),
        }
    }
}
