use mcp_utils::client::{McpServerConfig, ParseError, RawMcpConfig, ServerConfig};
use std::collections::HashMap;
use std::env;

#[tokio::test]
async fn test_parse_stdio_config() {
    unsafe { env::set_var("GITHUB_TOKEN", "test_token") };

    let json = r#"
    {
        "servers": {
            "githubMcp": {
                "type": "stdio",
                "command": "npx",
                "args": ["-y", "@modelcontextprotocol/server-github"],
                "env": {
                    "GITHUB_TOKEN": "$GITHUB_TOKEN"
                }
            }
        }
    }
    "#;

    let raw_config = RawMcpConfig::from_json(json).unwrap();
    let configs = raw_config.into_configs(&HashMap::new()).await.unwrap();

    assert_eq!(configs.len(), 1);
    match &configs[0] {
        McpServerConfig::Server(ServerConfig::Stdio {
            name,
            command,
            args,
            env,
        }) => {
            assert_eq!(name, "githubMcp");
            assert_eq!(command, "npx");
            assert_eq!(args.len(), 2);
            assert_eq!(args[0], "-y");
            assert_eq!(args[1], "@modelcontextprotocol/server-github");
            assert_eq!(env.get("GITHUB_TOKEN").unwrap(), "test_token");
        }
        _ => panic!("Expected Stdio config"),
    }

    unsafe { env::remove_var("GITHUB_TOKEN") };
}

#[tokio::test]
async fn test_parse_http_config() {
    unsafe { env::set_var("API_TOKEN", "secret_token") };

    let json = r#"
    {
        "servers": {
            "mcpMesh": {
                "type": "http",
                "url": "http://localhost:3000/mcp",
                "headers": {
                    "Authorization": "Bearer $API_TOKEN"
                }
            }
        }
    }
    "#;

    let raw_config = RawMcpConfig::from_json(json).unwrap();
    let configs = raw_config.into_configs(&HashMap::new()).await.unwrap();

    assert_eq!(configs.len(), 1);
    match &configs[0] {
        McpServerConfig::Server(ServerConfig::Http { name, config }) => {
            assert_eq!(name, "mcpMesh");
            assert_eq!(config.uri.to_string(), "http://localhost:3000/mcp");
            assert_eq!(config.auth_header.as_ref().unwrap(), "Bearer secret_token");
        }
        _ => panic!("Expected Http config"),
    }

    unsafe { env::remove_var("API_TOKEN") };
}

#[tokio::test]
async fn test_parse_sse_config() {
    let json = r#"
    {
        "servers": {
            "sseServer": {
                "type": "sse",
                "url": "http://localhost:4000/sse",
                "headers": {}
            }
        }
    }
    "#;

    let raw_config = RawMcpConfig::from_json(json).unwrap();
    let configs = raw_config.into_configs(&HashMap::new()).await.unwrap();

    assert_eq!(configs.len(), 1);
    // SSE maps to HTTP internally
    match &configs[0] {
        McpServerConfig::Server(ServerConfig::Http { name, config }) => {
            assert_eq!(name, "sseServer");
            assert_eq!(config.uri.to_string(), "http://localhost:4000/sse");
        }
        _ => panic!("Expected Http config"),
    }
}

// Note: InMemory server testing requires complex setup with tool_handler macros
// and is better tested in integration tests with actual server implementations.
// Skipping this test for now as it requires too much boilerplate.

#[tokio::test]
async fn test_missing_env_var_error() {
    let json = r#"
    {
        "servers": {
            "test": {
                "type": "stdio",
                "command": "$MISSING_VAR",
                "args": []
            }
        }
    }
    "#;

    let raw_config = RawMcpConfig::from_json(json).unwrap();
    let result = raw_config.into_configs(&HashMap::new()).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ParseError::VarError(_) => (),
        _ => panic!("Expected VarError"),
    }
}

#[tokio::test]
async fn test_factory_not_found_error() {
    let json = r#"
    {
        "servers": {
            "test": {
                "type": "in-memory"
            }
        }
    }
    "#;

    let raw_config = RawMcpConfig::from_json(json).unwrap();
    let result = raw_config.into_configs(&HashMap::new()).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ParseError::FactoryNotFound(name) => {
            assert_eq!(name, "test");
        }
        _ => panic!("Expected FactoryNotFound"),
    }
}

#[tokio::test]
async fn test_multiple_servers() {
    unsafe { env::set_var("TOKEN", "test") };

    let json = r#"
    {
        "servers": {
            "server1": {
                "type": "stdio",
                "command": "node",
                "args": ["server.js"]
            },
            "server2": {
                "type": "http",
                "url": "http://localhost:3000/mcp",
                "headers": {
                    "Authorization": "$TOKEN"
                }
            }
        }
    }
    "#;

    let raw_config = RawMcpConfig::from_json(json).unwrap();
    let configs = raw_config.into_configs(&HashMap::new()).await.unwrap();

    assert_eq!(configs.len(), 2);

    unsafe { env::remove_var("TOKEN") };
}

#[tokio::test]
async fn test_env_var_in_url() {
    unsafe {
        env::set_var("HOST", "localhost");
        env::set_var("PORT", "8080");
    }

    let json = r#"
    {
        "servers": {
            "test": {
                "type": "http",
                "url": "http://${HOST}:${PORT}/mcp"
            }
        }
    }
    "#;

    let raw_config = RawMcpConfig::from_json(json).unwrap();
    let configs = raw_config.into_configs(&HashMap::new()).await.unwrap();

    match &configs[0] {
        McpServerConfig::Server(ServerConfig::Http { config, .. }) => {
            assert_eq!(config.uri.to_string(), "http://localhost:8080/mcp");
        }
        _ => panic!("Expected Http config"),
    }

    unsafe {
        env::remove_var("HOST");
        env::remove_var("PORT");
    }
}

#[tokio::test]
async fn test_parse_tool_proxy_config() {
    let json = r#"
    {
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
                        "sentry": {
                            "type": "http",
                            "url": "https://sentry.example.com/mcp"
                        }
                    }
                }
            }
        }
    }
    "#;

    let raw_config = RawMcpConfig::from_json(json).unwrap();
    let configs = raw_config.into_configs(&HashMap::new()).await.unwrap();

    assert_eq!(configs.len(), 1);
    match &configs[0] {
        McpServerConfig::ToolProxy { name, servers } => {
            assert_eq!(name, "proxy");
            assert_eq!(servers.len(), 2);

            // Verify nested servers are properly parsed
            let has_stdio = servers
                .iter()
                .any(|s| matches!(s, ServerConfig::Stdio { .. }));
            let has_http = servers
                .iter()
                .any(|s| matches!(s, ServerConfig::Http { .. }));
            assert!(has_stdio, "Expected a Stdio nested server");
            assert!(has_http, "Expected an Http nested server");
        }
        McpServerConfig::Server(_) => {
            panic!("Expected ToolProxy config");
        }
    }
}

#[tokio::test]
async fn test_tool_proxy_rejects_nested_in_memory() {
    let json = r#"
    {
        "servers": {
            "proxy": {
                "type": "in-memory",
                "input": {
                    "servers": {
                        "bad": {
                            "type": "in-memory"
                        }
                    }
                }
            }
        }
    }
    "#;

    let raw_config = RawMcpConfig::from_json(json).unwrap();
    let result = raw_config.into_configs(&HashMap::new()).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ParseError::InvalidNestedConfig(msg) => {
            assert!(msg.contains("in-memory"));
            assert!(msg.contains("bad"));
        }
        other => panic!("Expected InvalidNestedConfig, got {other:?}"),
    }
}

#[tokio::test]
async fn test_tool_proxy_rejects_nested_tool_proxy() {
    let json = r#"
    {
        "servers": {
            "outer": {
                "type": "in-memory",
                "input": {
                    "servers": {
                        "inner": {
                            "type": "in-memory",
                            "input": {
                                "servers": {}
                            }
                        }
                    }
                }
            }
        }
    }
    "#;

    let raw_config = RawMcpConfig::from_json(json).unwrap();
    let result = raw_config.into_configs(&HashMap::new()).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ParseError::InvalidNestedConfig(msg) => {
            assert!(msg.contains("in-memory"));
            assert!(msg.contains("inner"));
        }
        other => panic!("Expected InvalidNestedConfig, got {other:?}"),
    }
}
