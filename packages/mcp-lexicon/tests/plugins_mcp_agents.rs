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
            "sub-agents/debugger/AGENTS.md",
            "---\ndescription: Debug and fix code issues\nmodel: anthropic:claude-3.5-sonnet\n---\nYou are a debugging expert.",
        ),
        (
            "sub-agents/code-reviewer/AGENTS.md",
            "---\ndescription: Review code for best practices\nmodel: anthropic:claude-3.5-sonnet\n---\nYou are a code review expert.",
        ),
        (
            "sub-agents/no-frontmatter/AGENTS.md",
            "# Agent with no frontmatter\n\nThis should have empty description.",
        ),
    ];

    let temp_dir = create_test_files(&test_files);

    // Create MCP server and client
    let (_server_handle, client) = create_test_client(temp_dir.path()).await;

    // Test list_subagents tool
    let result = client
        .call_tool(CallToolRequestParam {
            name: "list_subagents".into(),
            arguments: None,
        })
        .await
        .expect("Failed to call list_subagents tool");

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

    // Test list_subagents tool with empty directory
    let result = client
        .call_tool(CallToolRequestParam {
            name: "list_subagents".into(),
            arguments: None,
        })
        .await
        .expect("Failed to call list_subagents tool");

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

#[tokio::test]
async fn test_spawn_agent_with_coding_mcp() {
    let test_files = vec![
        (
            "sub-agents/coder/AGENTS.md",
            "---\ndescription: A coding agent with file access\nmodel: anthropic:claude-3.5-sonnet\n---\nYou are a coding assistant with access to file operations.",
        ),
        (
            "sub-agents/coder/mcp.json",
            r#"{"servers": {"coding": {"type": "in-memory"}}}"#,
        ),
    ];

    let temp_dir = create_test_files(&test_files);

    // Create MCP server
    let (_server_handle, _client) = create_test_client(temp_dir.path()).await;

    // If we get here without panicking, the coding server was registered successfully
    // We can't test actual tool execution without a real LLM, but we verified:
    // 1. The mcp.json loads without errors
    // 2. The "coding" in-memory server factory is available
    // 3. The server configuration is valid
}

#[tokio::test]
async fn test_spawn_subagents_empty_tasks() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create empty sub-agents directory
    fs::create_dir_all(temp_dir.path().join("sub-agents"))
        .expect("Failed to create sub-agents directory");

    // Create MCP server and client
    let (_server_handle, client) = create_test_client(temp_dir.path()).await;

    // Call spawn_subagent with empty tasks array
    let mut args = serde_json::Map::new();
    args.insert("tasks".to_string(), serde_json::json!([]));
    let result = client
        .call_tool(CallToolRequestParam {
            name: "spawn_subagent".into(),
            arguments: Some(args),
        })
        .await
        .expect("Failed to call spawn_subagent tool");

    // Verify we get empty results with zero counts
    if let Some(content) = result.content.first() {
        if let Some(text_content) = content.as_text() {
            let parsed: serde_json::Value =
                serde_json::from_str(&text_content.text).expect("Invalid JSON response");

            let results = parsed["results"]
                .as_array()
                .expect("Expected results array");
            assert_eq!(results.len(), 0);
            assert_eq!(parsed["successCount"], 0);
            assert_eq!(parsed["errorCount"], 0);
        } else {
            panic!("Expected text content");
        }
    } else {
        panic!("Expected content in result");
    }
}

#[tokio::test]
async fn test_spawn_subagent_agent_not_found() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create empty sub-agents directory
    fs::create_dir_all(temp_dir.path().join("sub-agents"))
        .expect("Failed to create sub-agents directory");

    // Create MCP server and client
    let (_server_handle, client) = create_test_client(temp_dir.path()).await;

    // Call spawn_subagent with non-existent agent
    let mut args = serde_json::Map::new();
    args.insert(
        "tasks".to_string(),
        serde_json::json!([{
            "agentName": "nonexistent-agent",
            "prompt": "Do something"
        }]),
    );
    let result = client
        .call_tool(CallToolRequestParam {
            name: "spawn_subagent".into(),
            arguments: Some(args),
        })
        .await
        .expect("Failed to call spawn_subagent tool");

    // Verify we get an error result for the agent
    if let Some(content) = result.content.first() {
        if let Some(text_content) = content.as_text() {
            let parsed: serde_json::Value =
                serde_json::from_str(&text_content.text).expect("Invalid JSON response");

            let results = parsed["results"]
                .as_array()
                .expect("Expected results array");
            assert_eq!(results.len(), 1);

            let first_result = &results[0];
            assert_eq!(first_result["status"], "error");
            assert_eq!(first_result["agentName"], "nonexistent-agent");
            assert!(
                first_result["error"]
                    .as_str()
                    .unwrap()
                    .contains("not found")
            );

            assert_eq!(parsed["successCount"], 0);
            assert_eq!(parsed["errorCount"], 1);
        } else {
            panic!("Expected text content");
        }
    } else {
        panic!("Expected content in result");
    }
}

#[tokio::test]
async fn test_spawn_subagents_task_id_assignment() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create sub-agents directory with one agent (that will fail due to missing model,
    // but that's fine - we're testing task_id assignment)
    fs::create_dir_all(temp_dir.path().join("sub-agents/test-agent"))
        .expect("Failed to create sub-agents directory");
    fs::write(
        temp_dir.path().join("sub-agents/test-agent/AGENTS.md"),
        "You are a test agent.",
    )
    .expect("Failed to write agent file");

    // Create MCP server and client
    let (_server_handle, client) = create_test_client(temp_dir.path()).await;

    // Call spawn_subagent with explicit task_id and without
    let mut args = serde_json::Map::new();
    args.insert(
        "tasks".to_string(),
        serde_json::json!([
            {
                "agentName": "test-agent",
                "prompt": "First task",
                "taskId": "custom-id-1"
            },
            {
                "agentName": "test-agent",
                "prompt": "Second task"
                // no taskId - should be auto-generated
            }
        ]),
    );
    let result = client
        .call_tool(CallToolRequestParam {
            name: "spawn_subagent".into(),
            arguments: Some(args),
        })
        .await
        .expect("Failed to call spawn_subagent tool");

    if let Some(content) = result.content.first() {
        if let Some(text_content) = content.as_text() {
            let parsed: serde_json::Value =
                serde_json::from_str(&text_content.text).expect("Invalid JSON response");

            let results = parsed["results"]
                .as_array()
                .expect("Expected results array");
            assert_eq!(results.len(), 2);

            // First task should have custom ID
            let first_result = &results[0];
            assert_eq!(first_result["taskId"], "custom-id-1");

            // Second task should have auto-generated ID
            let second_result = &results[1];
            assert_eq!(second_result["taskId"], "task_1");
        } else {
            panic!("Expected text content");
        }
    } else {
        panic!("Expected content in result");
    }
}
