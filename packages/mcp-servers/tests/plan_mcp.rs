use mcp_servers::PlanMcp;
use mcp_utils::testing::connect;
use rmcp::RoleClient;
use rmcp::model::{CallToolRequestParams, ClientCapabilities, ClientInfo, Implementation};
use rmcp::service::RunningService;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

fn test_client_info() -> ClientInfo {
    ClientInfo::new(ClientCapabilities::default(), Implementation::new("plan-test-client", "0.1.0"))
}

fn parse_tool_result(result: &rmcp::model::CallToolResult) -> serde_json::Value {
    let content = result.content.first().expect("Expected content");
    let text = content.as_text().expect("Expected text content");
    serde_json::from_str(&text.text).expect("Invalid JSON response")
}

fn call_submit_plan_request(plan_path: &str) -> CallToolRequestParams {
    CallToolRequestParams::new("submit_plan")
        .with_arguments(json!({ "planPath": plan_path }).as_object().unwrap().clone())
}

fn write_reviewer_script(dir: &TempDir, name: &str, body: &str) -> std::path::PathBuf {
    let script_path = dir.path().join(name);
    let script = format!("#!/usr/bin/env bash\nset -euo pipefail\n{body}\n");
    fs::write(&script_path, script).expect("write reviewer script");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(&script_path).expect("script metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script_path, permissions).expect("set script permissions");
    }

    script_path
}

async fn call_submit_plan(client: &RunningService<RoleClient, ClientInfo>, plan_path: &str) -> serde_json::Value {
    let result = client.call_tool(call_submit_plan_request(plan_path)).await.expect("call submit_plan");

    parse_tool_result(&result)
}

#[tokio::test]
async fn submit_plan_command_path_passes_payload_and_returns_generic_response() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let plan_path = temp_dir.path().join("example-plan.md");
    let plan_content = "# Plan\n\nShip the feature.";
    fs::write(&plan_path, plan_content).expect("write plan file");

    let payload_path = temp_dir.path().join("review-payload.json");
    let pwd_path = temp_dir.path().join("reviewer-pwd.txt");

    let reviewer = write_reviewer_script(
        &temp_dir,
        "reviewer-approve.sh",
        &format!(
            "payload=\"$(cat)\"\nprintf '%s' \"$payload\" > \"{}\"\npwd > \"{}\"\necho '{{\"approved\": true}}'",
            payload_path.display(),
            pwd_path.display()
        ),
    );

    let reviewer_command = format!("bash '{}'", reviewer.display());
    let server = PlanMcp::from_args(vec!["--command".into(), reviewer_command])
        .expect("parse plan mcp args")
        .with_root_dir(temp_dir.path().to_path_buf());

    let (_server_handle, client) = connect(server, test_client_info()).await.expect("connect client");
    let result = call_submit_plan(&client, plan_path.to_string_lossy().as_ref()).await;

    assert_eq!(result["approved"], true);
    assert!(result["feedback"].is_null());

    let payload_text = fs::read_to_string(&payload_path).expect("read payload capture");
    let payload: serde_json::Value = serde_json::from_str(&payload_text).expect("parse captured payload");

    assert_eq!(payload["protocol"], "aether-plan-review/v1");
    assert_eq!(payload["cwd"], temp_dir.path().to_string_lossy().to_string());
    assert_eq!(payload["plan_path"], plan_path.to_string_lossy().to_string());
    assert_eq!(payload["permission_mode"], "default");
    assert_eq!(payload["tool_input"]["plan"], plan_content);

    let reviewer_pwd = fs::read_to_string(&pwd_path).expect("read reviewer pwd");
    assert_eq!(reviewer_pwd.trim(), temp_dir.path().to_string_lossy());
}

#[tokio::test]
async fn submit_plan_command_path_supports_wrapper_normalizing_hook_output() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let plan_path = temp_dir.path().join("example-plan.md");
    fs::write(&plan_path, "# Plan\n\nNeed review.").expect("write plan file");

    let fake_plannotator = write_reviewer_script(
        &temp_dir,
        "plannotator",
        "cat >/dev/null\necho '{\"hookSpecificOutput\":{\"decision\":{\"behavior\":\"deny\",\"message\":\"Needs more detail\"}}}'",
    );

    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root");
    let adapter_path = workspace_root.join("scripts/plannotator-mcp-adapter");

    let reviewer_command = format!("PATH=\"{}:$PATH\" '{}'", temp_dir.path().display(), adapter_path.display());

    let server = PlanMcp::from_args(vec!["--command".into(), reviewer_command])
        .expect("parse plan mcp args")
        .with_root_dir(temp_dir.path().to_path_buf());

    let (_server_handle, client) = connect(server, test_client_info()).await.expect("connect client");
    let result = call_submit_plan(&client, plan_path.to_string_lossy().as_ref()).await;

    assert_eq!(result["approved"], false);
    assert_eq!(result["feedback"], "Needs more detail");

    assert!(fake_plannotator.exists());
}
