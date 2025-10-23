mod common;

use common::mcp::connect;
use mcp_lexicon::PluginsMcp;
use rmcp::model::{CallToolRequestParam, ClientInfo, Implementation};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Creates test files and directories from a slice of (path, content) pairs
/// Returns the temp directory path for cleanup
fn create_test_files(files: &[(&str, &str)]) -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    for (path, content) in files {
        let full_path = temp_dir.path().join(path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)
                .unwrap_or_else(|_| panic!("Failed to create directory for {path}"));
        }
        fs::write(&full_path, content).unwrap_or_else(|_| panic!("Failed to write file {path}"));
    }

    temp_dir
}

/// Helper to create MCP client connected to a test server
async fn create_test_client(
    test_dir: &Path,
) -> (
    rmcp::service::RunningService<rmcp::RoleServer, PluginsMcp>,
    rmcp::service::RunningService<rmcp::RoleClient, rmcp::model::ClientInfo>,
) {
    let server_service = PluginsMcp::new(test_dir.to_path_buf());
    let client_info = ClientInfo {
        client_info: Implementation {
            name: "test-client".to_string(),
            version: "0.1.0".to_string(),
            icons: None,
            title: None,
            website_url: None,
        },
        ..Default::default()
    };

    let (server_handle, client) = connect(server_service, client_info)
        .await
        .expect("Failed to connect MCP server and client");

    (server_handle, client)
}

#[tokio::test]
async fn test_list_agents_tool() {
    let test_files = vec![
        (
            "sub-agents/debugger/AGENT.md",
            "---\ndescription: Debug and fix code issues\nmodel: anthropic:claude-3.5-sonnet\n---\nYou are a debugging expert.",
        ),
        (
            "sub-agents/code-reviewer/AGENT.md",
            "---\ndescription: Review code for best practices\n---\nYou are a code review expert.",
        ),
        (
            "sub-agents/no-frontmatter/AGENT.md",
            "# Agent with no frontmatter\n\nThis should have empty description.",
        ),
    ];

    let temp_dir = create_test_files(&test_files);

    // Create MCP server and client
    let (_server_handle, client) = create_test_client(temp_dir.path()).await;

    // Test list_agents tool
    let result = client
        .call_tool(CallToolRequestParam {
            name: "list_agents".into(),
            arguments: None,
        })
        .await
        .expect("Failed to call list_agents tool");

    // Verify result
    assert!(result.content.len() == 1);
    if let Some(content) = result.content.first() {
        if let Some(text_content) = content.as_text() {
            let parsed: serde_json::Value =
                serde_json::from_str(&text_content.text).expect("Invalid JSON response");

            let agents = parsed["agents"].as_array().expect("Expected agents array");
            assert_eq!(agents.len(), 3);

            // Verify debugger agent
            let debugger = agents.iter().find(|a| a["name"] == "debugger").unwrap();
            assert_eq!(debugger["description"], "Debug and fix code issues");

            // Verify code-reviewer agent
            let reviewer = agents
                .iter()
                .find(|a| a["name"] == "code-reviewer")
                .unwrap();
            assert_eq!(reviewer["description"], "Review code for best practices");

            // Verify no-frontmatter agent has empty description
            let no_fm = agents
                .iter()
                .find(|a| a["name"] == "no-frontmatter")
                .unwrap();
            assert_eq!(no_fm["description"], "");
        } else {
            panic!("Expected text content");
        }
    } else {
        panic!("Expected content in result");
    }

    // TempDir automatically cleans up when dropped
}

#[tokio::test]
async fn test_list_agents_empty_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create empty sub-agents directory
    fs::create_dir_all(temp_dir.path().join("sub-agents"))
        .expect("Failed to create sub-agents directory");

    // Create MCP server and client
    let (_server_handle, client) = create_test_client(temp_dir.path()).await;

    // Test list_agents tool with empty directory
    let result = client
        .call_tool(CallToolRequestParam {
            name: "list_agents".into(),
            arguments: None,
        })
        .await
        .expect("Failed to call list_agents tool");

    // Verify we get empty array
    if let Some(content) = result.content.first() {
        if let Some(text_content) = content.as_text() {
            let parsed: serde_json::Value =
                serde_json::from_str(&text_content.text).expect("Invalid JSON response");

            let agents = parsed["agents"].as_array().expect("Expected agents array");
            assert_eq!(agents.len(), 0);
        }
    }
}
