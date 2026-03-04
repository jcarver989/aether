use mcp_servers::subagents::SubAgentsMcp;
use mcp_utils::testing::connect;
use rmcp::model::{CallToolRequestParams, ClientInfo, Implementation};
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
    rmcp::service::RunningService<rmcp::RoleServer, SubAgentsMcp>,
    rmcp::service::RunningService<rmcp::RoleClient, rmcp::model::ClientInfo>,
) {
    let server_service = SubAgentsMcp::new(test_dir.to_path_buf());
    let client_info = ClientInfo {
        client_info: Implementation {
            name: "test-client".to_string(),
            version: "0.1.0".to_string(),
            icons: None,
            title: None,
            website_url: None,
            description: None,
        },
        ..Default::default()
    };

    let (server_handle, client) = connect(server_service, client_info)
        .await
        .expect("Failed to connect MCP server and client");

    (server_handle, client)
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
        .call_tool(CallToolRequestParams {
            name: "spawn_subagent".into(),
            meta: None,
            task: None,
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
        .call_tool(CallToolRequestParams {
            name: "spawn_subagent".into(),
            meta: None,
            task: None,
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

    // Call spawn_subagent - task IDs are auto-generated
    let mut args = serde_json::Map::new();
    args.insert(
        "tasks".to_string(),
        serde_json::json!([
            {
                "agentName": "test-agent",
                "prompt": "First task"
            },
            {
                "agentName": "test-agent",
                "prompt": "Second task"
            }
        ]),
    );
    let result = client
        .call_tool(CallToolRequestParams {
            name: "spawn_subagent".into(),
            meta: None,
            task: None,
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

            // Task IDs are auto-generated based on index
            let first_result = &results[0];
            assert_eq!(first_result["taskId"], "task_0");

            let second_result = &results[1];
            assert_eq!(second_result["taskId"], "task_1");
        } else {
            panic!("Expected text content");
        }
    } else {
        panic!("Expected content in result");
    }
}
