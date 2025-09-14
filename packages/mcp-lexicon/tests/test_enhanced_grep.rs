use aether::testing::connect;
use mcp_lexicon::coding::{GrepArgs, perform_grep, CodingMcp};
use rmcp::model::{CallToolRequestParam, ClientInfo, Implementation};
use std::fs;
use tempfile::TempDir;

async fn create_test_dir() -> TempDir {
    // Create test directory with files
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_dir = temp_dir.path();

    // Create test files of different types
    fs::write(test_dir.join("test.rs"), "fn main() {\n    println!(\"Hello, world!\");\n    let x = 42;\n}").unwrap();
    fs::write(test_dir.join("script.py"), "def hello():\n    print(\"Hello, world!\")\n    x = 42\n").unwrap();
    fs::write(test_dir.join("app.js"), "function hello() {\n    console.log(\"Hello, world!\");\n    const x = 42;\n}").unwrap();
    fs::write(test_dir.join("README.md"), "# Test Project\n\nThis is a test project.\nHello, world!\n").unwrap();
    fs::write(test_dir.join("data.txt"), "line one\nline two with hello\nline three\nline four\n").unwrap();

    temp_dir
}

#[tokio::test]
async fn test_file_type_filtering() {
    let temp_dir = create_test_dir().await;
    let test_path = temp_dir.path().to_str().unwrap();

    let args = GrepArgs {
        pattern: "hello".to_string(),
        path: Some(test_path.to_string()),
        file_path: None,
        output_mode: None,
        case_insensitive: Some(true),
        line_numbers: Some(true),
        context_after: None,
        context_before: None,
        context_around: None,
        file_types: Some(vec!["rust".to_string()]),
        max_results: None,
        invert_match: None,
        word_boundary: None,
    };

    let result = perform_grep(args).await.expect("Failed to perform grep");

    assert_eq!(result["status"], "success");
    assert_eq!(result["file_types"].as_array().unwrap(), &vec!["rust"]);

    let matches = result["matches"].as_array().unwrap();
    assert!(matches.len() > 0);
    // Should only find matches in .rs files
    assert!(matches.iter().all(|m| m.as_str().unwrap().contains("test.rs")));
}

#[tokio::test]
async fn test_multiple_file_types() {
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

    let temp_dir = create_test_dir().await;
    let test_path = temp_dir.path().to_str().unwrap();

    // Test filtering for multiple file types
    let result = client
        .call_tool(CallToolRequestParam {
            name: "grep".into(),
            arguments: Some(
                serde_json::json!({
                    "pattern": "hello",
                    "path": test_path,
                    "file_types": ["rust", "python"],
                    "case_insensitive": true
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to call grep tool");

    let content = result.content.first().unwrap().as_text().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content.text).unwrap();

    assert_eq!(parsed["status"], "success");
    let file_types = parsed["file_types"].as_array().unwrap();
    assert!(file_types.contains(&serde_json::Value::String("rust".to_string())));
    assert!(file_types.contains(&serde_json::Value::String("python".to_string())));

    let matches = parsed["matches"].as_array().unwrap();
    assert!(matches.len() > 0);
    // Should find matches in both .rs and .py files
    let has_rust = matches.iter().any(|m| m.as_str().unwrap().contains("test.rs"));
    let has_python = matches.iter().any(|m| m.as_str().unwrap().contains("script.py"));
    assert!(has_rust && has_python);
}

#[tokio::test]
async fn test_max_results_limit() {
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

    let temp_dir = create_test_dir().await;
    let test_path = temp_dir.path().to_str().unwrap();

    // Test with max_results limit
    let result = client
        .call_tool(CallToolRequestParam {
            name: "grep".into(),
            arguments: Some(
                serde_json::json!({
                    "pattern": "line",
                    "path": test_path,
                    "max_results": 2
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to call grep tool");

    let content = result.content.first().unwrap().as_text().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content.text).unwrap();

    assert_eq!(parsed["status"], "success");
    assert_eq!(parsed["max_results"], 2);

    let matches = parsed["matches"].as_array().unwrap();
    assert!(matches.len() <= 2);

    // If we found exactly 2 matches, it should be marked as truncated
    if matches.len() == 2 {
        assert_eq!(parsed["truncated"], true);
    }
}

#[tokio::test]
async fn test_word_boundary_matching() {
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

    let temp_dir = create_test_dir().await;

    // Create a specific test file for word boundary testing
    fs::write(temp_dir.path().join("words.txt"), "hello world\nhelloworld\nworld hello\n").unwrap();

    let test_path = temp_dir.path().to_str().unwrap();

    // Test word boundary matching - should only match "hello" as a whole word
    let result = client
        .call_tool(CallToolRequestParam {
            name: "grep".into(),
            arguments: Some(
                serde_json::json!({
                    "pattern": "hello",
                    "path": test_path,
                    "word_boundary": true,
                    "file_path": temp_dir.path().join("words.txt").to_str().unwrap()
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to call grep tool");

    let content = result.content.first().unwrap().as_text().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content.text).unwrap();

    assert_eq!(parsed["status"], "success");
    assert_eq!(parsed["word_boundary"], true);

    let matches = parsed["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 2); // Should find "hello world" and "world hello" but not "helloworld"

    // Verify it didn't match "helloworld"
    assert!(!matches.iter().any(|m| m.as_str().unwrap().contains("helloworld")));
}

#[tokio::test]
async fn test_invert_match() {
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

    let temp_dir = create_test_dir().await;
    let test_path = temp_dir.path().join("invert_test.txt");
    fs::write(&test_path, "line with hello\nline without target\nanother line with hello\n").unwrap();

    // Test invert matching - should find lines that DON'T contain "hello"
    let result = client
        .call_tool(CallToolRequestParam {
            name: "grep".into(),
            arguments: Some(
                serde_json::json!({
                    "pattern": "hello",
                    "file_path": test_path.to_str().unwrap(),
                    "invert_match": true
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to call grep tool");

    let content = result.content.first().unwrap().as_text().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content.text).unwrap();

    assert_eq!(parsed["status"], "success");
    assert_eq!(parsed["invert_match"], true);

    let matches = parsed["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 1); // Should find only the line without "hello"
    assert!(matches[0].as_str().unwrap().contains("without target"));
}

#[tokio::test]
async fn test_context_lines() {
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

    let temp_dir = create_test_dir().await;
    let test_path = temp_dir.path().join("context_test.txt");
    fs::write(&test_path, "line 1\nline 2\ntarget line\nline 4\nline 5\n").unwrap();

    // Test context lines around matches
    let result = client
        .call_tool(CallToolRequestParam {
            name: "grep".into(),
            arguments: Some(
                serde_json::json!({
                    "pattern": "target",
                    "file_path": test_path.to_str().unwrap(),
                    "context_around": 1
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to call grep tool");

    let content = result.content.first().unwrap().as_text().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content.text).unwrap();

    assert_eq!(parsed["status"], "success");
    let context = &parsed["context_lines"];
    assert_eq!(context["before"], 1);
    assert_eq!(context["after"], 1);

    // With context, we should get multiple lines in the output
    let matches = parsed["matches"].as_array().unwrap();
    assert!(matches.len() >= 1); // Should include the match and context lines
}

#[tokio::test]
async fn test_context_before_after() {
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

    let temp_dir = create_test_dir().await;
    let test_path = temp_dir.path().join("context_test2.txt");
    fs::write(&test_path, "line 1\nline 2\nline 3\ntarget line\nline 5\nline 6\nline 7\n").unwrap();

    // Test different before/after context
    let result = client
        .call_tool(CallToolRequestParam {
            name: "grep".into(),
            arguments: Some(
                serde_json::json!({
                    "pattern": "target",
                    "file_path": test_path.to_str().unwrap(),
                    "context_before": 2,
                    "context_after": 1
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to call grep tool");

    let content = result.content.first().unwrap().as_text().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content.text).unwrap();

    assert_eq!(parsed["status"], "success");
    let context = &parsed["context_lines"];
    assert_eq!(context["before"], 2);
    assert_eq!(context["after"], 1);
}