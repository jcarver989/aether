use mcp_servers::skills::SkillsMcp;
use mcp_utils::testing::connect;
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
async fn test_add_entry_to_new_skill() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join("skills")).unwrap();

    let (_server, client) = create_test_client(temp_dir.path()).await;

    let result = client
        .call_tool(call_tool_params(
            "add_skill_entry",
            serde_json::json!({
                "skill": "rust-borrow-checker",
                "skillDescription": "Strategies for fixing borrow checker errors",
                "content": "Prefer clone() over lifetime gymnastics in tests."
            }),
        ))
        .await
        .expect("add_skill_entry failed");

    let parsed = parse_tool_result(&result);
    assert_eq!(parsed["skillName"], "rust-borrow-checker");
    assert_eq!(parsed["status"], "created");
    assert!(parsed["entryId"].as_str().unwrap().len() == 6);

    // Verify file on disk
    let skill_md = temp_dir.path().join("skills/rust-borrow-checker/SKILL.md");
    assert!(skill_md.exists());
    let content = std::fs::read_to_string(&skill_md).unwrap();
    assert!(content.contains("description: Strategies for fixing borrow checker errors"));
    assert!(content.contains("## Agent Entries"));
    assert!(content.contains("Prefer clone()"));

    // List skills — should include the new skill
    let result = client
        .call_tool(call_tool_params("list_skills", serde_json::json!({})))
        .await
        .expect("list_skills failed");

    let parsed = parse_tool_result(&result);
    let skills = parsed["skills"].as_array().unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0]["name"], "rust-borrow-checker");
}

#[tokio::test]
async fn test_add_entry_to_existing_human_skill() {
    let temp_dir = TempDir::new().unwrap();
    let skills_dir = temp_dir.path().join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();

    // Create a human-authored skill
    let human_dir = skills_dir.join("rust-basics");
    std::fs::create_dir_all(&human_dir).unwrap();
    std::fs::write(
        human_dir.join("SKILL.md"),
        "---\ndescription: Rust basics\n---\n# Rust Basics\n\nHuman-written content here.\n",
    )
    .unwrap();

    let (_server, client) = create_test_client(temp_dir.path()).await;

    let result = client
        .call_tool(call_tool_params(
            "add_skill_entry",
            serde_json::json!({
                "skill": "rust-basics",
                "content": "Agent-discovered tip about lifetimes."
            }),
        ))
        .await
        .unwrap();

    let parsed = parse_tool_result(&result);
    assert_eq!(parsed["status"], "added_to_existing");

    // Verify human content preserved and entry appended
    let content = std::fs::read_to_string(human_dir.join("SKILL.md")).unwrap();
    assert!(content.contains("# Rust Basics"));
    assert!(content.contains("Human-written content here."));
    assert!(content.contains("## Agent Entries"));
    assert!(content.contains("Agent-discovered tip about lifetimes."));
}

#[tokio::test]
async fn test_score_entry_helpful() {
    let temp_dir = TempDir::new().unwrap();
    let skills_dir = temp_dir.path().join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();

    let (_server, client) = create_test_client(temp_dir.path()).await;

    // Create a skill with an entry
    let result = client
        .call_tool(call_tool_params(
            "add_skill_entry",
            serde_json::json!({
                "skill": "tips",
                "skillDescription": "Useful tips",
                "content": "A helpful tip."
            }),
        ))
        .await
        .unwrap();
    let entry_id = parse_tool_result(&result)["entryId"]
        .as_str()
        .unwrap()
        .to_string();

    // Score it helpful
    let result = client
        .call_tool(call_tool_params(
            "score_skill_entry",
            serde_json::json!({
                "skill": "tips",
                "entryId": entry_id,
                "helpful": true
            }),
        ))
        .await
        .unwrap();

    let parsed = parse_tool_result(&result);
    assert_eq!(parsed["status"], "scored");
    assert!(parsed["confidence"].as_f64().unwrap() > 0.0);

    // Verify on disk
    let content = std::fs::read_to_string(skills_dir.join("tips/SKILL.md")).unwrap();
    assert!(content.contains(&format!("### {entry_id} (+1/-0)")));
}

#[tokio::test]
async fn test_score_entry_harmful() {
    let temp_dir = TempDir::new().unwrap();
    let skills_dir = temp_dir.path().join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();

    let (_server, client) = create_test_client(temp_dir.path()).await;

    let result = client
        .call_tool(call_tool_params(
            "add_skill_entry",
            serde_json::json!({
                "skill": "tips",
                "skillDescription": "Tips",
                "content": "A bad tip."
            }),
        ))
        .await
        .unwrap();
    let entry_id = parse_tool_result(&result)["entryId"]
        .as_str()
        .unwrap()
        .to_string();

    let result = client
        .call_tool(call_tool_params(
            "score_skill_entry",
            serde_json::json!({
                "skill": "tips",
                "entryId": entry_id,
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
async fn test_entry_auto_prune() {
    let temp_dir = TempDir::new().unwrap();
    let skills_dir = temp_dir.path().join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();

    let (_server, client) = create_test_client(temp_dir.path()).await;

    let result = client
        .call_tool(call_tool_params(
            "add_skill_entry",
            serde_json::json!({
                "skill": "bad-advice",
                "skillDescription": "Bad advice",
                "content": "Don't do this."
            }),
        ))
        .await
        .unwrap();
    let entry_id = parse_tool_result(&result)["entryId"]
        .as_str()
        .unwrap()
        .to_string();

    // Score harmful 3x to trigger prune (0 helpful, 3 harmful, confidence = 0/4 = 0.0 < 0.2, total >= 3)
    for i in 0..3 {
        let result = client
            .call_tool(call_tool_params(
                "score_skill_entry",
                serde_json::json!({
                    "skill": "bad-advice",
                    "entryId": entry_id,
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

    // Entry should be removed from SKILL.md
    let content = std::fs::read_to_string(skills_dir.join("bad-advice/SKILL.md")).unwrap();
    assert!(!content.contains(&entry_id));

    // Should be in archive log
    let archive =
        std::fs::read_to_string(skills_dir.join(".archived/bad-advice/pruned.log")).unwrap();
    assert!(archive.contains(&entry_id));
    assert!(archive.contains("Don't do this."));
}

#[tokio::test]
async fn test_replace_entry() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join("skills")).unwrap();

    let (_server, client) = create_test_client(temp_dir.path()).await;

    // Create
    let result = client
        .call_tool(call_tool_params(
            "add_skill_entry",
            serde_json::json!({
                "skill": "evolving",
                "skillDescription": "Evolving skill",
                "content": "Original content."
            }),
        ))
        .await
        .unwrap();
    let entry_id = parse_tool_result(&result)["entryId"]
        .as_str()
        .unwrap()
        .to_string();

    // Score it a couple times
    for _ in 0..2 {
        client
            .call_tool(call_tool_params(
                "score_skill_entry",
                serde_json::json!({
                    "skill": "evolving",
                    "entryId": entry_id,
                    "helpful": true
                }),
            ))
            .await
            .unwrap();
    }

    // Replace it
    let result = client
        .call_tool(call_tool_params(
            "add_skill_entry",
            serde_json::json!({
                "skill": "evolving",
                "content": "Updated content.",
                "replaceId": entry_id
            }),
        ))
        .await
        .unwrap();

    let parsed = parse_tool_result(&result);
    assert_eq!(parsed["status"], "replaced");
    assert_eq!(parsed["entryId"], entry_id);

    // Verify: updated content, counters reset
    let content =
        std::fs::read_to_string(temp_dir.path().join("skills/evolving/SKILL.md")).unwrap();
    assert!(!content.contains("Original content."));
    assert!(content.contains("Updated content."));
    assert!(content.contains(&format!("### {entry_id} (+0/-0)")));
}

#[tokio::test]
async fn test_list_and_get_unchanged() {
    let temp_dir = TempDir::new().unwrap();
    let skills_dir = temp_dir.path().join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();

    // Create a human-authored skill with some content
    let human_dir = skills_dir.join("testing");
    std::fs::create_dir_all(&human_dir).unwrap();
    std::fs::write(
        human_dir.join("SKILL.md"),
        "---\ndescription: Testing conventions\n---\n# Testing\n\nUse Fake prefix.\n",
    )
    .unwrap();

    let (_server, client) = create_test_client(temp_dir.path()).await;

    // list_skills
    let result = client
        .call_tool(call_tool_params("list_skills", serde_json::json!({})))
        .await
        .unwrap();
    let parsed = parse_tool_result(&result);
    let skills = parsed["skills"].as_array().unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0]["name"], "testing");
    assert_eq!(skills[0]["description"], "Testing conventions");

    // get_skills
    let result = client
        .call_tool(call_tool_params(
            "get_skills",
            serde_json::json!({"skills": ["testing"]}),
        ))
        .await
        .unwrap();
    let parsed = parse_tool_result(&result);
    let loaded = parsed["skills"].as_array().unwrap();
    assert_eq!(loaded.len(), 1);
    assert!(
        loaded[0]["content"]
            .as_str()
            .unwrap()
            .contains("Use Fake prefix.")
    );
}

#[tokio::test]
async fn test_full_lifecycle() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join("skills")).unwrap();

    let (_server, client) = create_test_client(temp_dir.path()).await;

    // 1. Add entry to new skill
    let result = client
        .call_tool(call_tool_params(
            "add_skill_entry",
            serde_json::json!({
                "skill": "lifecycle-skill",
                "skillDescription": "Test lifecycle",
                "content": "Step 1: do stuff."
            }),
        ))
        .await
        .unwrap();
    let parsed = parse_tool_result(&result);
    assert_eq!(parsed["status"], "created");
    let entry_id = parsed["entryId"].as_str().unwrap().to_string();

    // 2. List — shows up
    let result = client
        .call_tool(call_tool_params("list_skills", serde_json::json!({})))
        .await
        .unwrap();
    let parsed = parse_tool_result(&result);
    assert!(
        parsed["skills"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["name"] == "lifecycle-skill")
    );

    // 3. Get — content loads (entries are just markdown)
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

    // 4. Score helpful 3x
    for _ in 0..3 {
        client
            .call_tool(call_tool_params(
                "score_skill_entry",
                serde_json::json!({
                    "skill": "lifecycle-skill",
                    "entryId": entry_id,
                    "helpful": true
                }),
            ))
            .await
            .unwrap();
    }

    // 5. Replace the entry with updated content
    let result = client
        .call_tool(call_tool_params(
            "add_skill_entry",
            serde_json::json!({
                "skill": "lifecycle-skill",
                "content": "Step 1: do better stuff.",
                "replaceId": entry_id
            }),
        ))
        .await
        .unwrap();
    let parsed = parse_tool_result(&result);
    assert_eq!(parsed["status"], "replaced");

    // 6. Verify update via get_skills
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

    // 7. Add a second entry, then prune it
    let result = client
        .call_tool(call_tool_params(
            "add_skill_entry",
            serde_json::json!({
                "skill": "lifecycle-skill",
                "content": "Bad advice."
            }),
        ))
        .await
        .unwrap();
    let bad_entry_id = parse_tool_result(&result)["entryId"]
        .as_str()
        .unwrap()
        .to_string();

    for _ in 0..3 {
        client
            .call_tool(call_tool_params(
                "score_skill_entry",
                serde_json::json!({
                    "skill": "lifecycle-skill",
                    "entryId": bad_entry_id,
                    "helpful": false
                }),
            ))
            .await
            .unwrap();
    }

    // 8. Skill still exists (first entry remains), but bad entry is gone
    let result = client
        .call_tool(call_tool_params(
            "get_skills",
            serde_json::json!({"skills": ["lifecycle-skill"]}),
        ))
        .await
        .unwrap();
    let parsed = parse_tool_result(&result);
    let content = parsed["skills"][0]["content"].as_str().unwrap();
    assert!(content.contains("better stuff"));
    assert!(!content.contains("Bad advice."));

    // 9. List still shows the skill
    let result = client
        .call_tool(call_tool_params("list_skills", serde_json::json!({})))
        .await
        .unwrap();
    let parsed = parse_tool_result(&result);
    assert!(
        parsed["skills"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["name"] == "lifecycle-skill")
    );
}
