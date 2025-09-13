use aether_core::mcp::builtin_servers::coding::{CodingMcp, GrepArgs, FindArgs, common::OutputMode};
use rmcp::handler::server::wrapper::Parameters;
use std::fs;
use tempfile::TempDir;

fn create_test_server() -> CodingMcp {
    CodingMcp::new()
}

async fn create_test_files(temp_dir: &TempDir) -> Result<(), std::io::Error> {
    let base_path = temp_dir.path();

    fs::write(base_path.join("test1.rs"), "fn main() {\n    println!(\"Hello, world!\");\n}")?;
    fs::write(base_path.join("test2.txt"), "This is a test file\nwith multiple lines\ncontaining test data")?;
    fs::write(base_path.join("example.py"), "def hello():\n    print(\"Hello from Python!\")\n")?;
    fs::create_dir_all(base_path.join("subdir"))?;
    fs::write(base_path.join("subdir/nested.rs"), "// This is a nested Rust file\nfn nested_function() {}")?;

    Ok(())
}

#[tokio::test]
async fn test_grep_matches_mode() {
    let temp_dir = TempDir::new().unwrap();
    create_test_files(&temp_dir).await.unwrap();
    let server = create_test_server();

    let args = GrepArgs {
        pattern: "Hello".to_string(),
        path: Some(temp_dir.path().to_string_lossy().to_string()),
        file_path: None,
        output_mode: Some(OutputMode::Matches),
        case_insensitive: Some(false),
        line_numbers: Some(true),
        context: None,
    };

    let result = server.grep(Parameters(args)).await;
    assert!(result.contains("\"status\": \"success\""));
    assert!(result.contains("Hello"));
}

#[tokio::test]
async fn test_grep_files_only_mode() {
    let temp_dir = TempDir::new().unwrap();
    create_test_files(&temp_dir).await.unwrap();
    let server = create_test_server();

    let args = GrepArgs {
        pattern: "Hello".to_string(),
        path: Some(temp_dir.path().to_string_lossy().to_string()),
        file_path: None,
        output_mode: Some(OutputMode::FilesOnly),
        case_insensitive: Some(false),
        line_numbers: None,
        context: None,
    };

    let result = server.grep(Parameters(args)).await;
    assert!(result.contains("\"status\": \"success\""));
    assert!(result.contains("\"files\""));
}

#[tokio::test]
async fn test_grep_single_file() {
    let temp_dir = TempDir::new().unwrap();
    create_test_files(&temp_dir).await.unwrap();
    let server = create_test_server();

    let test_file = temp_dir.path().join("test1.rs");
    let args = GrepArgs {
        pattern: "main".to_string(),
        path: None,
        file_path: Some(test_file.to_string_lossy().to_string()),
        output_mode: Some(OutputMode::Matches),
        case_insensitive: Some(false),
        line_numbers: Some(true),
        context: None,
    };

    let result = server.grep(Parameters(args)).await;
    assert!(result.contains("\"status\": \"success\""));
    assert!(result.contains("main"));
}

#[tokio::test]
async fn test_grep_case_insensitive() {
    let temp_dir = TempDir::new().unwrap();
    create_test_files(&temp_dir).await.unwrap();
    let server = create_test_server();

    let args = GrepArgs {
        pattern: "hello".to_string(),
        path: Some(temp_dir.path().to_string_lossy().to_string()),
        file_path: None,
        output_mode: Some(OutputMode::FilesOnly),
        case_insensitive: Some(true),
        line_numbers: None,
        context: None,
    };

    let result = server.grep(Parameters(args)).await;
    assert!(result.contains("\"status\": \"success\""));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    let file_count = parsed["file_count"].as_u64().unwrap();
    assert!(file_count > 0);
}

#[tokio::test]
async fn test_grep_nonexistent_file() {
    let server = create_test_server();

    let args = GrepArgs {
        pattern: "test".to_string(),
        path: None,
        file_path: Some("/nonexistent/file.txt".to_string()),
        output_mode: Some(OutputMode::Matches),
        case_insensitive: Some(false),
        line_numbers: None,
        context: None,
    };

    let result = server.grep(Parameters(args)).await;
    assert!(result.contains("Grep error"));
    assert!(result.contains("File does not exist"));
}

#[tokio::test]
async fn test_find_by_extension() {
    let temp_dir = TempDir::new().unwrap();
    create_test_files(&temp_dir).await.unwrap();
    let server = create_test_server();

    let args = FindArgs {
        pattern: "*.rs".to_string(),
        path: Some(temp_dir.path().to_string_lossy().to_string()),
        case_insensitive: Some(false),
    };

    let result = server.find(Parameters(args)).await;
    assert!(result.contains("\"status\": \"success\""));
    assert!(result.contains(".rs"));

    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    let files = parsed["files"].as_array().unwrap();
    assert!(files.len() >= 2);
}

#[tokio::test]
async fn test_find_exact_name() {
    let temp_dir = TempDir::new().unwrap();
    create_test_files(&temp_dir).await.unwrap();
    let server = create_test_server();

    let args = FindArgs {
        pattern: "test1.rs".to_string(),
        path: Some(temp_dir.path().to_string_lossy().to_string()),
        case_insensitive: Some(false),
    };

    let result = server.find(Parameters(args)).await;
    assert!(result.contains("\"status\": \"success\""));
    assert!(result.contains("test1.rs"));

    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    let files = parsed["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
}

#[tokio::test]
async fn test_find_case_insensitive() {
    let temp_dir = TempDir::new().unwrap();
    create_test_files(&temp_dir).await.unwrap();
    let server = create_test_server();

    let args = FindArgs {
        pattern: "TEST*.RS".to_string(),
        path: Some(temp_dir.path().to_string_lossy().to_string()),
        case_insensitive: Some(true),
    };

    let result = server.find(Parameters(args)).await;
    assert!(result.contains("\"status\": \"success\""));

    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    let files = parsed["files"].as_array().unwrap();
    assert!(files.len() >= 1);
}