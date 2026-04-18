use mcp_servers::PlanMcp;
use mcp_utils::client::{McpClient, McpClientEvent};
use mcp_utils::testing::connect;
use rmcp::model::{
    CallToolRequestParams, ClientCapabilities, ClientInfo, CreateElicitationRequestParams, CreateElicitationResult,
    ElicitationAction, Implementation,
};
use rmcp::service::RunningService;
use rmcp::{RoleClient, Service};
use serde_json::json;
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::{RwLock, mpsc};
use utils::plan_review::PlanReviewElicitationMeta;

#[tokio::test]
async fn submit_plan_attaches_plan_review_metadata_and_preserves_schema() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let plan_path = temp_dir.path().join("example-plan.md");
    let plan_content = "# Plan\n\nShip the feature.";
    fs::write(&plan_path, plan_content).expect("write plan file");

    let (event_tx, event_rx) = mpsc::channel(8);
    let client =
        McpClient::new(test_client_info(), "plan-test-server".to_string(), event_tx, Arc::new(RwLock::new(Vec::new())));

    let task_handle = respond_to_elicitation_request(
        event_rx,
        CreateElicitationResult {
            action: ElicitationAction::Accept,
            content: Some(json!({ "decision": "approve" })),
            meta: None,
        },
    );

    let server = PlanMcp::new();
    let (_server_handle, client_handle) = connect(server, client).await.expect("connect client");

    let result = submit_plan(&client_handle, plan_path.to_string_lossy().as_ref()).await;
    assert_eq!(result["approved"], true);
    assert!(result["feedback"].is_null());

    let elicitation_request = task_handle.await.expect("script task panicked").expect("expected elicitation request");
    let CreateElicitationRequestParams::FormElicitationParams { meta, requested_schema, .. } = elicitation_request
    else {
        panic!("submit_plan should issue form elicitation request");
    };

    let required = requested_schema.required.clone().unwrap_or_default();
    assert_eq!(required, vec!["decision".to_string()]);
    assert!(requested_schema.properties.contains_key("decision"));
    assert!(requested_schema.properties.contains_key("feedback"));

    let meta = meta.expect("plan review metadata should be set");
    let parsed_meta = PlanReviewElicitationMeta::parse(Some(&meta.0)).expect("should parse plan review metadata");
    assert_eq!(parsed_meta.ui, "planReview");
    assert_eq!(parsed_meta.plan_path, plan_path.display().to_string());
    assert_eq!(parsed_meta.markdown, plan_content);
}

fn test_client_info() -> ClientInfo {
    ClientInfo::new(ClientCapabilities::default(), Implementation::new("plan-test-client", "0.1.0"))
}

fn respond_to_elicitation_request(
    mut event_rx: mpsc::Receiver<McpClientEvent>,
    response: CreateElicitationResult,
) -> tokio::task::JoinHandle<Option<CreateElicitationRequestParams>> {
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            if let McpClientEvent::Elicitation(req) = event {
                let captured = req.request;
                let _ = req.response_sender.send(response);
                return Some(captured);
            }
        }
        None
    })
}

async fn submit_plan<T: Service<RoleClient>>(
    client: &RunningService<RoleClient, T>,
    plan_path: &str,
) -> serde_json::Value {
    let request = CallToolRequestParams::new("submit_plan")
        .with_arguments(json!({ "planPath": plan_path }).as_object().unwrap().clone());

    let result = client.call_tool(request).await.expect("call submit_plan");
    let content = result.content.first().expect("Expected content");
    let text = content.as_text().expect("Expected text content");
    serde_json::from_str(&text.text).expect("Invalid JSON response")
}
