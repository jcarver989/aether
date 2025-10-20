mod test_terminal;

use aether::agent::AgentMessage;
use aether::llm::{ToolCallRequest, ToolCallResult};
use test_terminal::{TestTerminal, assert_buffer_eq};
use wisp::colors::Theme;
use wisp::renderer::Renderer;

#[tokio::test]
async fn test_agent_message_text_chunks() {
    let (renderer, _theme) = render(vec![
        text_chunk("Hello"),
        text_chunk(" World"),
        text_complete(""), // Signal completion
    ])
    .await;

    assert_buffer_eq(
        renderer.writer(),
        &["Hello World", ">"],
    );
}

#[tokio::test]
async fn test_agent_message_tool_call() {
    let (renderer, _theme) = render(vec![tool_call("test_tool", r#"{"arg1": "value1"}"#)]).await;

    assert_buffer_eq(
        renderer.writer(),
        &[
            "● test_tool {\"arg1\": \"value1\"}",
            ">",
        ],
    );
}

#[tokio::test]
async fn test_agent_message_tool_result() {
    let args = r#"{"arg1": "value1"}"#;
    let (renderer, _theme) = render(vec![
        tool_call("test_tool", args),
        tool_result("test_tool", args, "success"),
    ])
    .await;

    // The tool result should have overwritten the tool call on the same line
    // So we should only see the success message, not both the initial and success
    assert_buffer_eq(
        renderer.writer(),
        &[
            "● test_tool ✓ {\"arg1\": \"value1\"}",
            ">",
            ">",  // Final prompt after tool result
        ],
    );
}

#[tokio::test]
async fn test_multiple_messages_sequence() {
    let args = r#"{"query": "test"}"#;
    let (renderer, _theme) = render(vec![
        text_complete("Processing your request"),
        tool_call("search", args),
        tool_result("search", args, "found items"),
        text_complete("Found results"),
    ])
    .await;

    // The tool result should have overwritten the tool call on the same line
    // So we should only see the success message, not both the initial and success
    assert_buffer_eq(
        renderer.writer(),
        &[
            "Processing your request",
            "● search ✓ {\"query\": \"test\"}",
            ">",
            "  Found results",  // Has leading spaces from clear line position
            ">",
        ],
    );
}

async fn render(messages: Vec<AgentMessage>) -> (Renderer<TestTerminal>, Theme) {
    let terminal = TestTerminal::new(200, 40);
    let mut renderer = Renderer::new(terminal);

    for msg in messages {
        // Update context with current terminal state
        let position = renderer.writer().cursor_position();
        let size = renderer.writer().size();
        renderer.update_render_context_with(position, size);
        renderer.on_agent_message(msg).await.unwrap();
    }

    (renderer, Theme::default())
}

fn text_chunk(chunk: &str) -> AgentMessage {
    AgentMessage::Text {
        message_id: "test_msg".to_string(),
        chunk: chunk.to_string(),
        is_complete: false,
        model_name: "test".to_string(),
    }
}

fn text_complete(text: &str) -> AgentMessage {
    AgentMessage::Text {
        message_id: "test_msg".to_string(),
        chunk: text.to_string(),
        is_complete: true,
        model_name: "test".to_string(),
    }
}

fn tool_call(name: &str, args: &str) -> AgentMessage {
    AgentMessage::ToolCall {
        request: ToolCallRequest {
            id: "test_call".to_string(),
            name: name.to_string(),
            arguments: args.to_string(),
        },
        model_name: "test".to_string(),
    }
}

fn tool_result(name: &str, args: &str, result: &str) -> AgentMessage {
    AgentMessage::ToolResult {
        result: ToolCallResult {
            id: "test_call".to_string(),
            name: name.to_string(),
            arguments: args.to_string(),
            result: result.to_string(),
        },
        model_name: "test".to_string(),
    }
}
