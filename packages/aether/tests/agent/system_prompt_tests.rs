use aether::{
    agent::{Prompt, agent},
    testing::FakeLlmProvider,
};
use std::fs;
use std::sync::Mutex;
use tempfile::TempDir;

// Mutex to serialize tests that change the current directory
static DIR_LOCK: Mutex<()> = Mutex::new(());

#[tokio::test]
async fn test_system_prompt_text() {
    let llm = FakeLlmProvider::new(vec![]);
    let prompt = Prompt::text("Hello, world!").build().unwrap();
    let result = agent(llm).system(&prompt).spawn().await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_system_prompt_multiple_text() {
    let llm = FakeLlmProvider::new(vec![]);
    let prompt = Prompt::build_all(&[Prompt::text("First prompt"), Prompt::text("Second prompt")])
        .unwrap();
    let result = agent(llm).system(&prompt).spawn().await;

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

    // Build the prompt while in the temp directory
    let prompt = Prompt::file("test.md", false).build().unwrap();

    // Restore original directory before spawning agent
    std::env::set_current_dir(original_dir).unwrap();

    let llm = FakeLlmProvider::new(vec![]);
    let result = agent(llm).system(&prompt).spawn().await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_system_prompt_file_with_ancestors() {
    // Acquire lock to prevent parallel tests from interfering with current_dir
    let _lock = DIR_LOCK.lock().unwrap();

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

    // Resolve the prompt while in the subdirectory
    let prompt = Prompt::file("AGENTS.md", true).build().unwrap();

    // Restore original directory before spawning agent
    std::env::set_current_dir(&original_dir).unwrap();

    let llm = FakeLlmProvider::new(vec![]);
    let result = agent(llm).system(&prompt).spawn().await;

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

    let result = Prompt::file("nonexistent.md", false).build();

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

    let prompt = Prompt::build_all(&[
        Prompt::file("context.md", false),
        Prompt::text("Additional instructions"),
    ])
    .unwrap();
    let llm = FakeLlmProvider::new(vec![]);
    let result = agent(llm).system(&prompt).spawn().await;

    std::env::set_current_dir(original_dir).unwrap();

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_system_prompt_empty_slice() {
    let llm = FakeLlmProvider::new(vec![]);
    let prompt = Prompt::build_all(&[]).unwrap();
    let result = agent(llm).system(&prompt).spawn().await;

    assert!(result.is_ok());
}
