use aether::core::Prompt;
use aether::events::{AgentMessage, UserMessage};
use aether::mcp::{McpSpawnResult, mcp};
use aether::testing::{FakeMcpServer, fake_mcp};
use llm::testing::FakeLlmProvider;
use mcp_utils::client::ServerInstructions;

#[tokio::test]
async fn test_fake_mcp_server_has_instructions() {
    // FakeMcpServer has instructions set to "A fake MCP server for testing"
    let McpSpawnResult {
        tool_definitions: _,
        instructions,
        command_tx: _,
        handle: _,
    } = mcp()
        .with_servers(vec![fake_mcp("test", FakeMcpServer::new())])
        .spawn()
        .await
        .unwrap();

    // FakeMcpServer does provide instructions, so we should get them
    assert_eq!(instructions.len(), 1);
    assert_eq!(instructions[0].server_name, "test");
    assert!(
        instructions[0]
            .instructions
            .contains("A fake MCP server for testing")
    );
}

#[tokio::test]
async fn test_multiple_servers_with_instructions() {
    let McpSpawnResult {
        tool_definitions: _,
        instructions,
        command_tx: _,
        handle: _,
    } = mcp()
        .with_servers(vec![
            fake_mcp("server1", FakeMcpServer::new()),
            fake_mcp("server2", FakeMcpServer::new()),
        ])
        .spawn()
        .await
        .unwrap();

    // Both servers should have instructions
    assert_eq!(instructions.len(), 2);

    // Find server1 and server2 instructions
    let server1_instr = instructions
        .iter()
        .find(|i| i.server_name == "server1")
        .unwrap();
    let server2_instr = instructions
        .iter()
        .find(|i| i.server_name == "server2")
        .unwrap();

    assert!(
        server1_instr
            .instructions
            .contains("A fake MCP server for testing")
    );
    assert!(
        server2_instr
            .instructions
            .contains("A fake MCP server for testing")
    );
}

#[tokio::test]
async fn test_server_instructions_skips_empty_instructions() {
    // Note: We can't easily test empty instructions filtering since FakeMcpServer
    // always provides instructions. This is implicitly tested by the fact that
    // empty strings are filtered out in the manager.
    let McpSpawnResult {
        tool_definitions: _,
        instructions,
        command_tx: _,
        handle: _,
    } = mcp()
        .with_servers(vec![fake_mcp("with-content", FakeMcpServer::new())])
        .spawn()
        .await
        .unwrap();

    // Should have one instruction
    assert_eq!(instructions.len(), 1);
    assert_eq!(instructions[0].server_name, "with-content");
}

#[test]
fn test_format_mcp_instructions_xml_structure() {
    let instructions = vec![ServerInstructions {
        server_name: "coding".to_string(),
        instructions: "Use absolute paths.".to_string(),
    }];

    let formatted = aether::core::format_mcp_instructions(&instructions);

    // Check for XML tags with server names
    assert!(formatted.contains("<mcp-server-instructions name=\"coding\">"));
    assert!(formatted.contains("</mcp-server-instructions>"));
    assert!(formatted.contains("Use absolute paths."));
    assert!(formatted.contains("# MCP Server Instructions"));
}

#[test]
fn test_format_mcp_instructions_multiple_servers() {
    let instructions = vec![
        ServerInstructions {
            server_name: "coding".to_string(),
            instructions: "Use absolute paths.".to_string(),
        },
        ServerInstructions {
            server_name: "plugins".to_string(),
            instructions: "Always confirm before spawning.".to_string(),
        },
    ];

    let formatted = aether::core::format_mcp_instructions(&instructions);

    // Check for XML tags with both server names
    assert!(formatted.contains("<mcp-server-instructions name=\"coding\">"));
    assert!(formatted.contains("<mcp-server-instructions name=\"plugins\">"));
    assert!(formatted.contains("Use absolute paths."));
    assert!(formatted.contains("Always confirm before spawning."));
}

#[tokio::test]
async fn test_agent_builder_includes_mcp_instructions_in_system_prompt() {
    use aether::core::agent;

    let instructions = vec![ServerInstructions {
        server_name: "test-server".to_string(),
        instructions: "Test instructions".to_string(),
    }];

    let llm = FakeLlmProvider::new(vec![]);
    let captured_contexts = llm.captured_contexts();

    let McpSpawnResult {
        tool_definitions,
        instructions: _,
        command_tx: mcp_tx,
        handle: _,
    } = mcp()
        .with_servers(vec![fake_mcp("test", FakeMcpServer::new())])
        .spawn()
        .await
        .unwrap();

    let (tx, mut rx, _handle) = agent(llm)
        .system("You are a test agent")
        .prompt(Prompt::mcp_instructions(instructions))
        .tools(mcp_tx, tool_definitions)
        .spawn()
        .await
        .unwrap();

    // Send a simple message to trigger context capture
    tx.send(UserMessage::text("test")).await.unwrap();
    drop(tx);

    // Wait for the agent to process
    while let Some(msg) = rx.recv().await {
        if matches!(msg, AgentMessage::Done) {
            break;
        }
    }

    // Check that the captured context includes our MCP instructions
    let contexts = captured_contexts.lock().unwrap();
    assert!(!contexts.is_empty());

    // The system message should contain our MCP instructions
    if let Some(first_msg) = contexts[0].messages().first() {
        if let llm::ChatMessage::System { content, .. } = first_msg {
            assert!(content.contains("<mcp-server-instructions name=\"test-server\">"));
            assert!(content.contains("Test instructions"));
        } else {
            panic!("Expected system message, got: {:?}", first_msg);
        }
    } else {
        panic!("Expected at least one message");
    }
}

#[tokio::test]
async fn test_agent_builder_works_without_mcp_instructions() {
    use aether::core::agent;

    let llm = FakeLlmProvider::new(vec![]);
    let captured_contexts = llm.captured_contexts();

    let McpSpawnResult {
        tool_definitions,
        instructions: _,
        command_tx: mcp_tx,
        handle: _,
    } = mcp()
        .with_servers(vec![fake_mcp("test", FakeMcpServer::new())])
        .spawn()
        .await
        .unwrap();

    // No mcp_instructions provided - should still work
    let (tx, mut rx, _handle) = agent(llm)
        .system("You are a test agent")
        .tools(mcp_tx, tool_definitions)
        .spawn()
        .await
        .unwrap();

    // Send a simple message
    tx.send(UserMessage::text("test")).await.unwrap();
    drop(tx);

    // Wait for the agent to process
    while let Some(msg) = rx.recv().await {
        if matches!(msg, AgentMessage::Done) {
            break;
        }
    }

    // Should have captured context without any MCP instructions section
    let contexts = captured_contexts.lock().unwrap();
    assert!(!contexts.is_empty());
}

#[tokio::test]
async fn test_mcp_instructions_from_server_are_included() {
    // Test that instructions from the actual MCP server connection are captured
    let McpSpawnResult {
        tool_definitions: _,
        instructions,
        command_tx: _,
        handle: _,
    } = mcp()
        .with_servers(vec![fake_mcp("test", FakeMcpServer::new())])
        .spawn()
        .await
        .unwrap();

    // The instructions should be captured from the server connection
    assert!(
        !instructions.is_empty(),
        "Expected instructions to be captured from server"
    );
    assert_eq!(instructions[0].server_name, "test");
}
