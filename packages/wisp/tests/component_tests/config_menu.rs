use acp_utils::config_meta::SelectOptionMeta;
use agent_client_protocol::SessionConfigSelectOption;
use tui::ViewContext;
use tui::testing::render_component;
use wisp::components::config_menu::{
    ConfigMenu, ConfigMenuEntry, ConfigMenuEntryKind, ConfigMenuValue,
};
use wisp::tui::Component;

fn make_select_option(
    id: &str,
    name: &str,
    current: &str,
    values: &[(&str, &str)],
) -> agent_client_protocol::SessionConfigOption {
    let options: Vec<SessionConfigSelectOption> = values
        .iter()
        .map(|(v, n)| SessionConfigSelectOption::new(v.to_string(), n.to_string()))
        .collect();
    agent_client_protocol::SessionConfigOption::select(
        id.to_string(),
        name.to_string(),
        current.to_string(),
        options,
    )
}

#[test]
fn component_renders_selected_row() {
    let opts = vec![
        make_select_option(
            "model",
            "Model",
            "gpt-4o",
            &[("gpt-4o", "GPT-4o"), ("claude", "Claude")],
        ),
        make_select_option(
            "mode",
            "Mode",
            "code",
            &[("code", "Code"), ("chat", "Chat")],
        ),
    ];
    let mut menu = ConfigMenu::from_config_options(&opts);

    let term = render_component(|ctx| menu.render(ctx), 80, 24);
    let output = term.get_lines();

    assert!(
        output[0].contains("▶"),
        "first row should have selection indicator"
    );
    assert!(
        output[0].contains("Model"),
        "first row should contain 'Model'"
    );
    assert!(
        output[0].contains("GPT-4o"),
        "first row should contain 'GPT-4o'"
    );
    assert!(
        output[1].contains("Mode"),
        "second row should contain 'Mode'"
    );
    assert!(
        output[1].contains("Code"),
        "second row should contain 'Code'"
    );
    assert!(
        !output[1].contains("▶"),
        "second row should not have selection indicator"
    );
    // Rows after the two entries should be empty
    assert!(output[2].trim().is_empty(), "row 2 should be empty");
}

#[test]
fn empty_options_renders_placeholder() {
    let mut menu = ConfigMenu::from_config_options(&[]);

    let term = render_component(|ctx| menu.render(ctx), 80, 24);
    let output = term.get_lines();

    assert!(
        output[0].contains("no config options"),
        "should show placeholder text"
    );
    // Second row should be empty
    assert!(output[1].trim().is_empty(), "row 1 should be empty");
}

#[test]
fn multi_select_with_display_name_not_dimmed_when_first_value_disabled() {
    let mut menu = ConfigMenu::from_entries(vec![ConfigMenuEntry {
        config_id: "model".to_string(),
        title: "Model".to_string(),
        values: vec![
            ConfigMenuValue {
                value: "a".to_string(),
                name: "Alpha".to_string(),
                description: Some("Unavailable: no key".to_string()),
                is_disabled: true,
                meta: SelectOptionMeta::default(),
            },
            ConfigMenuValue {
                value: "b".to_string(),
                name: "Beta".to_string(),
                description: None,
                is_disabled: false,
                meta: SelectOptionMeta::default(),
            },
        ],
        current_value_index: 0,
        current_raw_value: "b,a".to_string(),
        entry_kind: ConfigMenuEntryKind::Select,
        multi_select: true,
        display_name: Some("Beta, Alpha".to_string()),
    }]);

    let ctx = ViewContext::new((80, 24));
    let term = render_component(|c| menu.render(c), 80, 24);
    let style = term.style_of_text(0, "Beta, Alpha").unwrap();
    assert_eq!(
        style.bg,
        Some(ctx.theme.highlight_bg()),
        "multi-select with display_name should get highlight_bg, not muted"
    );
}
