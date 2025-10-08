use aether::{
    agent::{SystemPrompt, agent},
    testing::FakeLlmProvider,
};
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_system_prompt_text() {
    let llm = FakeLlmProvider::new(vec![]);
    let result = agent(llm)
        .system(&[SystemPrompt::Text("Hello, world!".to_string())])
        .spawn()
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_system_prompt_multiple_text() {
    let llm = FakeLlmProvider::new(vec![]);
    let result = agent(llm)
        .system(&[
            SystemPrompt::Text("First prompt".to_string()),
            SystemPrompt::Text("Second prompt".to_string()),
        ])
        .spawn()
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_system_prompt_file_single() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.md");
    fs::write(&file_path, "Test content").unwrap();

    // Change to temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let llm = FakeLlmProvider::new(vec![]);
    let result = agent(llm)
        .system(&[SystemPrompt::File {
            path: "test.md".to_string(),
            ancestors: false,
        }])
        .spawn()
        .await;

    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_system_prompt_file_with_ancestors() {
    // Create directory structure:
    // temp/
    //   AGENTS.md (root)
    //   subdir/
    //     AGENTS.md (child)
    let temp_dir = TempDir::new().unwrap();
    let root_file = temp_dir.path().join("AGENTS.md");
    fs::write(&root_file, "Root instructions").unwrap();

    let subdir = temp_dir.path().join("subdir");
    fs::create_dir(&subdir).unwrap();
    let child_file = subdir.join("AGENTS.md");
    fs::write(&child_file, "Child instructions").unwrap();

    // Change to subdirectory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&subdir).unwrap();

    let llm = FakeLlmProvider::new(vec![]);
    let result = agent(llm)
        .system(&[SystemPrompt::File {
            path: "AGENTS.md".to_string(),
            ancestors: true,
        }])
        .spawn()
        .await;

    // Restore original directory before temp_dir is dropped
    std::env::set_current_dir(&original_dir).unwrap();

    // Keep temp_dir alive until after the result check
    assert!(result.is_ok());

    // Explicitly drop temp_dir here
    drop(temp_dir);
}

#[tokio::test]
async fn test_system_prompt_file_missing_error() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let llm = FakeLlmProvider::new(vec![]);
    let result = agent(llm)
        .system(&[SystemPrompt::File {
            path: "nonexistent.md".to_string(),
            ancestors: false,
        }])
        .spawn()
        .await;

    std::env::set_current_dir(original_dir).unwrap();

    assert!(result.is_err());
}

#[tokio::test]
async fn test_system_prompt_combined() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("context.md");
    fs::write(&file_path, "File content").unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let llm = FakeLlmProvider::new(vec![]);
    let result = agent(llm)
        .system(&[
            SystemPrompt::File {
                path: "context.md".to_string(),
                ancestors: false,
            },
            SystemPrompt::Text("Additional instructions".to_string()),
        ])
        .spawn()
        .await;

    std::env::set_current_dir(original_dir).unwrap();

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_system_prompt_empty_slice() {
    let llm = FakeLlmProvider::new(vec![]);
    let result = agent(llm).system(&[]).spawn().await;

    assert!(result.is_ok());
}
