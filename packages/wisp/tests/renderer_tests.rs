mod test_terminal;

use aether::agent::AgentMessage;
use aether::llm::{ToolCallRequest, ToolCallResult};
use test_terminal::{assert_buffer_eq, TestTerminal};
use wisp::renderer::Renderer;

/// Helper to create a test renderer with a test terminal
fn create_test_renderer() -> Renderer<TestTerminal> {
    let terminal = TestTerminal::new(80, 24);
    Renderer::new(terminal)
}

#[tokio::test]
async fn test_agent_message_text_chunks() {
    let mut renderer = create_test_renderer();

    // Send first chunk
    let msg1 = AgentMessage::Text {
        message_id: "msg1".to_string(),
        chunk: "Hello".to_string(),
        is_complete: false,
        model_name: "test".to_string(),
    };
    renderer.on_agent_message(msg1).await.unwrap();

    // Verify the terminal contains the text
    let lines = renderer.writer().get_lines();
    assert!(lines.iter().any(|line| line.contains("Hello")));

    // Send second chunk
    let msg2 = AgentMessage::Text {
        message_id: "msg1".to_string(),
        chunk: " World".to_string(),
        is_complete: false,
        model_name: "test".to_string(),
    };
    renderer.on_agent_message(msg2).await.unwrap();

    // Both parts should be present (may be on different lines due to clearing)
    let lines = renderer.writer().get_lines();
    let buffer_str = lines.join("\n");
    assert!(buffer_str.contains("Hello"));
    assert!(buffer_str.contains("World"));

    // Complete the message
    let msg3 = AgentMessage::Text {
        message_id: "msg1".to_string(),
        chunk: "".to_string(),
        is_complete: true,
        model_name: "test".to_string(),
    };
    renderer.on_agent_message(msg3).await.unwrap();
}

#[tokio::test]
async fn test_agent_message_tool_call() {
    let mut renderer = create_test_renderer();

    let request = ToolCallRequest {
        id: "call_1".to_string(),
        name: "test_tool".to_string(),
        arguments: r#"{"arg1": "value1"}"#.to_string(),
    };

    let msg = AgentMessage::ToolCall {
        request,
        model_name: "test".to_string(),
    };

    renderer.on_agent_message(msg).await.unwrap();

    // Assert terminal buffer contains the tool name
    let lines = renderer.writer().get_lines();
    assert!(lines.iter().any(|line| line.contains("test_tool")));
}

#[tokio::test]
async fn test_agent_message_tool_result() {
    let mut renderer = create_test_renderer();

    // First, send a tool call request
    let request = ToolCallRequest {
        id: "call_1".to_string(),
        name: "test_tool".to_string(),
        arguments: r#"{"arg1": "value1"}"#.to_string(),
    };

    let msg1 = AgentMessage::ToolCall {
        request,
        model_name: "test".to_string(),
    };
    renderer.on_agent_message(msg1).await.unwrap();

    // Then send the result
    let result = ToolCallResult {
        id: "call_1".to_string(),
        name: "test_tool".to_string(),
        arguments: r#"{"arg1": "value1"}"#.to_string(),
        result: "success".to_string(),
    };

    let msg2 = AgentMessage::ToolResult {
        result,
        model_name: "test".to_string(),
    };
    renderer.on_agent_message(msg2).await.unwrap();

    // Assert terminal buffer shows the tool completed (with checkmark)
    let lines = renderer.writer().get_lines();
    let buffer_str = lines.join("\n");
    assert!(buffer_str.contains("test_tool"));
    assert!(buffer_str.contains("✓"));
}

#[tokio::test]
async fn test_multiple_messages_sequence() {
    let mut renderer = create_test_renderer();

    // Simulate a full conversation flow
    let messages = vec![
        AgentMessage::Text {
            message_id: "msg1".to_string(),
            chunk: "Processing your request".to_string(),
            is_complete: true,
            model_name: "test".to_string(),
        },
        AgentMessage::ToolCall {
            request: ToolCallRequest {
                id: "call_1".to_string(),
                name: "search".to_string(),
                arguments: r#"{"query": "test"}"#.to_string(),
            },
            model_name: "test".to_string(),
        },
        AgentMessage::ToolResult {
            result: ToolCallResult {
                id: "call_1".to_string(),
                name: "search".to_string(),
                arguments: r#"{"query": "test"}"#.to_string(),
                result: "found items".to_string(),
            },
            model_name: "test".to_string(),
        },
        AgentMessage::Text {
            message_id: "msg3".to_string(),
            chunk: "Found results".to_string(),
            is_complete: true,
            model_name: "test".to_string(),
        },
    ];

    for msg in messages {
        renderer.on_agent_message(msg).await.unwrap();
    }

    // Assert terminal buffer contains tool-related content
    // Note: Text messages may be cleared/overwritten, but tool status should remain
    let lines = renderer.writer().get_lines();
    let buffer_str = lines.join("\n");
    assert!(buffer_str.contains("search"));
    assert!(buffer_str.contains("✓"));
}

/// Example demonstrating direct buffer equality assertions
#[tokio::test]
async fn test_buffer_equality() {
    let mut terminal = TestTerminal::new(80, 24);

    use std::io::Write;
    write!(terminal, "Line 1\nLine 2\nLine 3").unwrap();

    // Assert exact buffer content
    assert_buffer_eq(&terminal, &["Line 1", "Line 2", "Line 3"]);
}
