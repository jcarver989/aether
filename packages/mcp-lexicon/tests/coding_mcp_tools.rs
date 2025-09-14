use aether::testing::connect;
use mcp_lexicon::CodingMcp;
use rmcp::model::{CallToolRequestParam, ClientInfo, Implementation};
use std::fs;

#[tokio::test]
async fn test_read_file_tool() {
    // Create server and client
    let server_service = CodingMcp::new();
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

    let (_server_handle, client) = connect(server_service, client_info)
        .await
        .expect("Failed to connect MCP server and client");

    // Create test file
    let test_content = "Hello, World!\nThis is a test file.";
    tokio::fs::write("/tmp/test_read_file.txt", test_content)
        .await
        .expect("Failed to create test file");

    // Test read_file tool
    let result = client
        .call_tool(CallToolRequestParam {
            name: "read_file".into(),
            arguments: Some(
                serde_json::json!({
                    "file_path": "/tmp/test_read_file.txt"
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to call read_file tool");

    // Verify result
    assert!(result.content.len() == 1);
    if let Some(content) = result.content.first() {
        if let Some(text_content) = content.as_text() {
            let parsed: serde_json::Value =
                serde_json::from_str(&text_content.text).expect("Invalid JSON response");
            assert_eq!(parsed["status"], "success");

            // Verify line-numbered content format (should read full file by default)
            let expected_formatted = "    1\tHello, World!\n    2\tThis is a test file.";
            assert_eq!(parsed["content"], expected_formatted);
            assert_eq!(parsed["total_lines"], 2);
            assert_eq!(parsed["lines_shown"], 2);
        } else {
            panic!("Expected text content");
        }
    } else {
        panic!("Expected content in result");
    }

    // Clean up
    let _ = tokio::fs::remove_file("/tmp/test_read_file.txt").await;
}

#[tokio::test]
async fn test_write_file_tool() {
    // Create server and client
    let server_service = CodingMcp::new();
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

    let (_server_handle, client) = connect(server_service, client_info)
        .await
        .expect("Failed to connect MCP server and client");

    let test_content = "This is test content written by the tool.";
    let test_path = "/tmp/test_write_file.txt";

    // Test write_file tool with new API
    let result = client
        .call_tool(CallToolRequestParam {
            name: "write_file".into(),
            arguments: Some(
                serde_json::json!({
                    "file_path": test_path,
                    "operations": [
                        {"type": "overwrite", "content": test_content}
                    ]
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to call write_file tool");

    // Verify result
    assert!(result.content.len() == 1);
    if let Some(content) = result.content.first() {
        if let Some(text_content) = content.as_text() {
            let parsed: serde_json::Value =
                serde_json::from_str(&text_content.text).expect("Invalid JSON response");
            assert_eq!(parsed["status"], "success");
            assert!(parsed["operations_applied"].as_array().unwrap().len() > 0);
        } else {
            panic!("Expected text content");
        }
    } else {
        panic!("Expected content in result");
    }

    // Verify file was actually written
    let file_content = tokio::fs::read_to_string(test_path)
        .await
        .expect("Failed to read written file");
    assert_eq!(file_content, test_content);

    // Clean up
    let _ = tokio::fs::remove_file(test_path).await;
}

#[tokio::test]
async fn test_bash_tool() {
    // Create server and client
    let server_service = CodingMcp::new();
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

    let (_server_handle, client) = connect(server_service, client_info)
        .await
        .expect("Failed to connect MCP server and client");

    // Test bash tool with a simple command
    let result = client
        .call_tool(CallToolRequestParam {
            name: "bash".into(),
            arguments: Some(
                serde_json::json!({
                    "command": "echo 'Hello from bash'"
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to call bash tool");

    // Verify result
    assert!(result.content.len() == 1);
    if let Some(content) = result.content.first() {
        if let Some(text_content) = content.as_text() {
            let parsed: serde_json::Value =
                serde_json::from_str(&text_content.text).expect("Invalid JSON response");
            assert_eq!(parsed["status"], "success");
            assert_eq!(parsed["exit_code"], 0);
            assert!(
                parsed["stdout"]
                    .as_str()
                    .unwrap()
                    .contains("Hello from bash")
            );
            assert_eq!(parsed["success"], true);
        } else {
            panic!("Expected text content");
        }
    } else {
        panic!("Expected content in result");
    }
}

#[tokio::test]
async fn test_write_file_append_mode() {
    // Create server and client
    let server_service = CodingMcp::new();
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

    let (_server_handle, client) = connect(server_service, client_info)
        .await
        .expect("Failed to connect MCP server and client");

    let test_path = "/tmp/test_append_file.txt";
    let first_content = "First line\n";
    let second_content = "Second line\n";

    // Write initial content
    let _ = client
        .call_tool(CallToolRequestParam {
            name: "write_file".into(),
            arguments: Some(
                serde_json::json!({
                    "file_path": test_path,
                    "operations": [
                        {"type": "overwrite", "content": first_content}
                    ]
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to call write_file tool");

    // Append content using line_range (start beyond end of file)
    let result = client
        .call_tool(CallToolRequestParam {
            name: "write_file".into(),
            arguments: Some(
                serde_json::json!({
                    "file_path": test_path,
                    "operations": [
                        {"type": "line_range", "start_line": 10, "end_line": 10, "content": second_content}
                    ]
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to call write_file tool in append mode");

    // Verify result
    if let Some(content) = result.content.first() {
        if let Some(text_content) = content.as_text() {
            let parsed: serde_json::Value =
                serde_json::from_str(&text_content.text).expect("Invalid JSON response");
            assert_eq!(parsed["status"], "success");
            assert!(parsed["operations_applied"].as_array().unwrap().len() > 0);
        }
    }

    // Verify file contains both contents
    let file_content = tokio::fs::read_to_string(test_path)
        .await
        .expect("Failed to read appended file");
    assert_eq!(file_content, format!("{}{}", first_content, second_content));

    // Clean up
    let _ = tokio::fs::remove_file(test_path).await;
}

#[tokio::test]
async fn test_list_files_tool() {
    // Create server and client
    let server_service = CodingMcp::new();
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

    let (_server_handle, client) = connect(server_service, client_info)
        .await
        .expect("Failed to connect MCP server and client");

    // Create test directory and files
    let test_dir = "/tmp/test_list_files";
    let _ = fs::remove_dir_all(test_dir); // Clean up any existing directory
    fs::create_dir_all(test_dir).expect("Failed to create test directory");

    // Create some test files
    fs::write(format!("{}/file1.txt", test_dir), "content1").expect("Failed to create test file 1");
    fs::write(format!("{}/file2.rs", test_dir), "fn main() {}")
        .expect("Failed to create test file 2");
    fs::create_dir(format!("{}/subdir", test_dir)).expect("Failed to create subdirectory");
    fs::write(format!("{}/.hidden_file", test_dir), "hidden content")
        .expect("Failed to create hidden file");

    // Test list_files tool
    let result = client
        .call_tool(CallToolRequestParam {
            name: "list_files".into(),
            arguments: Some(
                serde_json::json!({
                    "path": test_dir
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to call list_files tool");

    // Verify result
    assert!(result.content.len() == 1);
    if let Some(content) = result.content.first() {
        if let Some(text_content) = content.as_text() {
            let parsed: serde_json::Value =
                serde_json::from_str(&text_content.text).expect("Invalid JSON response");
            assert_eq!(parsed["status"], "success");
            assert_eq!(parsed["total_count"], 3); // Should not include hidden file by default

            let files = parsed["files"]
                .as_array()
                .expect("Files should be an array");
            let file_names: Vec<String> = files
                .iter()
                .map(|f| f["name"].as_str().unwrap().to_string())
                .collect();

            assert!(file_names.contains(&"file1.txt".to_string()));
            assert!(file_names.contains(&"file2.rs".to_string()));
            assert!(file_names.contains(&"subdir".to_string()));
            assert!(!file_names.contains(&".hidden_file".to_string())); // Hidden file should not be included
        } else {
            panic!("Expected text content");
        }
    } else {
        panic!("Expected content in result");
    }

    // Test including hidden files
    let result_with_hidden = client
        .call_tool(CallToolRequestParam {
            name: "list_files".into(),
            arguments: Some(
                serde_json::json!({
                    "path": test_dir,
                    "include_hidden": true
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to call list_files tool with hidden files");

    if let Some(content) = result_with_hidden.content.first() {
        if let Some(text_content) = content.as_text() {
            let parsed: serde_json::Value =
                serde_json::from_str(&text_content.text).expect("Invalid JSON response");
            assert_eq!(parsed["total_count"], 4); // Should include hidden file now
        }
    }

    // Clean up
    let _ = fs::remove_dir_all(test_dir);
}

#[tokio::test]
async fn test_line_range_operations() {
    // Create server and client
    let server_service = CodingMcp::new();
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

    let (_server_handle, client) = connect(server_service, client_info)
        .await
        .expect("Failed to connect MCP server and client");

    let test_path = "/tmp/test_line_range.txt";

    // Create initial file with 5 lines
    let initial_content = "line 1\nline 2\nline 3\nline 4\nline 5";
    let _ = client
        .call_tool(CallToolRequestParam {
            name: "write_file".into(),
            arguments: Some(
                serde_json::json!({
                    "file_path": test_path,
                    "operations": [
                        {"type": "overwrite", "content": initial_content}
                    ]
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to create initial file");

    // Test 1: Replace line range (lines 2-3)
    let _result = client
        .call_tool(CallToolRequestParam {
            name: "write_file".into(),
            arguments: Some(
                serde_json::json!({
                    "file_path": test_path,
                    "operations": [
                        {"type": "line_range", "start_line": 2, "end_line": 3, "content": "replaced line 2\nreplaced line 3"}
                    ]
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to replace line range");

    // Verify replacement worked
    let file_content = tokio::fs::read_to_string(test_path)
        .await
        .expect("Failed to read file");
    assert_eq!(file_content, "line 1\nreplaced line 2\nreplaced line 3\nline 4\nline 5");

    // Test 2: Insert between lines (insert at line 3)
    let _ = client
        .call_tool(CallToolRequestParam {
            name: "write_file".into(),
            arguments: Some(
                serde_json::json!({
                    "file_path": test_path,
                    "operations": [
                        {"type": "line_range", "start_line": 3, "end_line": 2, "content": "inserted line"}
                    ]
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to insert line");

    let file_content = tokio::fs::read_to_string(test_path)
        .await
        .expect("Failed to read file after insert");
    assert_eq!(file_content, "line 1\nreplaced line 2\ninserted line\nreplaced line 3\nline 4\nline 5");

    // Test 3: Append to end using line number beyond file length
    let _ = client
        .call_tool(CallToolRequestParam {
            name: "write_file".into(),
            arguments: Some(
                serde_json::json!({
                    "file_path": test_path,
                    "operations": [
                        {"type": "line_range", "start_line": 10, "end_line": 10, "content": "appended line"}
                    ]
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to append line");

    let file_content = tokio::fs::read_to_string(test_path)
        .await
        .expect("Failed to read file after append");
    assert!(file_content.ends_with("line 5\nappended line"));

    // Test 4: Multiple operations in one call
    let _ = client
        .call_tool(CallToolRequestParam {
            name: "write_file".into(),
            arguments: Some(
                serde_json::json!({
                    "file_path": test_path,
                    "operations": [
                        {"type": "overwrite", "content": "fresh start\nline 2\nline 3"},
                        {"type": "line_range", "start_line": 2, "end_line": 2, "content": "modified line 2"},
                        {"type": "line_range", "start_line": 4, "end_line": 3, "content": "new line 4"}
                    ]
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to perform multiple operations");

    let file_content = tokio::fs::read_to_string(test_path)
        .await
        .expect("Failed to read file after multiple operations");
    assert_eq!(file_content, "fresh start\nmodified line 2\nline 3\nnew line 4");


    // Clean up
    let _ = tokio::fs::remove_file(test_path).await;
}
