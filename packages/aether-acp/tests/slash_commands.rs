use agent_client_protocol as acp;
use rmcp::model::{Prompt, PromptArgument};

/// Test helper to create an MCP prompt
fn create_test_prompt(name: &str, description: Option<&str>, args: Vec<&str>) -> Prompt {
    let arguments = if args.is_empty() {
        None
    } else {
        Some(
            args.into_iter()
                .map(|name| PromptArgument {
                    name: name.into(),
                    description: None,
                    required: Some(true),
                    title: None,
                })
                .collect(),
        )
    };

    Prompt::new(
        name.to_string(),
        description.map(|s| s.to_string()),
        arguments,
    )
}

#[test]
fn test_map_prompt_to_command_strips_namespace() {
    let prompt = create_test_prompt("mcp-lexicon__web", Some("Search the web"), vec![]);

    let command = aether_acp::mappers::map_mcp_prompt_to_available_command(&prompt);

    assert_eq!(command.name, "web");
    assert_eq!(command.description, "Search the web");
    // All commands now have input hints for optional arguments
    assert!(command.input.is_some());
}

#[test]
fn test_map_prompt_to_command_without_namespace() {
    let prompt = create_test_prompt("test", Some("Run tests"), vec![]);

    let command = aether_acp::mappers::map_mcp_prompt_to_available_command(&prompt);

    assert_eq!(command.name, "test");
    assert_eq!(command.description, "Run tests");
}

#[test]
fn test_map_prompt_to_command_with_arguments() {
    let prompt = create_test_prompt(
        "mcp-lexicon__search",
        Some("Search code"),
        vec!["query", "pattern"],
    );

    let command = aether_acp::mappers::map_mcp_prompt_to_available_command(&prompt);

    assert_eq!(command.name, "search");
    match command.input {
        Some(acp::AvailableCommandInput::Unstructured(input)) => {
            assert_eq!(input.hint, "query pattern");
        }
        _ => panic!("Expected Unstructured input with hint"),
    }
}

#[test]
fn test_map_prompt_to_command_no_description() {
    let prompt = create_test_prompt("foo__bar", None, vec![]);

    let command = aether_acp::mappers::map_mcp_prompt_to_available_command(&prompt);

    assert_eq!(command.name, "bar");
    assert_eq!(command.description, "No description available");
}

#[test]
fn test_map_prompt_to_command_empty_arguments() {
    let prompt = create_test_prompt("cmd", Some("A command"), vec![]);

    let command = aether_acp::mappers::map_mcp_prompt_to_available_command(&prompt);

    // All commands now have input hints for optional arguments
    assert!(command.input.is_some());
}
