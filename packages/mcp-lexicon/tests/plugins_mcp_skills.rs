mod common;

use common::mcp::connect;
use mcp_lexicon::{MarkdownFile, PluginsMcp};
use rmcp::model::{CallToolRequestParam, ClientInfo, Implementation};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestFrontmatter {
    pub description: Option<String>,
}

/// Creates test files and directories from a slice of (path, content) pairs
/// Returns the temp directory path for cleanup
fn create_test_files(files: &[(&str, &str)]) -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    for (path, content) in files {
        let full_path = temp_dir.path().join(path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).expect(&format!("Failed to create directory for {}", path));
        }
        fs::write(&full_path, content).expect(&format!("Failed to write file {}", path));
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
async fn test_load_from_nested_directories() {
    let test_files = vec![
        (
            "skill-1/SKILL.md",
            "---\ndescription: First skill\n---\nThis is skill 1 content",
        ),
        (
            "skill-2/SKILL.md",
            "---\ndescription: Second skill\n---\nThis is skill 2 content",
        ),
        ("illegal-flat-skill.md", "This should be ignored"),
    ];

    let temp_dir = create_test_files(&test_files);

    // Load skills from nested directories
    let skills_with_dirs: Vec<(PathBuf, MarkdownFile<TestFrontmatter>)> =
        MarkdownFile::from_nested_dirs(temp_dir.path(), "SKILL.md")
            .await
            .expect("Failed to load skills");

    // Verify we get exactly 2 skills (flat file should be ignored)
    assert_eq!(skills_with_dirs.len(), 2);

    let skill_names: Vec<String> = skills_with_dirs
        .iter()
        .filter_map(|(dir, _)| {
            let name = dir.file_name()?.to_string_lossy().to_string();
            Some(name)
        })
        .collect();

    assert!(skill_names.contains(&"skill-1".to_string()));
    assert!(skill_names.contains(&"skill-2".to_string()));
    assert!(!skill_names.contains(&"illegal-flat-skill".to_string()));

    // Verify content is loaded correctly
    let skill1 = skills_with_dirs
        .iter()
        .find(|(dir, _)| dir.file_name().unwrap().to_str() == Some("skill-1"))
        .map(|(_, file)| file)
        .unwrap();
    assert!(skill1.content.contains("This is skill 1 content"));
    assert_eq!(
        skill1.frontmatter.as_ref().unwrap().description,
        Some("First skill".to_string())
    );

    // TempDir automatically cleans up when dropped
}

#[tokio::test]
async fn test_load_skills_tool() {
    let test_files = vec![
        (
            "skills/skill-1/SKILL.md",
            "---\ndescription: First skill for testing\n---\n# Skill 1\n\nThis is the content for skill 1.",
        ),
        (
            "skills/skill-2/SKILL.md",
            "# Skill 2\n\nThis is the content for skill 2 with no frontmatter.",
        ),
        (
            "skills/skill-3/SKILL.md",
            "---\ndescription: Third skill\n---\n# Skill 3\n\nThis is skill 3.",
        ),
    ];

    let temp_dir = create_test_files(&test_files);

    // Create MCP server and client
    let (_server_handle, client) = create_test_client(temp_dir.path()).await;

    // Test loading multiple skills
    let result = client
        .call_tool(CallToolRequestParam {
            name: "get_skills".into(),
            arguments: Some(
                serde_json::json!({
                    "skills": ["skill-1", "skill-2", "skill-3"]
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to call get_skills tool");

    // Verify result
    assert!(result.content.len() == 1);
    if let Some(content) = result.content.first() {
        if let Some(text_content) = content.as_text() {
            let parsed: serde_json::Value =
                serde_json::from_str(&text_content.text).expect("Invalid JSON response");

            let skills = parsed["skills"].as_array().expect("Expected skills array");
            assert_eq!(skills.len(), 3);

            // Verify skill-1
            let skill1 = skills.iter().find(|s| s["name"] == "skill-1").unwrap();
            assert!(
                skill1["content"]
                    .as_str()
                    .unwrap()
                    .contains("This is the content for skill 1")
            );

            // Verify skill-2
            let skill2 = skills.iter().find(|s| s["name"] == "skill-2").unwrap();
            assert!(
                skill2["content"]
                    .as_str()
                    .unwrap()
                    .contains("This is the content for skill 2")
            );

            // Verify skill-3
            let skill3 = skills.iter().find(|s| s["name"] == "skill-3").unwrap();
            assert!(
                skill3["content"]
                    .as_str()
                    .unwrap()
                    .contains("This is skill 3")
            );
        } else {
            panic!("Expected text content");
        }
    } else {
        panic!("Expected content in result");
    }

    // Test loading with some missing skills
    let result_with_missing = client
        .call_tool(CallToolRequestParam {
            name: "get_skills".into(),
            arguments: Some(
                serde_json::json!({
                    "skills": ["skill-1", "nonexistent-skill", "skill-2"]
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await
        .expect("Failed to call get_skills tool with missing skills");

    if let Some(content) = result_with_missing.content.first() {
        if let Some(text_content) = content.as_text() {
            let parsed: serde_json::Value =
                serde_json::from_str(&text_content.text).expect("Invalid JSON response");

            // Should only have 2 skills loaded (nonexistent-skill is silently skipped)
            let skills = parsed["skills"].as_array().unwrap();
            assert_eq!(skills.len(), 2);

            // Verify we got skill-1 and skill-2 (not nonexistent-skill)
            assert!(skills.iter().any(|s| s["name"] == "skill-1"));
            assert!(skills.iter().any(|s| s["name"] == "skill-2"));
        }
    }

    // TempDir automatically cleans up when dropped
}

#[tokio::test]
async fn test_list_skills_tool() {
    let test_files = vec![
        (
            "skills/skill-1/SKILL.md",
            "---\ndescription: First skill\n---\nContent here.",
        ),
        ("skills/skill-2/SKILL.md", "# Skill 2\n\nNo frontmatter."),
        (
            "skills/skill-3/SKILL.md",
            "---\ndescription: Third skill\n---\nContent.",
        ),
    ];

    let temp_dir = create_test_files(&test_files);

    // Create MCP server and client
    let (_server_handle, client) = create_test_client(temp_dir.path()).await;

    // Test list_skills tool
    let result = client
        .call_tool(CallToolRequestParam {
            name: "list_skills".into(),
            arguments: None,
        })
        .await
        .expect("Failed to call list_skills tool");

    // Verify result
    assert!(result.content.len() == 1);
    if let Some(content) = result.content.first() {
        if let Some(text_content) = content.as_text() {
            let parsed: serde_json::Value =
                serde_json::from_str(&text_content.text).expect("Invalid JSON response");

            let skills = parsed["skills"].as_array().expect("Expected skills array");
            assert_eq!(skills.len(), 3);

            // Verify skill-1 has description
            let skill1 = skills.iter().find(|s| s["name"] == "skill-1").unwrap();
            assert_eq!(skill1["description"], "First skill");

            // Verify skill-2 has empty description
            let skill2 = skills.iter().find(|s| s["name"] == "skill-2").unwrap();
            assert_eq!(skill2["description"], "");

            // Verify skill-3 has description
            let skill3 = skills.iter().find(|s| s["name"] == "skill-3").unwrap();
            assert_eq!(skill3["description"], "Third skill");
        } else {
            panic!("Expected text content");
        }
    } else {
        panic!("Expected content in result");
    }

    // TempDir automatically cleans up when dropped
}
