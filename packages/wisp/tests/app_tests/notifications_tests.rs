use agent_client_protocol::schema as acp;
use tui::testing::TestTerminal;

use super::common::*;

#[tokio::test]
async fn test_sub_agent_progress_notification_triggers_render() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[], (TEST_WIDTH, 40));
    renderer.initial_render().unwrap();

    let json = r#"{"parent_tool_id":"p1","task_id":"t1","agent_name":"explorer","event":{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{}"},"model_name":"m"}}}"#;
    let raw = serde_json::value::to_raw_value(&serde_json::from_str::<serde_json::Value>(json).unwrap()).unwrap();
    let notification = acp::ExtNotification::new("_aether/sub_agent_progress", std::sync::Arc::from(raw));

    renderer.on_ext_notification(notification).unwrap();

    // Should render without crashing
    let lines = renderer.writer().get_lines();
    assert!(!lines.is_empty());
}

#[tokio::test]
async fn test_invalid_sub_agent_progress_json_silently_ignored() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[], (TEST_WIDTH, 40));
    renderer.initial_render().unwrap();

    let raw = serde_json::value::to_raw_value(&serde_json::json!({"bad": "data"})).unwrap();
    let notification = acp::ExtNotification::new("_aether/sub_agent_progress", std::sync::Arc::from(raw));

    renderer.on_ext_notification(notification).unwrap();

    // Should render without crashing
    let lines = renderer.writer().get_lines();
    assert!(!lines.is_empty());
}

#[tokio::test]
async fn test_context_usage_notification_updates_nominal_display() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[], (TEST_WIDTH, 40));
    renderer.initial_render().unwrap();

    let raw = serde_json::value::to_raw_value(&serde_json::json!({
        "usage_ratio": 0.75,
        "input_tokens": 150_000,
        "context_limit": 200_000
    }))
    .unwrap();
    let notification =
        acp::ExtNotification::new(acp_utils::notifications::CONTEXT_USAGE_METHOD, std::sync::Arc::from(raw));

    renderer.on_ext_notification(notification).unwrap();

    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains("150k / 200k")),
        "Status line should show nominal context usage.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_context_usage_notification_with_unknown_limit_clears_meter() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[], (TEST_WIDTH, 40));
    renderer.initial_render().unwrap();

    // First set a known usage
    let raw = serde_json::value::to_raw_value(&serde_json::json!({
        "usage_ratio": 0.67,
        "input_tokens": 100_000,
        "context_limit": 150_000
    }))
    .unwrap();
    let notification =
        acp::ExtNotification::new(acp_utils::notifications::CONTEXT_USAGE_METHOD, std::sync::Arc::from(raw));
    renderer.on_ext_notification(notification).unwrap();

    // Then clear it with null limit
    let raw = serde_json::value::to_raw_value(&serde_json::json!({
        "usage_ratio": null,
        "input_tokens": 0,
        "context_limit": null
    }))
    .unwrap();
    let notification =
        acp::ExtNotification::new(acp_utils::notifications::CONTEXT_USAGE_METHOD, std::sync::Arc::from(raw));
    renderer.on_ext_notification(notification).unwrap();

    let lines = renderer.writer().get_lines();
    assert!(
        !lines.iter().any(|l| l.contains("ctx")),
        "Context segment should not be shown when limit is unknown.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_context_cleared_notification_resets_conversation() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[], (TEST_WIDTH, 40));
    renderer.initial_render().unwrap();

    // Add some conversation content
    renderer
        .on_session_update(acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
            acp::TextContent::new("hello world"),
        ))))
        .unwrap();

    let lines = renderer.writer().get_lines();
    assert!(lines.iter().any(|l| l.contains("hello world")), "Content should be visible before clear");

    // Send context_cleared notification
    let raw = serde_json::value::to_raw_value(&serde_json::json!({})).unwrap();
    let notification =
        acp::ExtNotification::new(acp_utils::notifications::CONTEXT_CLEARED_METHOD, std::sync::Arc::from(raw));
    renderer.on_ext_notification(notification).unwrap();

    let lines = renderer.writer().get_lines();
    assert!(
        !lines.iter().any(|l| l.contains("hello world")),
        "Content should be cleared after context_cleared.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_on_tick_requests_render_while_completed_entries() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[], (TEST_WIDTH, 40));
    renderer.initial_render().unwrap();

    // Send a plan with completed entries
    renderer
        .on_session_update(acp::SessionUpdate::Plan(acp::Plan::new(vec![acp::PlanEntry::new(
            "1",
            acp::PlanEntryPriority::Medium,
            acp::PlanEntryStatus::Completed,
        )])))
        .unwrap();

    // Tick should produce a render (entries within grace period)
    renderer.on_tick().await.unwrap();

    // Should render without crashing
    let lines = renderer.writer().get_lines();
    assert!(!lines.is_empty());
}
