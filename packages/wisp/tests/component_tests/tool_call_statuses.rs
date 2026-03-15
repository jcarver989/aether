use wisp::components::tool_call_statuses::ToolCallStatuses;
use wisp::components::tool_call_status_view::{ToolCallStatus, ToolCallStatusView, MAX_TOOL_ARG_LENGTH};
use tui::{ViewContext, Line, BRAILLE_FRAMES as FRAMES};
use tui::testing::render_lines;
use acp_utils::notifications::SubAgentProgressParams;
use agent_client_protocol as acp;

fn ctx() -> ViewContext {
    ViewContext::new((80, 24))
}

fn render_all(statuses: &ToolCallStatuses, ids: &[&str], ctx: &ViewContext) -> Vec<Line> {
    ids.iter().flat_map(|id| statuses.render_tool(id, ctx)).collect()
}

fn make_tool_call(id: &str, title: &str, raw_input: Option<&str>) -> acp::ToolCall {
    let mut tc = acp::ToolCall::new(id.to_string(), title);
    if let Some(input) = raw_input {
        tc = tc.raw_input(serde_json::from_str::<serde_json::Value>(input).unwrap());
    }
    tc
}

fn make_tool_call_update(id: &str, status: acp::ToolCallStatus) -> acp::ToolCallUpdate {
    acp::ToolCallUpdate::new(
        id.to_string(),
        acp::ToolCallUpdateFields::new().status(status),
    )
}

fn make_sub_agent_notification(
    parent_tool_id: &str,
    agent_name: &str,
    event_json: &str,
) -> SubAgentProgressParams {
    make_sub_agent_notification_with_task_id(parent_tool_id, agent_name, agent_name, event_json)
}

fn make_sub_agent_notification_with_task_id(
    parent_tool_id: &str,
    task_id: &str,
    agent_name: &str,
    event_json: &str,
) -> SubAgentProgressParams {
    let json = format!(
        r#"{{"parent_tool_id":"{parent_tool_id}","task_id":"{task_id}","agent_name":"{agent_name}","event":{event_json}}}"#,
    );
    serde_json::from_str(&json).unwrap()
}

#[test]
fn request_tracks_tool() {
    let mut statuses = ToolCallStatuses::new();
    statuses.on_tool_call(&make_tool_call(
        "tool-1",
        "Read",
        Some(r#""/path/to/file""#),
    ));
    let lines = statuses.render_tool("tool-1", &ctx());
    assert_eq!(lines.len(), 1);
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("Read"));
}

#[test]
fn update_to_success() {
    let mut statuses = ToolCallStatuses::new();
    statuses.on_tool_call(&make_tool_call("tool-1", "Read", None));
    statuses.on_tool_call_update(&make_tool_call_update(
        "tool-1",
        acp::ToolCallStatus::Completed,
    ));
    let lines = statuses.render_tool("tool-1", &ctx());
    assert_eq!(lines.len(), 1);
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("✓"));
}

#[test]
fn unknown_update_is_ignored() {
    let mut statuses = ToolCallStatuses::new();
    statuses.on_tool_call_update(&make_tool_call_update(
        "unknown",
        acp::ToolCallStatus::Completed,
    ));
    let lines = statuses.render_tool("unknown", &ctx());
    assert!(lines.is_empty());
}

#[test]
fn update_to_error() {
    let mut statuses = ToolCallStatuses::new();
    statuses.on_tool_call(&make_tool_call("tool-1", "Read", None));
    statuses.on_tool_call_update(&make_tool_call_update(
        "tool-1",
        acp::ToolCallStatus::Failed,
    ));
    let lines = statuses.render_tool("tool-1", &ctx());
    assert_eq!(lines.len(), 1);
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("✗"));
}

#[test]
fn multiple_tools_render_in_order() {
    let mut statuses = ToolCallStatuses::new();
    statuses.on_tool_call(&make_tool_call("tool-1", "Read", None));
    statuses.on_tool_call(&make_tool_call("tool-2", "Write", None));
    let lines = render_all(&statuses, &["tool-1", "tool-2"], &ctx());
    assert_eq!(lines.len(), 2);
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("Read"));
    assert!(output[1].contains("Write"));
}

#[test]
fn multiple_tools_complete_independently() {
    let mut statuses = ToolCallStatuses::new();
    statuses.on_tool_call(&make_tool_call("tool-1", "Read", None));
    statuses.on_tool_call(&make_tool_call("tool-2", "Write", None));
    statuses.on_tool_call_update(&make_tool_call_update(
        "tool-1",
        acp::ToolCallStatus::Completed,
    ));
    let lines = render_all(&statuses, &["tool-1", "tool-2"], &ctx());
    assert_eq!(lines.len(), 2);
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("✓")); // Read completed
    assert!(!output[1].contains("✓")); // Write still running
}

#[test]
fn clear_removes_all() {
    let mut statuses = ToolCallStatuses::new();
    statuses.on_tool_call(&make_tool_call("tool-1", "Read", None));
    statuses.clear();
    assert!(!statuses.has_tool("tool-1"));
    assert!(statuses.render_tool("tool-1", &ctx()).is_empty());
}

#[test]
fn view_renders_running_with_spinner() {
    let status = ToolCallStatus::Running;
    let view = ToolCallStatusView {
        name: "TestTool",
        arguments: "test args",
        display_value: None,
        diff_preview: None,
        status: &status,
        tick: 0,
    };
    let lines = view.render(&ctx());
    assert_eq!(lines.len(), 1);
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("TestTool"));
    assert!(!output[0].contains("test args"));
    assert!(output[0].contains(FRAMES[0]));
}

#[test]
fn view_running_spinner_changes_with_tick() {
    let status = ToolCallStatus::Running;
    let view_a = ToolCallStatusView {
        name: "TestTool",
        arguments: "",
        display_value: None,
        diff_preview: None,
        status: &status,
        tick: 0,
    };
    let view_b = ToolCallStatusView {
        name: "TestTool",
        arguments: "",
        display_value: None,
        diff_preview: None,
        status: &status,
        tick: 1,
    };
    let lines_a = view_a.render(&ctx());
    let lines_b = view_b.render(&ctx());
    let term_a = render_lines(&lines_a, 80, 24);
    let term_b = render_lines(&lines_b, 80, 24);
    let a = &term_a.get_lines()[0];
    let b = &term_b.get_lines()[0];
    assert_ne!(a, b);
}

#[test]
fn view_renders_success() {
    let status = ToolCallStatus::Success;
    let view = ToolCallStatusView {
        name: "TestTool",
        arguments: "test args",
        display_value: None,
        diff_preview: None,
        status: &status,
        tick: 0,
    };
    let lines = view.render(&ctx());
    assert_eq!(lines.len(), 1);
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("✓"));
}

#[test]
fn view_renders_error() {
    let status = ToolCallStatus::Error("boom".to_string());
    let view = ToolCallStatusView {
        name: "TestTool",
        arguments: "test args",
        display_value: None,
        diff_preview: None,
        status: &status,
        tick: 0,
    };
    let lines = view.render(&ctx());
    assert_eq!(lines.len(), 1);
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("✗"));
    assert!(output[0].contains("boom"));
}

#[test]
fn view_truncates_utf8_arguments_without_panicking() {
    let arguments = format!("{}界", "a".repeat(MAX_TOOL_ARG_LENGTH - 2));
    let status = ToolCallStatus::Success;
    let view = ToolCallStatusView {
        name: "TestTool",
        arguments: &arguments,
        display_value: None,
        diff_preview: None,
        status: &status,
        tick: 0,
    };

    let lines = view.render(&ctx());
    assert_eq!(lines.len(), 1);
    let expected = format!("✓ TestTool {}", "a".repeat(MAX_TOOL_ARG_LENGTH - 2));
    let width = expected.len() + 10;
    let term = render_lines(&lines, width as u16, 24);
    let output = term.get_lines();
    assert_eq!(output[0], expected);
}

#[test]
fn view_running_hides_raw_args_then_shows_display_value() {
    let status = ToolCallStatus::Running;
    let view = ToolCallStatusView {
        name: "Read",
        arguments: r#"{"file_path":"/path/to/main.rs"}"#,
        display_value: None,
        diff_preview: None,
        status: &status,
        tick: 0,
    };

    // While running with no display_value, raw args are hidden
    let lines = view.render(&ctx());
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    assert!(!output[0].contains("file_path"));
    assert_eq!(output[0], format!("{} Read", FRAMES[0]));

    // After display_value arrives, it is shown
    let view = ToolCallStatusView {
        display_value: Some("main.rs"),
        ..view
    };
    let lines = view.render(&ctx());
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    assert_eq!(output[0], format!("{} Read (main.rs)", FRAMES[0]));
}

#[test]
fn sub_agent_tool_call_renders_nested() {
    let mut statuses = ToolCallStatuses::new();
    statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

    statuses.on_sub_agent_progress(&make_sub_agent_notification(
        "parent-1",
        "explorer",
        r#"{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{\"pattern\":\"test\"}"},"model_name":"m"}}"#,
    ));

    let lines = statuses.render_tool("parent-1", &ctx());
    assert_eq!(lines.len(), 2);
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("explorer"));
    assert!(output[0].starts_with("  "));
    assert!(output[1].starts_with("  └─ "));
    assert!(output[1].contains("grep"));
}

#[test]
fn sub_agent_tool_call_update_appends_chunk() {
    let mut statuses = ToolCallStatuses::new();
    statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

    statuses.on_sub_agent_progress(&make_sub_agent_notification(
        "parent-1",
        "explorer",
        r#"{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":""},"model_name":"m"}}"#,
    ));
    statuses.on_sub_agent_progress(&make_sub_agent_notification(
        "parent-1",
        "explorer",
        r#"{"ToolCallUpdate":{"update":{"id":"c1","chunk":"{\"pattern\":\"updated\"}"}}}"#,
    ));
    statuses.on_sub_agent_progress(&make_sub_agent_notification(
        "parent-1",
        "explorer",
        r#"{"ToolResult":{"result":{"id":"c1","name":"grep","arguments":"{}","result":"ok"},"model_name":"m"}}"#,
    ));

    let lines = statuses.render_tool("parent-1", &ctx());
    assert_eq!(lines.len(), 2);
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    assert!(output[1].contains("updated"));
}

#[test]
fn sub_agent_tool_result_shows_checkmark() {
    let mut statuses = ToolCallStatuses::new();
    statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

    statuses.on_sub_agent_progress(&make_sub_agent_notification(
        "parent-1",
        "explorer",
        r#"{"ToolCall":{"request":{"id":"c1","name":"read_file","arguments":"{}"},"model_name":"m"}}"#,
    ));
    statuses.on_sub_agent_progress(&make_sub_agent_notification(
        "parent-1",
        "explorer",
        r#"{"ToolResult":{"result":{"id":"c1","name":"read_file","arguments":"{}","result":"ok"},"model_name":"m"}}"#,
    ));

    let lines = statuses.render_tool("parent-1", &ctx());
    assert_eq!(lines.len(), 2);
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    assert!(output[1].contains("✓"));
}

#[test]
fn sub_agent_tool_result_uses_result_meta() {
    let mut statuses = ToolCallStatuses::new();
    statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

    statuses.on_sub_agent_progress(&make_sub_agent_notification(
        "parent-1",
        "explorer",
        r#"{"ToolCall":{"request":{"id":"c1","name":"coding__read_file","arguments":"{\"filePath\":\"Cargo.toml\"}"},"model_name":"m"}}"#,
    ));
    statuses.on_sub_agent_progress(&make_sub_agent_notification(
        "parent-1",
        "explorer",
        r#"{"ToolResult":{"result":{"id":"c1","name":"coding__read_file","result_meta":{"display":{"title":"Read file","value":"Cargo.toml, 156 lines"}}},"model_name":"m"}}"#,
    ));

    let lines = statuses.render_tool("parent-1", &ctx());
    assert_eq!(lines.len(), 2);
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    let tool_line = &output[1];
    assert!(tool_line.contains("✓"));
    assert!(tool_line.contains("Read file"));
    assert!(tool_line.contains("(Cargo.toml, 156 lines)"));
    assert!(!tool_line.contains("filePath"));
}

#[test]
fn sub_agent_tool_error_shows_x() {
    let mut statuses = ToolCallStatuses::new();
    statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

    statuses.on_sub_agent_progress(&make_sub_agent_notification(
        "parent-1",
        "explorer",
        r#"{"ToolCall":{"request":{"id":"c1","name":"read_file","arguments":"{}"},"model_name":"m"}}"#,
    ));
    statuses.on_sub_agent_progress(&make_sub_agent_notification(
        "parent-1",
        "explorer",
        r#"{"ToolError":{"error":{"id":"c1","name":"read_file","arguments":"{}","error":"not found"},"model_name":"m"}}"#,
    ));

    let lines = statuses.render_tool("parent-1", &ctx());
    assert_eq!(lines.len(), 2);
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    assert!(output[1].contains("✗"));
}

#[test]
fn multiple_sub_agents_render_separate_headers() {
    let mut statuses = ToolCallStatuses::new();
    statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

    statuses.on_sub_agent_progress(&make_sub_agent_notification(
        "parent-1",
        "explorer",
        r#"{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{}"},"model_name":"m"}}"#,
    ));
    statuses.on_sub_agent_progress(&make_sub_agent_notification(
        "parent-1",
        "writer",
        r#"{"ToolCall":{"request":{"id":"c2","name":"write_file","arguments":"{}"},"model_name":"m"}}"#,
    ));

    let lines = statuses.render_tool("parent-1", &ctx());
    assert_eq!(lines.len(), 5);
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("explorer"));
    assert!(output[3].contains("writer"));
}

#[test]
fn same_name_agents_with_different_task_ids_render_separately() {
    let mut statuses = ToolCallStatuses::new();
    statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

    statuses.on_sub_agent_progress(&make_sub_agent_notification_with_task_id(
        "parent-1",
        "task-1",
        "codebase-explorer",
        r#"{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{}"},"model_name":"m"}}"#,
    ));
    statuses.on_sub_agent_progress(&make_sub_agent_notification_with_task_id(
        "parent-1",
        "task-2",
        "codebase-explorer",
        r#"{"ToolCall":{"request":{"id":"c2","name":"read_file","arguments":"{}"},"model_name":"m"}}"#,
    ));
    statuses.on_sub_agent_progress(&make_sub_agent_notification_with_task_id(
        "parent-1",
        "task-3",
        "codebase-explorer",
        r#"{"ToolCall":{"request":{"id":"c3","name":"list_files","arguments":"{}"},"model_name":"m"}}"#,
    ));

    let lines = statuses.render_tool("parent-1", &ctx());
    assert_eq!(lines.len(), 8);
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    assert!(output[1].contains("grep"));
    assert!(output[4].contains("read_file"));
    assert!(output[7].contains("list_files"));
}

#[test]
fn sub_agent_renders_latest_three_tools_with_overflow() {
    let mut statuses = ToolCallStatuses::new();
    statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

    // Tool 1
    statuses.on_sub_agent_progress(&make_sub_agent_notification(
        "parent-1",
        "explorer",
        r#"{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{}"},"model_name":"m"}}"#,
    ));
    statuses.on_sub_agent_progress(&make_sub_agent_notification(
        "parent-1",
        "explorer",
        r#"{"ToolResult":{"result":{"id":"c1","name":"grep","arguments":"{}","result":"ok"},"model_name":"m"}}"#,
    ));
    // Tool 2
    statuses.on_sub_agent_progress(&make_sub_agent_notification(
        "parent-1",
        "explorer",
        r#"{"ToolCall":{"request":{"id":"c2","name":"read_file","arguments":"{}"},"model_name":"m"}}"#,
    ));
    // Tool 3
    statuses.on_sub_agent_progress(&make_sub_agent_notification(
        "parent-1",
        "explorer",
        r#"{"ToolCall":{"request":{"id":"c3","name":"list_files","arguments":"{}"},"model_name":"m"}}"#,
    ));
    // Tool 4
    statuses.on_sub_agent_progress(&make_sub_agent_notification(
        "parent-1",
        "explorer",
        r#"{"ToolCall":{"request":{"id":"c4","name":"write_file","arguments":"{}"},"model_name":"m"}}"#,
    ));

    let lines = statuses.render_tool("parent-1", &ctx());
    assert_eq!(lines.len(), 5);
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    assert!(output[1].contains("1 earlier tool calls"));
    assert!(output[2].contains("read_file"));
    assert!(output[2].contains("├─"));
    assert!(output[3].contains("list_files"));
    assert!(output[3].contains("├─"));
    assert!(output[4].contains("write_file"));
    assert!(output[4].contains("└─"));
}

#[test]
fn agent_header_shows_spinner_while_running() {
    let mut statuses = ToolCallStatuses::new();
    statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

    statuses.on_sub_agent_progress(&make_sub_agent_notification(
        "parent-1",
        "explorer",
        r#"{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{}"},"model_name":"m"}}"#,
    ));
    statuses.on_sub_agent_progress(&make_sub_agent_notification(
        "parent-1",
        "explorer",
        r#"{"ToolResult":{"result":{"id":"c1","name":"grep","arguments":"{}","result":"ok"},"model_name":"m"}}"#,
    ));

    let lines = statuses.render_tool("parent-1", &ctx());
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    let header = &output[0];
    assert!(
        !header.contains('✓'),
        "Expected spinner, not ✓ in header: {header}"
    );
}

#[test]
fn agent_header_shows_done_after_done_event() {
    let mut statuses = ToolCallStatuses::new();
    statuses.on_tool_call(&make_tool_call("parent-1", "spawn_subagent", None));

    statuses.on_sub_agent_progress(&make_sub_agent_notification(
        "parent-1",
        "explorer",
        r#"{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{}"},"model_name":"m"}}"#,
    ));
    statuses.on_sub_agent_progress(&make_sub_agent_notification(
        "parent-1",
        "explorer",
        r#""Done""#,
    ));

    let lines = statuses.render_tool("parent-1", &ctx());
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    let header = &output[0];
    assert!(header.contains('✓'), "Expected ✓ in header: {header}");
}

#[test]
fn test_display_value_shown_on_completion() {
    let mut statuses = ToolCallStatuses::new();
    statuses.on_tool_call(&make_tool_call("tool-1", "coding__read_file", None));

    let mut meta_map = serde_json::Map::new();
    meta_map.insert("display_value".into(), "Cargo.toml, 156 lines".into());
    let update = acp::ToolCallUpdate::new(
        "tool-1".to_string(),
        acp::ToolCallUpdateFields::new()
            .title("Read file")
            .status(acp::ToolCallStatus::Completed),
    )
    .meta(meta_map);
    statuses.on_tool_call_update(&update);

    let lines = render_all(&statuses, &["tool-1"], &ctx());
    assert_eq!(lines.len(), 1);
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    let text = &output[0];
    assert!(
        text.contains("Read file"),
        "Expected display title in output: {text}"
    );
    assert!(
        text.contains("(Cargo.toml, 156 lines)"),
        "Expected display value in output: {text}"
    );
}

#[test]
fn test_display_value_shown_while_running() {
    let mut statuses = ToolCallStatuses::new();
    statuses.on_tool_call(&make_tool_call(
        "tool-1",
        "Read file",
        Some(r#"{"file_path":"/path/to/main.rs"}"#),
    ));

    let mut meta_map = serde_json::Map::new();
    meta_map.insert("display_value".into(), "main.rs".into());
    let update = acp::ToolCallUpdate::new(
        "tool-1".to_string(),
        acp::ToolCallUpdateFields::new(),
    )
    .meta(meta_map);
    statuses.on_tool_call_update(&update);

    let lines = render_all(&statuses, &["tool-1"], &ctx());
    assert_eq!(lines.len(), 1);
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    let text = &output[0];
    assert!(
        text.contains("(main.rs)"),
        "Expected display value while running: {text}"
    );
    assert!(
        !text.contains("file_path"),
        "Raw args should not appear: {text}"
    );
}
