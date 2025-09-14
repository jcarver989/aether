use aether::testing::connect;
use mcp_lexicon::CodingMcp;
use rmcp::model::{CallToolRequestParam, ClientInfo, Implementation};

#[tokio::test]
async fn test_read_file_line_range() {
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

    // Create test file with multiple lines
    let test_content = "line 1\nline 2\nline 3\nline 4\nline 5\nline 6";
    let test_path = "/tmp/test_read_line_range.txt";
    tokio::fs::write(test_path, test_content)
        .await
        .expect("Failed to create test file");

    // Test 1: Read full file (default behavior)
    let result = client
        .call_tool(CallToolRequestParam {
            name: "read_file".into(),
            arguments: Some(
                serde_json::json!({
                    "file_path": test_path
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to read full file");

    if let Some(content) = result.content.first() {
        if let Some(text_content) = content.as_text() {
            let parsed: serde_json::Value =
                serde_json::from_str(&text_content.text).expect("Invalid JSON response");
            assert_eq!(parsed["total_lines"], 6);
            assert_eq!(parsed["lines_shown"], 6);
            assert!(parsed["content"].as_str().unwrap().contains("   1│ line 1"));
            assert!(parsed["content"].as_str().unwrap().contains("   6│ line 6"));
        }
    }

    // Test 2: Read specific lines with offset and limit (lines 2-4)
    let result = client
        .call_tool(CallToolRequestParam {
            name: "read_file".into(),
            arguments: Some(
                serde_json::json!({
                    "file_path": test_path,
                    "offset": 2,
                    "limit": 3
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to read with offset and limit");

    if let Some(content) = result.content.first() {
        if let Some(text_content) = content.as_text() {
            let parsed: serde_json::Value =
                serde_json::from_str(&text_content.text).expect("Invalid JSON response");
            assert_eq!(parsed["total_lines"], 6); // Total lines in file
            assert_eq!(parsed["lines_shown"], 3); // Lines actually shown (2, 3, 4)
            assert_eq!(parsed["offset"], 2);
            assert_eq!(parsed["limit"], 3);
            let expected_content = "   2│ line 2\n   3│ line 3\n   4│ line 4";
            assert_eq!(parsed["content"], expected_content);
        }
    }

    // Test 3: Read single line with offset only
    let result = client
        .call_tool(CallToolRequestParam {
            name: "read_file".into(),
            arguments: Some(
                serde_json::json!({
                    "file_path": test_path,
                    "offset": 3,
                    "limit": 1
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to read single line");

    if let Some(content) = result.content.first() {
        if let Some(text_content) = content.as_text() {
            let parsed: serde_json::Value =
                serde_json::from_str(&text_content.text).expect("Invalid JSON response");
            assert_eq!(parsed["lines_shown"], 1);
            assert_eq!(parsed["content"], "   3│ line 3");
        }
    }

    // Test 4: Read from offset to end (no limit)
    let result = client
        .call_tool(CallToolRequestParam {
            name: "read_file".into(),
            arguments: Some(
                serde_json::json!({
                    "file_path": test_path,
                    "offset": 4
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to read from offset to end");

    if let Some(content) = result.content.first() {
        if let Some(text_content) = content.as_text() {
            let parsed: serde_json::Value =
                serde_json::from_str(&text_content.text).expect("Invalid JSON response");
            assert_eq!(parsed["lines_shown"], 3); // Lines 4, 5, 6
            assert_eq!(parsed["offset"], 4);
            assert_eq!(parsed["limit"], serde_json::Value::Null);
            let expected_content = "   4│ line 4\n   5│ line 5\n   6│ line 6";
            assert_eq!(parsed["content"], expected_content);
        }
    }

    // Clean up
    let _ = tokio::fs::remove_file(test_path).await;
}