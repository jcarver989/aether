use aether_core::{mcp::builtin_servers::CodingMcp, testing::connect};
use rmcp::model::{CallToolRequestParam, ClientInfo, Implementation};

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
            let parsed: serde_json::Value = serde_json::from_str(&text_content.text).expect("Invalid JSON response");
            assert_eq!(parsed["status"], "success");
            assert_eq!(parsed["content"], test_content);
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
    
    // Test write_file tool
    let result = client
        .call_tool(CallToolRequestParam {
            name: "write_file".into(),
            arguments: Some(
                serde_json::json!({
                    "file_path": test_path,
                    "content": test_content
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
            let parsed: serde_json::Value = serde_json::from_str(&text_content.text).expect("Invalid JSON response");
            assert_eq!(parsed["status"], "success");
            assert_eq!(parsed["operation"], "written");
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
            let parsed: serde_json::Value = serde_json::from_str(&text_content.text).expect("Invalid JSON response");
            assert_eq!(parsed["status"], "success");
            assert_eq!(parsed["exit_code"], 0);
            assert!(parsed["stdout"].as_str().unwrap().contains("Hello from bash"));
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
                    "content": first_content
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to call write_file tool");
    
    // Append content
    let result = client
        .call_tool(CallToolRequestParam {
            name: "write_file".into(),
            arguments: Some(
                serde_json::json!({
                    "file_path": test_path,
                    "content": second_content,
                    "append": true
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
            let parsed: serde_json::Value = serde_json::from_str(&text_content.text).expect("Invalid JSON response");
            assert_eq!(parsed["status"], "success");
            assert_eq!(parsed["operation"], "appended");
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