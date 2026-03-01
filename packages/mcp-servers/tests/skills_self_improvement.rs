use mcp_servers::skills::SkillsMcp;
use mcp_utils::testing::connect;
use rmcp::ServerHandler;
use rmcp::model::{CallToolRequestParams, ClientInfo, Implementation};
use std::path::Path;
use tempfile::TempDir;

async fn create_test_client(
    test_dir: &Path,
) -> (
    rmcp::service::RunningService<rmcp::RoleServer, SkillsMcp>,
    rmcp::service::RunningService<rmcp::RoleClient, ClientInfo>,
) {
    let server_service = SkillsMcp::new(test_dir.to_path_buf());
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

    connect(server_service, client_info)
        .await
        .expect("Failed to connect MCP server and client")
}

fn call_tool_params(name: &str, args: serde_json::Value) -> CallToolRequestParams {
    CallToolRequestParams {
        name: name.to_string().into(),
        meta: None,
        task: None,
        arguments: Some(args.as_object().unwrap().clone()),
    }
}

fn parse_tool_result(result: &rmcp::model::CallToolResult) -> serde_json::Value {
    let content = result.content.first().expect("Expected content");
    let text = content.as_text().expect("Expected text content");
    serde_json::from_str(&text.text).expect("Invalid JSON response")
}

#[tokio::test]
async fn test_save_skill_creates_new() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join("skills")).unwrap();

    let (_server, client) = create_test_client(temp_dir.path()).await;

    let result = client
        .call_tool(call_tool_params(
            "save_skill",
            serde_json::json!({
                "name": "fake-not-mock",
                "description": "Always use Fake prefix, never Mock",
                "tags": ["convention", "testing"],
                "content": "When writing test doubles, use the Fake prefix instead of Mock."
            }),
        ))
        .await
        .expect("save_skill failed");

    let parsed = parse_tool_result(&result);
    assert_eq!(parsed["name"], "fake-not-mock");
    assert_eq!(parsed["status"], "created");

    // Verify file on disk
    let skill_md = temp_dir.path().join("skills/fake-not-mock/SKILL.md");
    assert!(skill_md.exists());
    let content = std::fs::read_to_string(&skill_md).unwrap();
    assert!(content.contains("description: Always use Fake prefix, never Mock"));
    assert!(content.contains("agent_authored: true"));
    assert!(content.contains("convention"));
    assert!(content.contains("testing"));
    assert!(content.contains("When writing test doubles"));
}

#[tokio::test]
async fn test_save_skill_updates_existing_agent_skill() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join("skills")).unwrap();

    let (_server, client) = create_test_client(temp_dir.path()).await;

    // Create
    client
        .call_tool(call_tool_params(
            "save_skill",
            serde_json::json!({
                "name": "my-skill",
                "description": "Original description",
                "tags": ["rust"],
                "content": "Original content."
            }),
        ))
        .await
        .unwrap();

    // Rate it helpful to set counters
    client
        .call_tool(call_tool_params(
            "rate_skill",
            serde_json::json!({
                "name": "my-skill",
                "helpful": true
            }),
        ))
        .await
        .unwrap();

    // Update
    let result = client
        .call_tool(call_tool_params(
            "save_skill",
            serde_json::json!({
                "name": "my-skill",
                "description": "Updated description",
                "tags": ["rust", "convention"],
                "content": "Updated content."
            }),
        ))
        .await
        .unwrap();

    let parsed = parse_tool_result(&result);
    assert_eq!(parsed["status"], "updated");

    // Verify counters preserved
    let content =
        std::fs::read_to_string(temp_dir.path().join("skills/my-skill/SKILL.md")).unwrap();
    assert!(content.contains("Updated description"));
    assert!(content.contains("Updated content."));
    assert!(content.contains("helpful: 1")); // counter preserved
}

#[tokio::test]
async fn test_save_skill_rejects_human_skill() {
    let temp_dir = TempDir::new().unwrap();
    let skills_dir = temp_dir.path().join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();

    // Create a human-authored skill
    let human_dir = skills_dir.join("human-skill");
    std::fs::create_dir_all(&human_dir).unwrap();
    std::fs::write(
        human_dir.join("SKILL.md"),
        "---\ndescription: Human skill\n---\n# Human content\n",
    )
    .unwrap();

    let (_server, client) = create_test_client(temp_dir.path()).await;

    let result = client
        .call_tool(call_tool_params(
            "save_skill",
            serde_json::json!({
                "name": "human-skill",
                "description": "Trying to overwrite",
                "content": "Should fail."
            }),
        ))
        .await
        .unwrap();

    // Should return an error (is_error flag set)
    assert!(result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn test_rate_skill_helpful() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join("skills")).unwrap();

    let (_server, client) = create_test_client(temp_dir.path()).await;

    // Create agent skill
    client
        .call_tool(call_tool_params(
            "save_skill",
            serde_json::json!({
                "name": "tips",
                "description": "Useful tips",
                "content": "A helpful tip."
            }),
        ))
        .await
        .unwrap();

    // Rate helpful
    let result = client
        .call_tool(call_tool_params(
            "rate_skill",
            serde_json::json!({
                "name": "tips",
                "helpful": true
            }),
        ))
        .await
        .unwrap();

    let parsed = parse_tool_result(&result);
    assert_eq!(parsed["status"], "scored");
    assert!(parsed["confidence"].as_f64().unwrap() > 0.0);

    // Verify on disk
    let content = std::fs::read_to_string(temp_dir.path().join("skills/tips/SKILL.md")).unwrap();
    assert!(content.contains("helpful: 1"));
}

#[tokio::test]
async fn test_rate_skill_harmful() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join("skills")).unwrap();

    let (_server, client) = create_test_client(temp_dir.path()).await;

    client
        .call_tool(call_tool_params(
            "save_skill",
            serde_json::json!({
                "name": "tips",
                "description": "Tips",
                "content": "A bad tip."
            }),
        ))
        .await
        .unwrap();

    let result = client
        .call_tool(call_tool_params(
            "rate_skill",
            serde_json::json!({
                "name": "tips",
                "helpful": false
            }),
        ))
        .await
        .unwrap();

    let parsed = parse_tool_result(&result);
    assert_eq!(parsed["status"], "scored");
    assert_eq!(parsed["confidence"].as_f64().unwrap(), 0.0);
}

#[tokio::test]
async fn test_rate_skill_auto_prune() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join("skills")).unwrap();

    let (_server, client) = create_test_client(temp_dir.path()).await;

    client
        .call_tool(call_tool_params(
            "save_skill",
            serde_json::json!({
                "name": "bad-advice",
                "description": "Bad advice",
                "content": "Don't do this."
            }),
        ))
        .await
        .unwrap();

    // Score harmful 3x to trigger prune
    for i in 0..3 {
        let result = client
            .call_tool(call_tool_params(
                "rate_skill",
                serde_json::json!({
                    "name": "bad-advice",
                    "helpful": false
                }),
            ))
            .await
            .unwrap();

        let parsed = parse_tool_result(&result);
        if i == 2 {
            assert_eq!(parsed["status"], "pruned");
        }
    }

    // Skill directory should be removed
    assert!(!temp_dir.path().join("skills/bad-advice").exists());

    // Should be in archive log
    let archive = std::fs::read_to_string(
        temp_dir
            .path()
            .join("skills/.archived/bad-advice/pruned.log"),
    )
    .unwrap();
    assert!(archive.contains("bad-advice"));
    assert!(archive.contains("Don't do this."));
}

#[tokio::test]
async fn test_rate_skill_rejects_human_skill() {
    let temp_dir = TempDir::new().unwrap();
    let skills_dir = temp_dir.path().join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();

    let human_dir = skills_dir.join("human-skill");
    std::fs::create_dir_all(&human_dir).unwrap();
    std::fs::write(
        human_dir.join("SKILL.md"),
        "---\ndescription: Human skill\n---\n# Content\n",
    )
    .unwrap();

    let (_server, client) = create_test_client(temp_dir.path()).await;

    let result = client
        .call_tool(call_tool_params(
            "rate_skill",
            serde_json::json!({
                "name": "human-skill",
                "helpful": true
            }),
        ))
        .await
        .unwrap();

    assert!(result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn test_toc_includes_tags() {
    let temp_dir = TempDir::new().unwrap();
    let skills_dir = temp_dir.path().join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();

    // Create a skill with tags
    let skill_dir = skills_dir.join("tagged-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\ndescription: A tagged skill\ntags:\n  - convention\n  - testing\nagent_authored: true\n---\nContent.\n",
    )
    .unwrap();

    let server = SkillsMcp::new(temp_dir.path().to_path_buf());
    let info = server.get_info();
    let instructions = info.instructions.unwrap();

    assert!(
        instructions.contains("[convention, testing]"),
        "Expected tags in TOC, got: {instructions}"
    );
    assert!(instructions.contains("**tagged-skill**"));
}

#[tokio::test]
async fn test_full_lifecycle() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join("skills")).unwrap();

    let (_server, client) = create_test_client(temp_dir.path()).await;

    // 1. Save a new skill
    let result = client
        .call_tool(call_tool_params(
            "save_skill",
            serde_json::json!({
                "name": "lifecycle-skill",
                "description": "Test lifecycle",
                "tags": ["test"],
                "content": "Step 1: do stuff."
            }),
        ))
        .await
        .unwrap();
    let parsed = parse_tool_result(&result);
    assert_eq!(parsed["status"], "created");

    // 2. Get — content loads
    let result = client
        .call_tool(call_tool_params(
            "get_skills",
            serde_json::json!({"skills": ["lifecycle-skill"]}),
        ))
        .await
        .unwrap();
    let parsed = parse_tool_result(&result);
    assert!(
        parsed["skills"][0]["content"]
            .as_str()
            .unwrap()
            .contains("Step 1")
    );

    // 3. Rate helpful 3x
    for _ in 0..3 {
        client
            .call_tool(call_tool_params(
                "rate_skill",
                serde_json::json!({
                    "name": "lifecycle-skill",
                    "helpful": true
                }),
            ))
            .await
            .unwrap();
    }

    // 4. Update the skill
    let result = client
        .call_tool(call_tool_params(
            "save_skill",
            serde_json::json!({
                "name": "lifecycle-skill",
                "description": "Updated lifecycle",
                "tags": ["test", "updated"],
                "content": "Step 1: do better stuff."
            }),
        ))
        .await
        .unwrap();
    let parsed = parse_tool_result(&result);
    assert_eq!(parsed["status"], "updated");

    // 5. Verify update via get_skills — helpful counters preserved
    let result = client
        .call_tool(call_tool_params(
            "get_skills",
            serde_json::json!({"skills": ["lifecycle-skill"]}),
        ))
        .await
        .unwrap();
    let parsed = parse_tool_result(&result);
    assert!(
        parsed["skills"][0]["content"]
            .as_str()
            .unwrap()
            .contains("better stuff")
    );

    let content =
        std::fs::read_to_string(temp_dir.path().join("skills/lifecycle-skill/SKILL.md")).unwrap();
    assert!(content.contains("helpful: 3"));

    // 6. Save a bad skill, then prune it
    client
        .call_tool(call_tool_params(
            "save_skill",
            serde_json::json!({
                "name": "bad-skill",
                "description": "Bad advice",
                "content": "Bad advice."
            }),
        ))
        .await
        .unwrap();

    for _ in 0..3 {
        client
            .call_tool(call_tool_params(
                "rate_skill",
                serde_json::json!({
                    "name": "bad-skill",
                    "helpful": false
                }),
            ))
            .await
            .unwrap();
    }

    // Bad skill should be pruned
    assert!(!temp_dir.path().join("skills/bad-skill").exists());

    // Good skill should still exist
    let result = client
        .call_tool(call_tool_params(
            "get_skills",
            serde_json::json!({"skills": ["lifecycle-skill"]}),
        ))
        .await
        .unwrap();
    let parsed = parse_tool_result(&result);
    assert!(
        parsed["skills"][0]["content"]
            .as_str()
            .unwrap()
            .contains("better stuff")
    );
}
