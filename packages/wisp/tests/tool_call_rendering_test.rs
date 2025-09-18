use aether::agent::AgentMessage;
use std::collections::HashMap;

#[derive(Debug)]
struct MockPartialToolCall {
    name: String,
    model_name: String,
    arguments: String,
    is_spinner_active: bool,
}

/// Simulates the tool call tracking logic from Wisp's main.rs
/// This test reproduces the bug where tool calls appear on separate lines
fn simulate_tool_call_handling(messages: Vec<AgentMessage>) -> Vec<String> {
    let mut active_tool_calls: HashMap<String, MockPartialToolCall> = HashMap::new();
    let mut output_lines = Vec::new();

    for message in messages {
        if let AgentMessage::ToolCall {
            tool_call_id,
            name,
            arguments,
            result: _,
            is_complete,
            model_name,
        } = message
        {
            if is_complete {
                if let Some(tool_call) = active_tool_calls.get(&tool_call_id) {
                    if tool_call.is_spinner_active {
                        output_lines.push(format!("✓ ({}) Tool {}", model_name, tool_call.name));
                    } else {
                        // BUG: This creates a new line instead of replacing the existing one
                        output_lines.push(format!("✓ ({}) Tool {}", model_name, name));
                    }
                }
                active_tool_calls.remove(&tool_call_id);
            } else if !name.is_empty() {
                // Tool starting - create spinner
                output_lines.push(format!("○ Tool {} ({}) running...", name, model_name));
                active_tool_calls.insert(
                    tool_call_id.clone(),
                    MockPartialToolCall {
                        name: name.clone(),
                        model_name: model_name.clone(),
                        arguments: String::new(),
                        is_spinner_active: true,
                    },
                );
            } else if let Some(args_chunk) = arguments {
                // Tool argument chunk - accumulate arguments
                if let Some(tool_call) = active_tool_calls.get_mut(&tool_call_id) {
                    tool_call.arguments.push_str(&args_chunk);
                }
            }
        }
    }

    output_lines
}

#[test]
fn test_tool_call_rendering_bug() {
    // Simulate the message sequence that agent.rs sends
    let messages = vec![
        // ToolRequestStart
        AgentMessage::ToolCall {
            tool_call_id: "call_123".to_string(),
            name: "read_file".to_string(),
            arguments: None,
            result: None,
            is_complete: false,
            model_name: "llamacpp".to_string(),
        },
        // ToolRequestArg chunks
        AgentMessage::ToolCall {
            tool_call_id: "call_123".to_string(),
            name: String::new(), // Empty name in arg chunks
            arguments: Some(r#"{"file_path": "/path/to/file.rs"}"#.to_string()),
            result: None,
            is_complete: false,
            model_name: "llamacpp".to_string(),
        },
        // ToolRequestComplete with result
        AgentMessage::ToolCall {
            tool_call_id: "call_123".to_string(),
            name: "read_file".to_string(), // Name is present again
            arguments: Some(r#"{"file_path": "/path/to/file.rs"}"#.to_string()),
            result: Some("File contents here...".to_string()),
            is_complete: true,
            model_name: "llamacpp".to_string(),
        },
    ];

    let output = simulate_tool_call_handling(messages);

    // This demonstrates the bug: we get 2 lines instead of 1 replaced line
    assert_eq!(
        output.len(),
        2,
        "Should only have 1 line (replacement), but got: {:?}",
        output
    );
    assert_eq!(output[0], "○ Tool read_file (llamacpp) running...");
    assert_eq!(output[1], "✓ (llamacpp) Tool read_file");

    // The expected behavior should be just one final line: "✓ (llamacpp) Tool read_file"
}

#[test]
fn test_correct_tool_call_rendering() {
    // Same test but with corrected logic
    let messages = vec![
        AgentMessage::ToolCall {
            tool_call_id: "call_123".to_string(),
            name: "read_file".to_string(),
            arguments: None,
            result: None,
            is_complete: false,
            model_name: "llamacpp".to_string(),
        },
        AgentMessage::ToolCall {
            tool_call_id: "call_123".to_string(),
            name: String::new(),
            arguments: Some(r#"{"file_path": "/path/to/file.rs"}"#.to_string()),
            result: None,
            is_complete: false,
            model_name: "llamacpp".to_string(),
        },
        AgentMessage::ToolCall {
            tool_call_id: "call_123".to_string(),
            name: "read_file".to_string(),
            arguments: Some(r#"{"file_path": "/path/to/file.rs"}"#.to_string()),
            result: Some("File contents here...".to_string()),
            is_complete: true,
            model_name: "llamacpp".to_string(),
        },
    ];

    let output = simulate_corrected_tool_call_handling(messages);

    // Should only have the final completion line
    assert_eq!(output.len(), 1);
    assert_eq!(output[0], "✓ (llamacpp) Tool read_file");
}

#[tokio::test]
async fn test_app_view_handles_events_correctly() {
    // This test verifies that AppView can handle the typical event sequence
    // without trying to control conversation flow
    use wisp::app_view::AppView;

    let mut app_view = AppView::new();

    // Tool call starts
    app_view.update(AgentMessage::ToolCall {
        tool_call_id: "call_123".to_string(),
        name: "read_file".to_string(),
        arguments: None,
        result: None,
        is_complete: false,
        model_name: "llamacpp".to_string(),
    }).unwrap();

    // Text generation completes
    app_view.update(AgentMessage::Text {
        message_id: "msg_123".to_string(),
        chunk: "I'll read that file for you.".to_string(),
        is_complete: true,
        model_name: "llamacpp".to_string(),
    }).unwrap();

    // Tool call completes
    app_view.update(AgentMessage::ToolCall {
        tool_call_id: "call_123".to_string(),
        name: "read_file".to_string(),
        arguments: Some(r#"{"file_path": "/path/to/file.rs"}"#.to_string()),
        result: Some("File contents here...".to_string()),
        is_complete: true,
        model_name: "llamacpp".to_string(),
    }).unwrap();

    // Test passes if no panics occur - AppView just handles the events
}

/// Corrected version of the tool call handling logic
fn simulate_corrected_tool_call_handling(messages: Vec<AgentMessage>) -> Vec<String> {
    let mut active_tool_calls: HashMap<String, MockPartialToolCall> = HashMap::new();
    let mut output_lines = Vec::new();

    for message in messages {
        if let AgentMessage::ToolCall {
            tool_call_id,
            name,
            arguments,
            result: _,
            is_complete,
            model_name,
        } = message
        {
            if is_complete {
                // Tool completed - show final result (this replaces the spinner in real UI)
                if let Some(tool_call) = active_tool_calls.remove(&tool_call_id) {
                    output_lines.push(format!(
                        "✓ ({}) Tool {}",
                        tool_call.model_name, tool_call.name
                    ));
                }
            } else if !name.is_empty() {
                // Tool starting - this would show a spinner in real UI
                active_tool_calls.insert(
                    tool_call_id.clone(),
                    MockPartialToolCall {
                        name: name.clone(),
                        model_name: model_name.clone(),
                        arguments: String::new(),
                        is_spinner_active: true,
                    },
                );
            } else if let Some(args_chunk) = arguments {
                // Tool argument chunk - accumulate arguments
                if let Some(tool_call) = active_tool_calls.get_mut(&tool_call_id) {
                    tool_call.arguments.push_str(&args_chunk);
                }
            }
        }
    }

    output_lines
}
