use agent_client_protocol as acp;
use tui::KeyCode;
use tui::KeyEvent;
use tui::KeyEventKind;
use tui::KeyEventState;
use tui::KeyModifiers;
use tui::testing::TestTerminal;

use super::common::*;

#[tokio::test]
async fn test_settings_command_opens_menu() {
    let config_options = make_settings_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options, (80, 24));
    renderer.initial_render().unwrap();

    type_string(&mut renderer, "/settings").await;
    press_enter(&mut renderer).await;

    // Settings menu should open; picker requires explicit Enter
    assert!(
        has_settings_menu(renderer.writer()),
        "Settings menu should be visible"
    );
    assert!(
        !has_settings_picker(renderer.writer()),
        "Settings picker should not be visible"
    );
}

#[tokio::test]
async fn test_settings_menu_esc_closes() {
    let config_options = make_settings_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options, (80, 24));
    renderer.initial_render().unwrap();

    type_string(&mut renderer, "/settings").await;
    press_enter(&mut renderer).await;
    assert!(
        has_settings_menu(renderer.writer()),
        "Settings menu should be visible"
    );
    assert!(
        !has_settings_picker(renderer.writer()),
        "Settings picker should not be visible"
    );

    // Open the picker by pressing Enter on the selected menu entry
    press_enter(&mut renderer).await;
    assert!(
        has_settings_menu(renderer.writer()),
        "Settings menu should be visible"
    );
    assert!(
        has_settings_picker(renderer.writer()),
        "Settings picker should be visible"
    );

    // First ESC closes the picker
    send_key(&mut renderer, KeyCode::Esc, KeyModifiers::empty()).await;
    assert!(
        has_settings_menu(renderer.writer()),
        "Settings menu should be visible"
    );
    assert!(
        !has_settings_picker(renderer.writer()),
        "Settings picker should not be visible"
    );

    // Second ESC closes the menu
    send_key(&mut renderer, KeyCode::Esc, KeyModifiers::empty()).await;
    assert!(
        !has_settings_menu(renderer.writer()),
        "Settings menu should not be visible"
    );
}

#[tokio::test]
async fn test_settings_menu_arrow_navigation_single_entry() {
    let config_options = make_settings_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options, (80, 24));
    renderer.initial_render().unwrap();

    type_string(&mut renderer, "/settings").await;
    press_enter(&mut renderer).await;

    // With single settings option + Theme + MCP servers, menu has 3 entries: Model, Theme, MCP Servers
    assert!(
        has_settings_menu(renderer.writer()),
        "Settings menu should be visible"
    );
    assert!(
        !has_settings_picker(renderer.writer()),
        "Settings picker should not be visible"
    );
    let label = settings_menu_selected_label(renderer.writer());
    assert!(
        label.as_deref().is_some_and(|l| l.contains("Model")),
        "Initial selection should be Model, got: {label:?}"
    );

    // Down goes to Theme (index 1)
    send_key(&mut renderer, KeyCode::Down, KeyModifiers::empty()).await;
    let label = settings_menu_selected_label(renderer.writer());
    assert!(
        label.as_deref().is_some_and(|l| l.contains("Theme")),
        "Second selection should be Theme, got: {label:?}"
    );

    // Down again goes to MCP Servers (index 2)
    send_key(&mut renderer, KeyCode::Down, KeyModifiers::empty()).await;
    let label = settings_menu_selected_label(renderer.writer());
    assert!(
        label.as_deref().is_some_and(|l| l.contains("MCP Servers")),
        "Third selection should be MCP Servers, got: {label:?}"
    );

    // Down again wraps back to Model (index 0)
    send_key(&mut renderer, KeyCode::Down, KeyModifiers::empty()).await;
    let label = settings_menu_selected_label(renderer.writer());
    assert!(
        label.as_deref().is_some_and(|l| l.contains("Model")),
        "Wrapped selection should be Model, got: {label:?}"
    );
}

#[tokio::test]
async fn test_settings_single_option_shows_model_picker() {
    let config_options = make_settings_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options, (80, 24));
    renderer.initial_render().unwrap();

    type_string(&mut renderer, "/settings").await;
    press_enter(&mut renderer).await;

    // Menu opens; press Enter to open the model picker
    assert!(
        has_settings_menu(renderer.writer()),
        "Settings menu should be visible"
    );
    press_enter(&mut renderer).await;

    assert!(
        has_settings_picker(renderer.writer()),
        "Settings picker should be visible"
    );
    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains("Model search")),
        "Should show model overlay directly.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_settings_picker_focuses_cursor_on_overlay_query() {
    let config_options = make_settings_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options, (80, 24));
    renderer.initial_render().unwrap();

    type_string(&mut renderer, "/settings").await;
    press_enter(&mut renderer).await;
    // Open the picker from the menu
    press_enter(&mut renderer).await;

    let lines = renderer.writer().get_lines();
    #[allow(clippy::cast_possible_truncation)]
    let search_row = lines
        .iter()
        .position(|l| l.contains("Model search:"))
        .expect("model search header row should be rendered") as u16;
    let (cursor_col, cursor_row) = renderer.writer().cursor_position();

    assert_eq!(
        cursor_row,
        search_row,
        "Cursor should be on overlay search row.\nBuffer:\n{}",
        lines.join("\n")
    );
    // Overlay border "│ " (2 cols) + "  Model search: " (16 cols) = 18
    assert_eq!(cursor_col, 18);
}

#[tokio::test]
async fn test_settings_picker_filters_model_options() {
    let config_options = make_settings_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options, (80, 24));
    renderer.initial_render().unwrap();

    type_string(&mut renderer, "/settings").await;
    press_enter(&mut renderer).await;
    // Open the picker from the menu
    press_enter(&mut renderer).await;

    type_string(&mut renderer, "claude").await;

    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains("Claude Sonnet")),
        "Should show fuzzy-matched model result.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_settings_menu_swallows_other_keys() {
    let config_options = vec![
        acp::SessionConfigOption::select(
            "model",
            "Model",
            "m1",
            vec![acp::SessionConfigSelectOption::new("m1", "M1")],
        ),
        acp::SessionConfigOption::select(
            "theme",
            "Theme",
            "dark",
            vec![acp::SessionConfigSelectOption::new("dark", "Dark")],
        ),
    ];

    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options, (80, 24));
    renderer.initial_render().unwrap();

    type_string(&mut renderer, "/settings").await;
    press_enter(&mut renderer).await;
    assert!(
        has_settings_menu(renderer.writer()),
        "Settings menu should be visible"
    );

    // Typing a character should not modify input buffer
    send_key(&mut renderer, KeyCode::Char('z'), KeyModifiers::empty()).await;

    // Settings menu should still be open
    assert!(
        has_settings_menu(renderer.writer()),
        "Settings menu should be visible"
    );
    // Input prompt is not rendered while overlay is open, so 'z' shouldn't appear anywhere
    let lines = renderer.writer().get_lines();
    assert!(
        !lines.iter().any(|l| l.contains('z')),
        "Typed char should be swallowed while settings overlay is open.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_settings_menu_ctrl_c_exits() {
    let config_options = make_settings_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options, (80, 24));
    renderer.initial_render().unwrap();

    type_string(&mut renderer, "/settings").await;
    press_enter(&mut renderer).await;
    assert!(
        has_settings_menu(renderer.writer()),
        "Settings menu should be visible"
    );

    // Ctrl+C should still exit even with settings menu open
    let action = renderer
        .on_key_event(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        })
        .await
        .unwrap();

    assert!(matches!(action, LoopAction::Exit));
}

#[tokio::test]
async fn test_settings_menu_updates_on_config_option_event() {
    let config_options = make_settings_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options, (80, 24));
    renderer.initial_render().unwrap();

    type_string(&mut renderer, "/settings").await;
    press_enter(&mut renderer).await;
    assert!(
        has_settings_menu(renderer.writer()),
        "Settings menu should be visible"
    );

    // Simulate the agent responding with updated settings
    let new_config = vec![
        acp::SessionConfigOption::select(
            "model".to_string(),
            "Model".to_string(),
            "anthropic:claude-sonnet-4-5".to_string(),
            vec![
                acp::SessionConfigSelectOption::new(
                    "openrouter:openai/gpt-4o".to_string(),
                    "OpenRouter / GPT-4o".to_string(),
                ),
                acp::SessionConfigSelectOption::new(
                    "anthropic:claude-sonnet-4-5".to_string(),
                    "Anthropic / Claude Sonnet 4.5".to_string(),
                ),
            ],
        )
        .category(acp::SessionConfigOptionCategory::Model),
    ];

    renderer
        .on_session_update(acp::SessionUpdate::ConfigOptionUpdate(
            acp::ConfigOptionUpdate::new(new_config),
        ))
        .unwrap();

    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains("Claude Sonnet")),
        "Menu should reflect updated settings.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_settings_clears_input_buffer() {
    let config_options = make_settings_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options, (80, 24));
    renderer.initial_render().unwrap();

    type_string(&mut renderer, "/settings").await;
    press_enter(&mut renderer).await;

    // Input buffer should be cleared
    let lines = renderer.writer().get_lines();
    // The prompt line should not contain "/settings"
    assert!(
        !lines.iter().any(|l| l.contains("/settings")),
        "Input buffer should be cleared after /settings.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_settings_with_no_options_shows_placeholder() {
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[], (80, 24));
    renderer.initial_render().unwrap();

    type_string(&mut renderer, "/settings").await;
    press_enter(&mut renderer).await;

    assert!(
        has_settings_menu(renderer.writer()),
        "Settings menu should be visible"
    );
    let lines = renderer.writer().get_lines();
    // Even with no settings options, the MCP Servers entry is always present
    assert!(
        lines.iter().any(|l| l.contains("MCP Servers")),
        "Should show MCP Servers entry even when no settings options.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_settings_overlay_renders_after_large_overflow_scrollback() {
    let config_options = make_settings_options();
    // Small viewport height to force overflow quickly (width must fit status line)
    let terminal = TestTerminal::new(80, 8);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options, (80, 8));
    renderer.initial_render().unwrap();

    // Feed a LOT of content in a single streaming response (no prompt_done)
    // This causes progressive flush to build up flushed_visual_count
    for i in 0..50 {
        let chunk = format!("Line {i:02} with enough content to wrap in 40 cols");
        renderer
            .on_session_update(acp::SessionUpdate::AgentMessageChunk(
                acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new(&chunk))),
            ))
            .unwrap();
    }

    // Now open settings overlay WHILE still in the streaming context
    // This is where the bug manifests - flushed_visual_count is high
    // but the overlay produces fewer lines

    type_string(&mut renderer, "/settings").await;
    press_enter(&mut renderer).await;

    // Assert overlay state is correct
    assert!(
        has_settings_menu(renderer.writer()),
        "Settings menu should be open"
    );

    // Assert overlay content is actually visible
    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains("Configuration")),
        "Configuration header should be visible in overlay.\nBuffer:\n{}",
        lines.join("\n")
    );
    assert!(
        lines.iter().any(|l| l.contains("Model")),
        "Model option should be visible in overlay.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_settings_overlay_open_close_after_overflow_keeps_prompt_and_layout_valid() {
    let config_options = make_settings_options();
    let terminal = TestTerminal::new(80, 8);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options, (80, 8));
    renderer.initial_render().unwrap();

    // Create overflow history within a single streaming response
    for i in 0..50 {
        let chunk = format!("Line {i:02} with enough content to wrap in 40 cols");
        renderer
            .on_session_update(acp::SessionUpdate::AgentMessageChunk(
                acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new(&chunk))),
            ))
            .unwrap();
    }

    // Open settings overlay while flushed_visual_count is high
    type_string(&mut renderer, "/settings").await;
    press_enter(&mut renderer).await;

    // Verify overlay rendered correctly
    assert!(
        has_settings_menu(renderer.writer()),
        "Settings menu should be visible"
    );
    let lines_before = renderer.writer().get_lines();
    assert!(
        lines_before.iter().any(|l| l.contains("Configuration")),
        "Configuration should be visible before closing.\nBuffer:\n{}",
        lines_before.join("\n")
    );

    // Close overlay with Esc
    send_key(&mut renderer, KeyCode::Esc, KeyModifiers::empty()).await;

    // Verify normal prompt rendering resumes
    assert!(
        !has_settings_menu(renderer.writer()),
        "Settings menu should not be visible"
    );
    let lines_after = renderer.writer().get_lines();

    // Prompt border/status line should be visible
    assert!(
        lines_after
            .iter()
            .any(|l| l.contains('╭') || l.contains('╰')),
        "Prompt border should be visible after closing overlay.\nBuffer:\n{}",
        lines_after.join("\n")
    );

    // Should not have an empty managed frame (at least some content should render)
    let has_content = lines_after.iter().any(|l| !l.trim().is_empty());
    assert!(
        has_content,
        "Frame should not be empty after closing overlay.\nBuffer:\n{}",
        lines_after.join("\n")
    );
}

#[tokio::test]
async fn test_settings_option_update_refreshes_mode_display() {
    let initial = vec![
        acp::SessionConfigOption::select(
            "mode",
            "Mode",
            "planner",
            vec![
                acp::SessionConfigSelectOption::new("planner", "Planner"),
                acp::SessionConfigSelectOption::new("coder", "Coder"),
            ],
        )
        .category(acp::SessionConfigOptionCategory::Mode),
    ];

    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &initial, (TEST_WIDTH, 40));
    renderer.initial_render().unwrap();

    let updated = vec![
        acp::SessionConfigOption::select(
            "mode",
            "Mode",
            "coder",
            vec![
                acp::SessionConfigSelectOption::new("planner", "Planner"),
                acp::SessionConfigSelectOption::new("coder", "Coder"),
            ],
        )
        .category(acp::SessionConfigOptionCategory::Mode),
    ];

    renderer
        .on_session_update(acp::SessionUpdate::ConfigOptionUpdate(
            acp::ConfigOptionUpdate::new(updated),
        ))
        .unwrap();

    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains("Coder")),
        "Status line should show updated mode 'Coder'.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_server_status_notification_updates_overlay_state() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[], (TEST_WIDTH, 40));
    renderer.initial_render().unwrap();

    type_string(&mut renderer, "/settings").await;
    press_enter(&mut renderer).await;
    assert!(
        has_settings_menu(renderer.writer()),
        "Settings overlay should be visible"
    );

    let notification =
        acp::ExtNotification::from(acp_utils::notifications::McpNotification::ServerStatus {
            servers: vec![acp_utils::notifications::McpServerStatusEntry {
                name: "docs".to_string(),
                status: acp_utils::notifications::McpServerStatus::Connected { tool_count: 0 },
            }],
        });

    renderer.on_ext_notification(notification).unwrap();

    assert!(
        has_settings_menu(renderer.writer()),
        "Settings overlay should still be visible after server status update"
    );
}
