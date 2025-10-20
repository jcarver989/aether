mod test_terminal;

use aether::agent::AgentMessage;
use aether::llm::{ToolCallRequest, ToolCallResult};
use crossterm::style::Stylize;
use test_terminal::{TestTerminal, assert_buffer_eq, styled_to_string};
use wisp::colors::Theme;
use wisp::renderer::Renderer;

#[tokio::test]
async fn test_agent_message_text_chunks() {
    let (renderer, theme) = render(vec![
        text_chunk("Hello"),
        text_chunk(" World"),
        text_complete(""), // Signal completion
    ])
    .await;

    let prompt = styled_to_string("> ".with(theme.primary));

    // FIXED: Text chunks should render on the same line
    assert_buffer_eq(
        renderer.writer(),
        &[
            "Hello World",
            &format!("1G{}", prompt),
        ],
    );
}

#[tokio::test]
async fn test_agent_message_tool_call() {
    let (renderer, theme) = render(vec![tool_call("test_tool", r#"{"arg1": "value1"}"#)]).await;

    let tool_name = styled_to_string("● test_tool".with(theme.info));
    let tool_args = styled_to_string(r#" {"arg1": "value1"}"#.with(theme.info));
    let prompt = styled_to_string("> ".with(theme.primary));

    assert_buffer_eq(
        renderer.writer(),
        &[
            &format!("  {}{}", tool_name, tool_args),
            &format!("1G{}", prompt),
        ],
    );
}

#[tokio::test]
async fn test_agent_message_tool_result() {
    let args = r#"{"arg1": "value1"}"#;
    let (renderer, theme) = render(vec![
        tool_call("test_tool", args),
        tool_result("test_tool", args, "success"),
    ])
    .await;

    let tool_name_initial = styled_to_string("● test_tool".with(theme.info));
    let tool_name_success = styled_to_string("● test_tool ✓".with(theme.success));
    let tool_args = styled_to_string(format!(" {}", args).with(theme.info));
    let prompt = styled_to_string("> ".with(theme.primary));

    let line1 = format!("  {}{}", tool_name_initial, tool_args);
    let line2_full = format!(
        "                            {}{}",
        tool_name_success, tool_args
    );

    // Split line2 at character position 80 (terminal wraps at 80 columns)
    let line2: String = line2_full.chars().take(80).collect();
    let line3: String = line2_full.chars().skip(80).collect();
    let line4 = "8".to_string();
    let line5 = format!("1G{}", prompt);

    assert_buffer_eq(renderer.writer(), &[&line1, &line2, &line3, &line4, &line5]);
}

#[tokio::test]
async fn test_multiple_messages_sequence() {
    let args = r#"{"query": "test"}"#;
    let (renderer, theme) = render(vec![
        text_complete("Processing your request"),
        tool_call("search", args),
        tool_result("search", args, "found items"),
        text_complete("Found results"),
    ])
    .await;

    let tool_name_initial = styled_to_string("● search".with(theme.info));
    let tool_name_success = styled_to_string("● search ✓".with(theme.success));
    let tool_args = styled_to_string(format!(" {}", args).with(theme.info));
    let prompt = styled_to_string("> ".with(theme.primary));

    let line3_full = format!(
        "                         {}{}",
        tool_name_initial, tool_args
    );
    let line5_full = format!(
        "                            {}{}",
        tool_name_success, tool_args
    );

    let line1 = "Processing your request".to_string();
    // Split lines at character position 80 (terminal wraps at 80 columns)
    let line2: String = line3_full.chars().take(80).collect();
    let line3: String = line3_full.chars().skip(80).collect();
    let line4: String = line5_full.chars().take(80).collect();
    let line5: String = line5_full.chars().skip(80).collect();
    let line6 = "8".to_string();
    let line7 = "                       Found results".to_string();
    let line8 = format!("1G{}", prompt);

    assert_buffer_eq(
        renderer.writer(),
        &[
            &line1, &line2, &line3, &line4, &line5, &line6, &line7, &line8,
        ],
    );
}

async fn render(messages: Vec<AgentMessage>) -> (Renderer<TestTerminal>, Theme) {
    let terminal = TestTerminal::new(80, 40);
    let mut renderer = Renderer::new(terminal);

    for msg in messages {
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
