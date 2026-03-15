use acp_utils::config_option_id::ConfigOptionId;
use agent_client_protocol::{self as acp, SessionConfigOption, SessionConfigOptionCategory};
use tui::ViewContext;
use tui::testing::render_lines;
use wisp::components::status_line::StatusLine;

fn mode_option(value: impl Into<String>, name: impl Into<String>) -> SessionConfigOption {
    let value = value.into();
    let name = name.into();
    SessionConfigOption::select(
        "mode",
        "Mode",
        value.clone(),
        vec![acp::SessionConfigSelectOption::new(value, name)],
    )
    .category(SessionConfigOptionCategory::Mode)
}

fn model_option(value: impl Into<String>, name: impl Into<String>) -> SessionConfigOption {
    let value = value.into();
    let name = name.into();
    SessionConfigOption::select(
        "model",
        "Model",
        value.clone(),
        vec![acp::SessionConfigSelectOption::new(value, name)],
    )
    .category(SessionConfigOptionCategory::Model)
}

fn reasoning_option(value: impl Into<String>) -> SessionConfigOption {
    let value = value.into();
    SessionConfigOption::select(
        ConfigOptionId::ReasoningEffort.as_str(),
        "Reasoning",
        value,
        vec![
            acp::SessionConfigSelectOption::new("none", "None"),
            acp::SessionConfigSelectOption::new("low", "Low"),
            acp::SessionConfigSelectOption::new("medium", "Medium"),
            acp::SessionConfigSelectOption::new("high", "High"),
        ],
    )
}

#[test]
fn renders_agent_name() {
    let status = StatusLine {
        agent_name: "claude-code",
        config_options: &[],
        context_pct_left: None,
        waiting_for_response: false,
        unhealthy_server_count: 0,
    };
    let ctx = ViewContext::new((80, 24));
    let term = render_lines(&status.render(&ctx), 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("claude-code"));
}

#[test]
fn renders_with_indentation() {
    let status = StatusLine {
        agent_name: "test-agent",
        config_options: &[],
        context_pct_left: None,
        waiting_for_response: false,
        unhealthy_server_count: 0,
    };
    let ctx = ViewContext::new((80, 24));
    let term = render_lines(&status.render(&ctx), 80, 24);
    let output = term.get_lines();
    // Should have leading spaces for indentation
    assert!(output[0].contains("  test-agent"));
}

#[test]
fn renders_model_display() {
    let options = vec![model_option("gpt-4o", "gpt-4o")];
    let status = StatusLine {
        agent_name: "aether-acp",
        config_options: &options,
        context_pct_left: None,
        waiting_for_response: false,
        unhealthy_server_count: 0,
    };
    let ctx = ViewContext::new((80, 24));
    let term = render_lines(&status.render(&ctx), 80, 24);
    let output = term.get_lines();
    assert!(
        output[0].contains("aether-acp"),
        "should contain agent name"
    );
    assert!(output[0].contains("gpt-4o"), "should contain model name");
}

#[test]
fn renders_without_model_when_none() {
    let status = StatusLine {
        agent_name: "aether-acp",
        config_options: &[],
        context_pct_left: None,
        waiting_for_response: false,
        unhealthy_server_count: 0,
    };
    let ctx = ViewContext::new((80, 24));
    let term = render_lines(&status.render(&ctx), 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("aether-acp"));
    assert!(
        !output[0].contains("·"),
        "should not contain separator when no model"
    );
}

#[test]
fn renders_context_usage_right_aligned() {
    let options = vec![model_option("gpt-4o", "gpt-4o")];
    let status = StatusLine {
        agent_name: "aether",
        config_options: &options,
        context_pct_left: Some(72),
        waiting_for_response: false,
        unhealthy_server_count: 0,
    };
    let ctx = ViewContext::new((80, 24));
    let term = render_lines(&status.render(&ctx), 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("aether"), "should contain agent name");
    assert!(
        output[0].contains("72% context"),
        "should contain context usage"
    );
}

#[test]
fn does_not_render_context_when_none() {
    let options = vec![model_option("gpt-4o", "gpt-4o")];
    let status = StatusLine {
        agent_name: "aether",
        config_options: &options,
        context_pct_left: None,
        waiting_for_response: false,
        unhealthy_server_count: 0,
    };
    let ctx = ViewContext::new((80, 24));
    let term = render_lines(&status.render(&ctx), 80, 24);
    let output = term.get_lines();
    assert!(
        !output[0].contains("context"),
        "should not contain context info"
    );
}

#[test]
fn renders_interrupt_message_when_waiting() {
    let options = vec![model_option("gpt-4o", "gpt-4o")];
    let status = StatusLine {
        agent_name: "aether",
        config_options: &options,
        context_pct_left: Some(72),
        waiting_for_response: true,
        unhealthy_server_count: 0,
    };
    let ctx = ViewContext::new((80, 24));
    let term = render_lines(&status.render(&ctx), 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("aether"), "should contain agent name");
    assert!(
        output[0].contains("esc to interrupt"),
        "should contain interrupt message"
    );
    assert!(
        output[0].contains("72% context"),
        "should contain context when waiting"
    );
}

#[test]
fn renders_interrupt_message_without_model_when_waiting() {
    let status = StatusLine {
        agent_name: "aether",
        config_options: &[],
        context_pct_left: None,
        waiting_for_response: true,
        unhealthy_server_count: 0,
    };
    let ctx = ViewContext::new((80, 24));
    let term = render_lines(&status.render(&ctx), 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("aether"), "should contain agent name");
    assert!(
        output[0].contains("esc to interrupt"),
        "should contain interrupt message"
    );
}

#[test]
fn renders_unhealthy_server_singular() {
    let options = vec![model_option("gpt-4o", "gpt-4o")];
    let status = StatusLine {
        agent_name: "aether",
        config_options: &options,
        context_pct_left: None,
        waiting_for_response: false,
        unhealthy_server_count: 1,
    };
    let ctx = ViewContext::new((80, 24));
    let term = render_lines(&status.render(&ctx), 80, 24);
    let output = term.get_lines();
    assert!(
        output[0].contains("1 server needs auth"),
        "should show singular unhealthy message"
    );
}

#[test]
fn renders_unhealthy_servers_plural() {
    let status = StatusLine {
        agent_name: "aether",
        config_options: &[],
        context_pct_left: None,
        waiting_for_response: false,
        unhealthy_server_count: 3,
    };
    let ctx = ViewContext::new((80, 24));
    let term = render_lines(&status.render(&ctx), 80, 24);
    let output = term.get_lines();
    assert!(
        output[0].contains("3 servers unhealthy"),
        "should show plural unhealthy message"
    );
}

#[test]
fn zero_unhealthy_servers_shows_nothing() {
    let status = StatusLine {
        agent_name: "aether",
        config_options: &[],
        context_pct_left: None,
        waiting_for_response: false,
        unhealthy_server_count: 0,
    };
    let ctx = ViewContext::new((80, 24));
    let term = render_lines(&status.render(&ctx), 80, 24);
    let output = term.get_lines();
    assert!(
        !output[0].contains("server"),
        "should not show server info when count is 0"
    );
}

#[test]
fn context_usage_takes_precedence_over_unhealthy() {
    let status = StatusLine {
        agent_name: "aether",
        config_options: &[],
        context_pct_left: Some(50),
        waiting_for_response: false,
        unhealthy_server_count: 2,
    };
    let ctx = ViewContext::new((80, 24));
    let term = render_lines(&status.render(&ctx), 80, 24);
    let output = term.get_lines();
    assert!(
        output[0].contains("50% context"),
        "context should take precedence"
    );
    assert!(
        !output[0].contains("unhealthy"),
        "should not show unhealthy when context is shown"
    );
}

#[test]
fn renders_agent_mode_model_in_order() {
    let options = vec![
        mode_option("planner", "Planner"),
        model_option("gpt-4o", "gpt-4o"),
    ];
    let status = StatusLine {
        agent_name: "wisp",
        config_options: &options,
        context_pct_left: None,
        waiting_for_response: false,
        unhealthy_server_count: 0,
    };
    let ctx = ViewContext::new((80, 24));
    let term = render_lines(&status.render(&ctx), 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("wisp"), "should contain agent name");
    assert!(output[0].contains("Planner"), "should contain mode");
    assert!(output[0].contains("gpt-4o"), "should contain model");

    // Verify order: agent name should appear before mode, mode before model
    let agent_index = output[0].find("wisp").expect("agent position");
    let mode_index = output[0].find("Planner").expect("mode position");
    let llm_index = output[0].find("gpt-4o").expect("model position");
    assert!(
        agent_index < mode_index,
        "agent should come before mode in status line"
    );
    assert!(
        mode_index < llm_index,
        "mode should come before model in status line"
    );
}

#[test]
fn renders_mode_with_secondary_color() {
    let options = vec![mode_option("planner", "Planner")];
    let status = StatusLine {
        agent_name: "wisp",
        config_options: &options,
        context_pct_left: None,
        waiting_for_response: false,
        unhealthy_server_count: 0,
    };
    let ctx = ViewContext::new((80, 24));
    let term = render_lines(&status.render(&ctx), 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("Planner"));
    let style = term.style_of_text(0, "Planner").unwrap();
    assert_eq!(
        style.fg,
        Some(ctx.theme.secondary()),
        "mode text should be colored with secondary theme color"
    );
}

#[test]
fn renders_agent_with_info_color() {
    let status = StatusLine {
        agent_name: "wisp",
        config_options: &[],
        context_pct_left: None,
        waiting_for_response: false,
        unhealthy_server_count: 0,
    };
    let ctx = ViewContext::new((80, 24));
    let term = render_lines(&status.render(&ctx), 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("wisp"));
    let style = term.style_of_text(0, "wisp").unwrap();
    assert_eq!(
        style.fg,
        Some(ctx.theme.info()),
        "agent name should be colored with info theme color"
    );
}

#[test]
fn renders_model_with_success_color() {
    let options = vec![model_option("gpt-4o", "gpt-4o")];
    let status = StatusLine {
        agent_name: "wisp",
        config_options: &options,
        context_pct_left: None,
        waiting_for_response: false,
        unhealthy_server_count: 0,
    };
    let ctx = ViewContext::new((80, 24));
    let term = render_lines(&status.render(&ctx), 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("gpt-4o"));
    let style = term.style_of_text(0, "gpt-4o").unwrap();
    assert_eq!(
        style.fg,
        Some(ctx.theme.success()),
        "model name should be colored with success theme color"
    );
}

#[test]
fn renders_each_element_with_distinct_color() {
    let options = vec![
        mode_option("planner", "Planner"),
        model_option("gpt-4o", "gpt-4o"),
    ];
    let status = StatusLine {
        agent_name: "wisp",
        config_options: &options,
        context_pct_left: None,
        waiting_for_response: false,
        unhealthy_server_count: 0,
    };
    let ctx = ViewContext::new((80, 24));
    let term = render_lines(&status.render(&ctx), 80, 24);

    let agent_style = term.style_of_text(0, "wisp");
    let mode_style = term.style_of_text(0, "Planner");
    let model_style = term.style_of_text(0, "gpt-4o");

    assert_ne!(
        agent_style.map(|s| s.fg),
        mode_style.map(|s| s.fg),
        "agent and mode should have different colors"
    );
    assert_ne!(
        mode_style.map(|s| s.fg),
        model_style.map(|s| s.fg),
        "mode and model should have different colors"
    );
    assert_ne!(
        agent_style.map(|s| s.fg),
        model_style.map(|s| s.fg),
        "agent and model should have different colors"
    );
}

#[test]
fn renders_reasoning_bar_next_to_model_when_reasoning_set() {
    let options = vec![model_option("gpt-4o", "gpt-4o"), reasoning_option("medium")];
    let status = StatusLine {
        agent_name: "wisp",
        config_options: &options,
        context_pct_left: None,
        waiting_for_response: false,
        unhealthy_server_count: 0,
    };
    let ctx = ViewContext::new((80, 24));
    let term = render_lines(&status.render(&ctx), 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("gpt-4o"), "should contain model name");
    assert!(
        output[0].contains("[■■·]"),
        "should contain reasoning bar for medium effort"
    );
    // Verify order: model should appear before bar
    let model_index = output[0].find("gpt-4o").expect("model position");
    let bar_index = output[0].find("[■■·]").expect("bar position");
    assert!(
        model_index < bar_index,
        "model should come before reasoning bar"
    );
}

#[test]
fn does_not_render_reasoning_bar_when_model_absent() {
    let options = vec![reasoning_option("high")];
    let status = StatusLine {
        agent_name: "wisp",
        config_options: &options,
        context_pct_left: None,
        waiting_for_response: false,
        unhealthy_server_count: 0,
    };
    let ctx = ViewContext::new((80, 24));
    let term = render_lines(&status.render(&ctx), 80, 24);
    let output = term.get_lines();
    assert!(
        !output[0].contains('■'),
        "should not contain filled bar chars"
    );
    assert!(
        !output[0].contains('·'),
        "should not contain empty bar chars"
    );
}

#[test]
fn renders_empty_reasoning_bar_for_none_effort() {
    let options = vec![model_option("gpt-4o", "gpt-4o")];
    let status = StatusLine {
        agent_name: "wisp",
        config_options: &options,
        context_pct_left: None,
        waiting_for_response: false,
        unhealthy_server_count: 0,
    };
    let ctx = ViewContext::new((80, 24));
    let term = render_lines(&status.render(&ctx), 80, 24);
    let output = term.get_lines();
    assert!(
        output[0].contains("[···]"),
        "should contain empty reasoning bar"
    );
}

#[test]
fn renders_reasoning_bar_with_model_semantic_color() {
    let options = vec![model_option("gpt-4o", "gpt-4o"), reasoning_option("low")];
    let status = StatusLine {
        agent_name: "wisp",
        config_options: &options,
        context_pct_left: None,
        waiting_for_response: false,
        unhealthy_server_count: 0,
    };
    let ctx = ViewContext::new((80, 24));
    let term = render_lines(&status.render(&ctx), 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("■"));
    let style = term.style_of_text(0, "■").unwrap();
    assert_eq!(
        style.fg,
        Some(ctx.theme.success()),
        "reasoning bar should use success color (same as model)"
    );
}
